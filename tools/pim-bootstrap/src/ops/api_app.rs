//! `ApiAppEnsureOp` — idempotent ensure over the project's API app.
//!
//! Observed shape (`App { id, name, state }`) does not expose the app's
//! auth method, so comparison is name-only: Zitadel's REST surface doesn't
//! report enough for a Drift vs Match split on auth method. This is
//! documented behaviour per ADR-0016 §3; fixing it requires an amending
//! ADR. Reads `ctx.project_id`; fatal if absent (driver order bug).

use async_trait::async_trait;
use zitadel_rest_client::app::{
    create_api_app, list_apps, ApiAuthMethodType, App, AppQuery, CreateApiAppRequest, ListAppsRequest,
};
use zitadel_rest_client::{ZitadelClient, ZitadelError};

use crate::config::{ApiAppSpec, AppAuthMethod};
use crate::ensure::{EnsureError, EnsureOp, EnsureOutcome, EnsureState, Flags};

use super::context::SharedContext;

pub struct ApiAppEnsureOp {
    desired: ApiAppSpec,
    ctx: SharedContext,
}

impl ApiAppEnsureOp {
    pub fn new(desired: &ApiAppSpec, ctx: SharedContext) -> Self {
        Self {
            desired: desired.clone(),
            ctx,
        }
    }

    fn project_id(&self) -> Result<String, EnsureError> {
        self.ctx
            .lock()
            .ok()
            .and_then(|c| c.project_id.clone())
            .ok_or_else(|| EnsureError::Fatal {
                op: "api-app".into(),
                reason: "project_id not populated — ProjectEnsureOp must run first".into(),
            })
    }

    fn stash_id(&self, id: &str) {
        if let Ok(mut c) = self.ctx.lock() {
            c.api_app_id = Some(id.to_string());
        }
    }

    fn auth_method_type(&self) -> ApiAuthMethodType {
        match self.desired.auth_method {
            AppAuthMethod::JwtProfile => ApiAuthMethodType::PrivateKeyJwt,
            AppAuthMethod::ClientSecret => ApiAuthMethodType::Basic,
        }
    }
}

#[async_trait]
impl EnsureOp for ApiAppEnsureOp {
    type Desired = ApiAppSpec;
    type Observed = App;

    fn name(&self) -> &str {
        "api-app"
    }

    fn desired(&self) -> &Self::Desired {
        &self.desired
    }

    async fn observe(&self, client: &ZitadelClient) -> Result<Option<App>, ZitadelError> {
        let project_id = match self.project_id() {
            Ok(id) => id,
            Err(_) => return Ok(None),
        };
        let req = ListAppsRequest {
            queries: Some(vec![AppQuery::Name {
                name: self.desired.name.clone(),
                method: None,
            }]),
            ..Default::default()
        };
        let resp = list_apps(client, &project_id, &req).await?;
        Ok(resp.result.into_iter().find(|a| a.name == self.desired.name))
    }

    fn classify(&self, desired: &ApiAppSpec, observed: Option<&App>) -> EnsureState {
        match observed {
            None => EnsureState::Missing,
            Some(a) if a.name == desired.name => EnsureState::Match,
            Some(_) => EnsureState::Drift,
        }
    }

    async fn act(
        &self,
        state: EnsureState,
        _flags: Flags,
        observed: Option<App>,
        client: &ZitadelClient,
    ) -> Result<EnsureOutcome, EnsureError> {
        let project_id = self.project_id()?;
        match state {
            EnsureState::Missing => {
                let resp = create_api_app(
                    client,
                    &project_id,
                    &CreateApiAppRequest {
                        name: self.desired.name.clone(),
                        auth_method_type: self.auth_method_type(),
                    },
                )
                .await?;
                self.stash_id(&resp.app_id);
                Ok(EnsureOutcome::Created { id: resp.app_id })
            }
            EnsureState::Match => {
                let a = observed.expect("Match implies observed Some");
                self.stash_id(&a.id);
                Ok(EnsureOutcome::NoChange { id: a.id })
            }
            EnsureState::Drift => {
                let a = observed.expect("Drift implies observed Some");
                self.stash_id(&a.id);
                Ok(EnsureOutcome::Blocked {
                    reason: "api-app name drift not auto-resolved; delete manually or rename spec".into(),
                })
            }
            EnsureState::Conflict => Err(EnsureError::Fatal {
                op: "api-app".into(),
                reason: "api-app name collides with unrelated object".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::context::new_shared_context;
    use super::*;
    use wiremock::matchers::{method, path as mpath};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use zitadel_rest_client::AdminCredential;

    fn spec() -> ApiAppSpec {
        ApiAppSpec {
            name: "api-gateway".into(),
            auth_method: AppAuthMethod::JwtProfile,
        }
    }

    fn mk_client(uri: String) -> ZitadelClient {
        ZitadelClient::new(uri, AdminCredential::Pat("t".into())).unwrap()
    }

    fn ctx_with_project() -> SharedContext {
        let c = new_shared_context();
        c.lock().unwrap().project_id = Some("p-1".into());
        c
    }

    #[tokio::test]
    async fn act_missing_creates_and_stashes() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects/p-1/apps/api"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"appId": "a-1"})))
            .mount(&server)
            .await;
        let ctx = ctx_with_project();
        let op = ApiAppEnsureOp::new(&spec(), ctx.clone());
        let outcome = op
            .act(EnsureState::Missing, Flags::default(), None, &mk_client(server.uri()))
            .await
            .unwrap();
        assert_eq!(outcome, EnsureOutcome::Created { id: "a-1".into() });
        assert_eq!(ctx.lock().unwrap().api_app_id.as_deref(), Some("a-1"));
    }

    #[tokio::test]
    async fn act_without_project_id_is_fatal() {
        let op = ApiAppEnsureOp::new(&spec(), new_shared_context());
        let err = op
            .act(
                EnsureState::Missing,
                Flags::default(),
                None,
                &mk_client("http://localhost:1/".into()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, EnsureError::Fatal { .. }));
    }

    #[tokio::test]
    async fn observe_finds_matching_name() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects/p-1/apps/_search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": [{"id": "a-1", "name": "api-gateway"}]
            })))
            .mount(&server)
            .await;
        let op = ApiAppEnsureOp::new(&spec(), ctx_with_project());
        let got = op.observe(&mk_client(server.uri())).await.unwrap().unwrap();
        assert_eq!(got.id, "a-1");
    }

    #[test]
    fn auth_method_mapping() {
        let jwt = ApiAppEnsureOp::new(&spec(), new_shared_context());
        assert_eq!(jwt.auth_method_type(), ApiAuthMethodType::PrivateKeyJwt);
        let basic_spec = ApiAppSpec {
            name: "x".into(),
            auth_method: AppAuthMethod::ClientSecret,
        };
        let basic = ApiAppEnsureOp::new(&basic_spec, new_shared_context());
        assert_eq!(basic.auth_method_type(), ApiAuthMethodType::Basic);
    }
}
