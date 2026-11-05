use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use karate_api::identity::unix_now;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

pub struct Provider {
    pub issuer: String,
    pub ca: Vec<u8>,
    pub root: PathBuf,
    process: Child,
}
impl Provider {
    pub async fn start() -> Self {
        let tls = PathBuf::from(
            std::env::var("KARATE_TEST_TLS_DIR").expect("make test supplies TLS fixture"),
        );
        let root = tls.join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("plan.json"), "{}").unwrap();
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/oidc_https.py");
        let process = Command::new("python3")
            .arg(script)
            .arg(&root)
            .arg(&tls)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut provider = Self {
            issuer: String::new(),
            ca: std::fs::read(tls.join("ca.pem")).unwrap(),
            root,
            process,
        };
        let port = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(port) = std::fs::read_to_string(provider.root.join("port"))
                    && let Ok(port) = port.parse::<u16>()
                {
                    break port;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        provider.issuer = format!("https://localhost:{port}/issuer");
        provider.configure(provider.normal("fixture"));
        provider
    }
    pub fn normal(&self, kid: &str) -> Value {
        let file = PathBuf::from(std::env::var("KARATE_TEST_TLS_DIR").unwrap()).join(
            if kid == "fixture" {
                "jwks.json"
            } else {
                "jwks2.json"
            },
        );
        let mut jwks: Value =
            serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        jwks["keys"][0]["kid"] = json!(kid);
        json!({"/issuer/.well-known/openid-configuration":{"body":json!({"issuer":self.issuer,"jwks_uri":format!("{}/keys",self.issuer),"authorization_endpoint":format!("{}/authorize",self.issuer),"token_endpoint":format!("{}/token",self.issuer),"response_types_supported":["code"],"id_token_signing_alg_values_supported":["RS256"],"code_challenge_methods_supported":["S256"]}).to_string()},
            "/issuer/keys":{"body":jwks.to_string()}})
    }
    pub fn configure(&self, plan: Value) {
        let temp = self.root.join("next.json");
        std::fs::write(&temp, plan.to_string()).unwrap();
        std::fs::rename(temp, self.root.join("plan.json")).unwrap();
    }
    pub fn count(&self) -> usize {
        std::fs::read_to_string(self.root.join("requests"))
            .unwrap_or_default()
            .lines()
            .count()
    }
    pub fn token(&self, kid: &str) -> String {
        let now = unix_now().unwrap();
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.into());
        header.typ = Some("at+jwt".into());
        let file = PathBuf::from(std::env::var("KARATE_TEST_TLS_DIR").unwrap()).join(
            if kid == "fixture" {
                "key.der"
            } else {
                "key2.der"
            },
        );
        let key = EncodingKey::from_rsa_der(&std::fs::read(file).unwrap());
        encode(&header, &json!({"iss":self.issuer,"aud":"karate-api","sub":"discovery-user","iat":now-1,"exp":now+300}), &key).unwrap()
    }
}
impl Drop for Provider {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
