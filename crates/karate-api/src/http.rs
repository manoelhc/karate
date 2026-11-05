use crate::{invitations::InviteInput, keys::KeyInput, service::ProjectService};
use axum::{
    Extension, Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query, Request, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use karate_core::{Error, authorization::Role};
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
            Error::OwnerRequired => (StatusCode::CONFLICT, "OWNER_REQUIRED", "conflict", false),
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

async fn language(
    State(service): State<Arc<ProjectService>>,
    mut request: Request,
    next: Next,
) -> Response {
    let locale = Locale::negotiate(
        request
            .headers()
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    );
    request.extensions_mut().insert(locale);
    let path = request.uri().path();
    let auth_flow = path == "/v1/auth/callback" || path.ends_with("/login") || path == "/healthz";
    let check = if auth_flow {
        Ok(())
    } else {
        attach_session(&service, &mut request)
    };
    let mut response = match check {
        Ok(()) => next.run(request).await,
        Err(e) => ApiError(e, locale).into_response(),
    };
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MemberRoleInput {
    role: Role,
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeInput {
    expected_revision: i64,
}

pub fn router(service: Arc<ProjectService>) -> Router {
    Router::new()
        .route("/healthz", get(|| async { StatusCode::NO_CONTENT }))
        .route("/v1/tenants/{tenant}/projects", post(create).get(list))
        .route(
            "/v1/tenants/{tenant}/projects/{project}",
            get(read).patch(update),
        )
        .route("/v1/tenants/{tenant}/memberships", get(list_members))
        .route(
            "/v1/tenants/{tenant}/memberships/{membership}",
            axum::routing::patch(change_member),
        )
        .route(
            "/v1/tenants/{tenant}/memberships/{membership}/revoke",
            post(revoke_member),
        )
        .route(
            "/v1/tenants/{tenant}/invitations",
            post(invite).get(invitations),
        )
        .route(
            "/v1/tenants/{tenant}/invitations/{invitation}/accept",
            post(accept_invite),
        )
        .route(
            "/v1/tenants/{tenant}/invitations/{invitation}/revoke",
            post(revoke_invite),
        )
        .route("/v1/tenants/{tenant}/keys", post(create_key).get(list_keys))
        .route("/v1/tenants/{tenant}/keys/{key}/rotate", post(rotate_key))
        .route("/v1/tenants/{tenant}/keys/{key}/revoke", post(revoke_key))
        .route("/v1/tenants/{tenant}/login", post(browser_start))
        .route("/v1/auth/callback", get(browser_callback))
        .route("/v1/session", get(browser_session))
        .route("/v1/session/logout", post(browser_logout))
        .route("/v1/session/revoke-all", post(browser_revoke_all))
        .fallback(|Extension(locale): Extension<Locale>| async move {
            ApiError(Error::NotFound, locale)
        })
        .method_not_allowed_fallback(|Extension(locale): Extension<Locale>| async move {
            ApiError(Error::Validation, locale)
        })
        .layer(DefaultBodyLimit::max(8192))
        .layer(middleware::from_fn_with_state(service.clone(), language))
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

async fn list_members(
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
            .list_members(credential, tenant, input.limit.unwrap_or(25), input.after)
            .await
            .map_err(error)?
    )))
}
async fn change_member(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
    body: Result<Json<MemberRoleInput>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path((tenant, id)) = path.map_err(|_| error(Error::Validation))?;
    let Json(input) = body.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .change_member(
                credential,
                tenant,
                id,
                input.expected_revision,
                Some(input.role)
            )
            .await
            .map_err(error)?
    )))
}
async fn revoke_member(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
    body: Result<Json<RevokeInput>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path((tenant, id)) = path.map_err(|_| error(Error::Validation))?;
    let Json(input) = body.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .change_member(credential, tenant, id, input.expected_revision, None)
            .await
            .map_err(error)?
    )))
}

async fn invite(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    tenant: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<InviteInput>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path(tenant) = tenant.map_err(|_| error(Error::Validation))?;
    let Json(input) = body.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .create_invitation(credential, tenant, &input)
            .await
            .map_err(error)?
    )))
}
async fn invitations(
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
            .list_invitations(credential, tenant, input.limit.unwrap_or(25), input.after)
            .await
            .map_err(error)?
    )))
}
async fn accept_invite(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path((tenant, id)) = path.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .accept_invitation(credential, tenant, id)
            .await
            .map_err(error)?
    )))
}
async fn revoke_invite(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path((tenant, id)) = path.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .revoke_invitation(credential, tenant, id)
            .await
            .map_err(error)?
    )))
}

async fn create_key(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    tenant: Result<Path<Uuid>, PathRejection>,
    body: Result<Json<KeyInput>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path(tenant) = tenant.map_err(|_| error(Error::Validation))?;
    let Json(input) = body.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .create_key(credential, tenant, &input)
            .await
            .map_err(error)?
    )))
}
async fn list_keys(
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
            .list_keys(credential, tenant, input.limit.unwrap_or(25), input.after)
            .await
            .map_err(error)?
    )))
}
async fn rotate_key(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
    body: Result<Json<RevokeInput>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path((tenant, id)) = path.map_err(|_| error(Error::Validation))?;
    let Json(input) = body.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .rotate_key(credential, tenant, id, input.expected_revision)
            .await
            .map_err(error)?
    )))
}
async fn revoke_key(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
    body: Result<Json<RevokeInput>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    let Path((tenant, id)) = path.map_err(|_| error(Error::Validation))?;
    let Json(input) = body.map_err(|_| error(Error::Validation))?;
    Ok(Json(json!(
        service
            .revoke_key(credential, tenant, id, input.expected_revision)
            .await
            .map_err(error)?
    )))
}

