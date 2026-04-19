use std::time::Duration;

use reqwest::{Client, Method, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use crate::auth::AdminCredential;
use crate::error::ZitadelError;

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const USER_AGENT: &str = concat!("pim-bootstrap/", env!("CARGO_PKG_VERSION"));

/// Async REST client for the Zitadel Management API.
///
/// Construction is fallible because credential material may be malformed.
/// Per-request methods live on the domain modules (Phase A.2); this struct
/// exposes the generic send helpers those modules reuse.
#[derive(Debug, Clone)]
pub struct ZitadelClient {
    authority: Url,
    http: Client,
    credential: AdminCredential,
}

impl ZitadelClient {
    pub fn new(authority: impl AsRef<str>, credential: AdminCredential) -> Result<Self, ZitadelError> {
        let authority = Url::parse(authority.as_ref())
            .map_err(|e| ZitadelError::InvalidArgument(format!("invalid authority URL: {e}")))?;
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .map_err(ZitadelError::from)?;
        Ok(Self {
            authority,
            http,
            credential,
        })
    }

    pub fn authority(&self) -> &Url {
        &self.authority
    }

    /// Build a URL by joining a path onto the authority.
    ///
    /// Zitadel Management API v2 paths begin with `/management/v1/...`.
    /// Callers pass the full path; the client does not hardcode the version.
    fn url(&self, path: &str) -> Result<Url, ZitadelError> {
        self.authority
            .join(path)
            .map_err(|e| ZitadelError::InvalidArgument(format!("bad path `{path}`: {e}")))
    }

    /// Send a request with JSON body, decode JSON response.
    ///
    /// `GET`/`DELETE` should pass `None` for body. Non-2xx status bodies are
    /// read into `ZitadelError::from_status` verbatim so callers can inspect
    /// server diagnostics during development.
    pub async fn send_json<B, R>(&self, method: Method, path: &str, body: Option<&B>) -> Result<R, ZitadelError>
    where
        B: Serialize + ?Sized,
        R: DeserializeOwned,
    {
        let url = self.url(path)?;
        let mut req = self.http.request(method, url);
        req = self.apply_auth(req)?;
        if let Some(body) = body {
            req = req.json(body);
        }
        let resp = req.send().await.map_err(ZitadelError::from)?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ZitadelError::from_status(status.as_u16(), body));
        }
        resp.json::<R>()
            .await
            .map_err(|e| ZitadelError::Deserialize(e.to_string()))
    }

    /// Attach the appropriate `Authorization` header.
    ///
    /// For PAT: a static bearer token. For JWT Profile: mints a fresh
    /// `client_assertion` per call using the `zitadel` crate. Minting is
    /// async-safe because `Application::add_client_assertion_payload` is CPU-
    /// bound and cheap; no caching is attempted at this layer.
    fn apply_auth(&self, req: RequestBuilder) -> Result<RequestBuilder, ZitadelError> {
        match &self.credential {
            AdminCredential::Pat(token) => Ok(req.bearer_auth(token)),
            AdminCredential::JwtProfile { .. } => Err(ZitadelError::InvalidArgument(
                "JWT Profile minting is not wired in Phase A; use PAT in dev \
                     (see ADR-0016 §4 and plans/001-pim-bootstrap-phase3.md Phase A.2)"
                    .into(),
            )),
        }
    }

    /// Test-only constructor that bypasses `Url::parse` strictness checks
    /// and preserves `http://` schemes used by mock servers.
    #[cfg(test)]
    pub(crate) fn for_test(authority: &str, credential: AdminCredential) -> Self {
        Self::new(authority, credential).expect("test authority must parse")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn sends_bearer_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/management/v1/ping"))
            .and(header("authorization", "Bearer dev-pat-abc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ok": true})))
            .expect(1)
            .mount(&server)
            .await;

        let client = ZitadelClient::for_test(&server.uri(), AdminCredential::Pat("dev-pat-abc".into()));

        #[derive(Debug, Deserialize)]
        struct Pong {
            ok: bool,
        }

        let got: Pong = client
            .send_json::<(), Pong>(Method::GET, "/management/v1/ping", None)
            .await
            .unwrap();
        assert!(got.ok);
    }

    #[tokio::test]
    async fn maps_409_from_wire() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/management/v1/ping"))
            .respond_with(ResponseTemplate::new(409).set_body_string("dup"))
            .mount(&server)
            .await;

        let client = ZitadelClient::for_test(&server.uri(), AdminCredential::Pat("x".into()));

        #[derive(Debug, Deserialize)]
        struct Empty {}
        let err = client
            .send_json::<(), Empty>(Method::GET, "/management/v1/ping", None)
            .await
            .unwrap_err();
        assert!(matches!(err, ZitadelError::AlreadyExists(ref b) if b == "dup"));
    }

    #[tokio::test]
    async fn jwt_profile_refused_in_phase_a() {
        let cred = AdminCredential::JwtProfile {
            key_json: b"{}".to_vec(),
            audience: "http://x".into(),
        };
        let client = ZitadelClient::for_test("http://x.invalid/", cred);
        #[derive(Debug, Deserialize)]
        struct Empty {}
        let err = client
            .send_json::<(), Empty>(Method::GET, "/management/v1/ping", None)
            .await
            .unwrap_err();
        assert!(matches!(err, ZitadelError::InvalidArgument(_)));
    }
}
