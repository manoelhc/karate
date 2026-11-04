use crate::service::ProjectService;
use axum::{
    Extension, Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query, Request, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use karate_core::Error;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    En,
    PtBr,
}

impl Locale {
    pub fn negotiate(header: &str) -> Self {
        if header.len() > 1024 {
            return Self::En;
        }
        let mut best = (0.0_f32, Self::En);
        for part in header.split(',').take(32) {
            let mut parts = part.trim().split(';');
            let tag = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
            let quality = match parts.next() {
                None => 1.0,
                Some(value) => match value
                    .trim()
                    .strip_prefix("q=")
                    .and_then(|q| q.parse::<f32>().ok())
                {
                    Some(q) if q.is_finite() && (0.0..=1.0).contains(&q) => q,
                    _ => continue,
                },
            };
            if parts.next().is_some() || quality <= best.0 {
                continue;
            }
            let language = tag.split('-').next().unwrap_or_default();
            let locale = match language {
                "pt" => Self::PtBr,
                "en" | "*" => Self::En,
                _ => continue,
            };
            best = (quality, locale);
        }
        best.1
    }
    fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::PtBr => "pt-BR",
        }
    }
}

type Catalog = BTreeMap<String, [String; 2]>;
fn catalog(locale: Locale) -> &'static Catalog {
    static EN: OnceLock<Catalog> = OnceLock::new();
    static PT: OnceLock<Catalog> = OnceLock::new();
    match locale {
        Locale::En => EN.get_or_init(|| {
            serde_json::from_str(include_str!("../locales/en.json"))
                .expect("checked locale catalog")
        }),
        Locale::PtBr => PT.get_or_init(|| {
            serde_json::from_str(include_str!("../locales/pt-BR.json"))
                .expect("checked locale catalog")
        }),
    }
}

pub struct ApiError(Error, Locale);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, category, retryable) = match self.0 {
            Error::Validation => (StatusCode::BAD_REQUEST, "VALIDATION", "input", false),
            Error::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHENTICATED",
                "authentication",
                false,
            ),
            Error::Forbidden | Error::OutOfScope | Error::Unverified => {
                (StatusCode::FORBIDDEN, "FORBIDDEN", "authorization", false)
            }
            Error::NotFound => (StatusCode::NOT_FOUND, "NOT_FOUND", "resource", false),
            Error::Conflict => (StatusCode::CONFLICT, "CONFLICT", "conflict", false),
            Error::LimitReached => (
                StatusCode::TOO_MANY_REQUESTS,
                "LIMIT_REACHED",
                "capacity",
                false,
            ),
            Error::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "UNAVAILABLE",
                "dependency",
                true,
            ),
        };
        let text = &catalog(self.1)[code];
        let mut response=(status,Json(json!({"code":code,"category":category,"message":text[0],"retryable":retryable,"remediation":text[1]}))).into_response();
        response
            .headers_mut()
            .insert("content-language", HeaderValue::from_static(self.1.tag()));
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert("www-authenticate", HeaderValue::from_static("Bearer"));
        }
        response
    }
}

async fn language(mut request: Request, next: Next) -> Response {
    let locale = Locale::negotiate(
        request
            .headers()
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    );
    request.extensions_mut().insert(locale);
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert("content-language", HeaderValue::from_static(locale.tag()));
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
}

fn bearer(headers: &HeaderMap) -> Result<&str, Error> {
    if headers.get_all("authorization").iter().count() != 1 {
        return Err(Error::Unauthenticated);
    }
    headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("Bearer"))
        .map(|(_, token)| token)
        .filter(|token| !token.is_empty() && token.len() <= 8192)
        .ok_or(Error::Unauthenticated)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateInput {
    project_id: Uuid,
    name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateInput {
    name: String,
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListInput {
    limit: Option<i64>,
    after: Option<Uuid>,
}

pub fn router(service: Arc<ProjectService>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { StatusCode::NO_CONTENT }))
        .route("/v1/tenants/{tenant}/projects", post(create).get(list))
        .route(
            "/v1/tenants/{tenant}/projects/{project}",
            get(read).patch(update),
        )
        .fallback(|Extension(locale): Extension<Locale>| async move {
            ApiError(Error::NotFound, locale)
        })
        .method_not_allowed_fallback(|Extension(locale): Extension<Locale>| async move {
            ApiError(Error::Validation, locale)
        })
        .layer(DefaultBodyLimit::max(8192))
        .layer(middleware::from_fn(language))
        .with_state(service)
}

async fn create(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    tenant: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<CreateInput>, JsonRejection>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path(tenant) = tenant.map_err(|_| error(Error::Validation))?;
    let Json(input) = body.map_err(|_| error(Error::Validation))?;
    let (project, created) = service
        .create(credential, tenant, input.project_id, &input.name)
        .await
        .map_err(error)?;
    Ok((
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(json!(project)),
    ))
}

async fn read(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path((tenant, id)) = path.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service.get(credential, tenant, id).await.map_err(error)?
    )))
}

async fn list(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    tenant: Result<Path<Uuid>, PathRejection>,
    query: Result<Query<ListInput>, QueryRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path(tenant) = tenant.map_err(|_| error(Error::Validation))?;
    let Query(input) = query.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .list(credential, tenant, input.limit.unwrap_or(25), input.after)
            .await
            .map_err(error)?
    )))
}

async fn update(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
    body: Result<Json<UpdateInput>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path((tenant, id)) = path.map_err(|_| error(Error::Validation))?;
    let Json(input) = body.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .update(credential, tenant, id, &input.name, input.expected_revision)
            .await
            .map_err(error)?
    )))
}