const LOGIN_COOKIE: &str = "__Host-karate-login";
const SESSION_COOKIE: &str = "__Host-karate-session";
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, Error> {
    if headers.get_all("cookie").iter().count() > 1 {
        return Err(Error::Unauthenticated);
    }
    let Some(header) = headers.get("cookie") else {
        return Ok(None);
    };
    let header = header.to_str().map_err(|_| Error::Unauthenticated)?;
    if header.len() > 8192 {
        return Err(Error::Unauthenticated);
    }
    let mut found = None;
    for pair in header.split(';') {
        if let Some((key, value)) = pair.trim().split_once('=')
            && key == name
        {
            if found.is_some() {
                return Err(Error::Unauthenticated);
            }
            found = Some(value);
        }
    }
    Ok(found)
}
fn csrf(service: &ProjectService, headers: &HeaderMap) -> Result<(), Error> {
    if headers.get_all("origin").iter().count() != 1
        || headers.get_all("x-karate-csrf").iter().count() != 1
        || headers.get("origin").and_then(|v| v.to_str().ok()) != Some(service.browser_origin()?)
        || headers.get("x-karate-csrf").and_then(|v| v.to_str().ok()) != Some("1")
    {
        return Err(Error::Forbidden);
    }
    Ok(())
}
fn attach_session(service: &ProjectService, request: &mut Request) -> Result<(), Error> {
    if let Some(value) = cookie(request.headers(), SESSION_COOKIE)? {
        if value.len() != 134 || !value.starts_with("ks1.") {
            return Err(Error::Unauthenticated);
        }
        if request.headers().contains_key("authorization") {
            return Err(Error::Unauthenticated);
        }
        if request.method() != axum::http::Method::GET
            && request.method() != axum::http::Method::HEAD
        {
            csrf(service, request.headers())?;
        }
        let header = HeaderValue::from_str(&format!("Bearer {value}"))
            .map_err(|_| Error::Unauthenticated)?;
        request.headers_mut().insert("authorization", header);
    }
    Ok(())
}
fn set_cookie(response: &mut Response, name: &str, value: &str, age: i64) -> Result<(), Error> {
    let value = HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={age}"
    ))
    .map_err(|_| Error::Unavailable)?;
    response.headers_mut().append("set-cookie", value);
    Ok(())
}
async fn browser_start(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    tenant: Result<Path<Uuid>, PathRejection>,
) -> Result<Response, ApiError> {
    let error = |e| ApiError(e, locale);
    csrf(&service, &headers).map_err(error)?;
    let Path(tenant) = tenant.map_err(|_| error(Error::Validation))?;
    let login = service.begin_login(tenant).await.map_err(error)?;
    let mut response = Json(json!({"authorization_url":login.authorization_url})).into_response();
    set_cookie(&mut response, LOGIN_COOKIE, &login.binding, 300).map_err(error)?;
    Ok(response)
}
#[derive(Deserialize)]
struct Callback {
    state: String,
    code: Option<String>,
    error: Option<String>,
    iss: Option<String>,
}
async fn browser_callback(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
    query: Result<Query<Callback>, QueryRejection>,
) -> Result<Response, ApiError> {
    let error = |e| ApiError(e, locale);
    let Query(input) = query.map_err(|_| error(Error::Validation))?;
    let binding = cookie(&headers, LOGIN_COOKIE)
        .map_err(error)?
        .ok_or_else(|| error(Error::Unauthenticated))?;
    let code = if input.error.is_some() {
        None
    } else {
        input.code.as_deref()
    };
    let result = service
        .complete_login(&input.state, binding, code, input.iss.as_deref())
        .await;
    let mut response = match result {
        Ok(session) => {
            let mut response =
                Redirect::to(&format!("{}/", service.browser_origin().map_err(error)?))
                    .into_response();
            set_cookie(
                &mut response,
                SESSION_COOKIE,
                &session.credential,
                session.expires_at - crate::identity::unix_now().map_err(error)?,
            )
            .map_err(error)?;
            response
        }
        Err(e) => error(e).into_response(),
    };
    set_cookie(&mut response, LOGIN_COOKIE, "", 0).map_err(error)?;
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    Ok(response)
}
async fn browser_session(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let error = |e| ApiError(e, locale);
    let credential = bearer(&headers).map_err(error)?;
    Ok(Json(json!(
        service.session_info(credential).await.map_err(error)?
    )))
}
async fn close_browser(
    service: Arc<ProjectService>,
    locale: Locale,
    headers: HeaderMap,
    all: bool,
) -> Result<Response, ApiError> {
    let error = |e| ApiError(e, locale);
    csrf(&service, &headers).map_err(error)?;
    service
        .logout(bearer(&headers).map_err(error)?, all)
        .await
        .map_err(error)?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    set_cookie(&mut response, SESSION_COOKIE, "", 0).map_err(error)?;
    Ok(response)
}
async fn browser_logout(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    close_browser(service, locale, headers, false).await
}
async fn browser_revoke_all(
    State(service): State<Arc<ProjectService>>,
    Extension(locale): Extension<Locale>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    close_browser(service, locale, headers, true).await
}
