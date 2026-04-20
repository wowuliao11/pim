//! `ServiceAccountEnsureOp` — ensure a machine user with a JWT-profile key.
//!
//! Two resources in one op because Zitadel models them as parent (machine
//! user) and child (key) and no ensure-op downstream cares about them
//! independently:
//!
//! 1. Machine user with `access_token_type = JWT`. Looked up by `user_name`
//!    via `list_users` + `UserQuery::UserName`. Missing → `create_machine_user`.
//! 2. JWT key on that user. Missing → `add_machine_user_key(KeyType::Json)`.
//!    The base64 key blob Zitadel returns once is stashed into
//!    `ctx.jwt_key_blob` for the sink layer (Phase D).
//!
//! Drift handling is restricted because `MachineUser` has no update endpoint
//! in Zitadel's v1 management API: a `description` change on a declarative
//! spec cannot be reconciled in place and is reported as `Blocked` for an
//! operator to resolve manually.
//!
//! Rotation: `--rotate-keys` on a `Match` issues a fresh key (and stashes
//! it) without deleting the old one — the sink layer decides atomic
//! file-replacement semantics.

use async_trait::async_trait;
use chrono::{Duration, Utc};
use zitadel_rest_client::user::{
    create_machine_user, list_users, AccessTokenType, CreateMachineUserRequest, ListUsersRequest, MachineUser, User,
    UserQuery,
};
use zitadel_rest_client::user_key::{add_machine_user_key, AddMachineUserKeyRequest, KeyType};
use zitadel_rest_client::{ZitadelClient, ZitadelError};

use crate::config::ServiceAccountSpec;
use crate::ensure::{EnsureError, EnsureOp, EnsureOutcome, EnsureState, Flags};

use super::context::SharedContext;

/// Years of validity Zitadel-issued JWT keys carry. Long enough that key
/// rotation is an operator decision, not a clock-driven event. 20y matches
/// the PIM operational model documented in ADR-0005.
const JWT_KEY_VALIDITY_YEARS: i64 = 20;

pub struct ServiceAccountEnsureOp {
    desired: ServiceAccountSpec,
    ctx: SharedContext,
}

/// Observed shape for this op — a user plus its (stable) identity fields.
///
/// We don't observe the key set: classification only needs the machine
/// user's current description to decide Match vs Drift. Key presence /
/// rotation is handled inside `act` based on flags and create-vs-match.
#[derive(Debug, Clone)]
pub struct ServiceAccountObserved {
    pub user_id: String,
    pub user_name: String,
    pub machine: MachineUser,
}

impl ServiceAccountEnsureOp {
    pub fn new(desired: &ServiceAccountSpec, ctx: SharedContext) -> Self {
        Self {
            desired: desired.clone(),
            ctx,
        }
    }

    fn stash_user_id(&self, id: &str) {
        if let Ok(mut c) = self.ctx.lock() {
            c.sa_user_id = Some(id.to_string());
        }
    }

    fn stash_key_blob(&self, blob: &str) {
        if let Ok(mut c) = self.ctx.lock() {
            c.jwt_key_blob = Some(blob.to_string());
        }
    }

    fn expiration_rfc3339() -> String {
        // 365 * JWT_KEY_VALIDITY_YEARS is a pragmatic approximation; Zitadel
        // only requires a future RFC-3339 timestamp, not calendar-accurate.
        let when = Utc::now() + Duration::days(365 * JWT_KEY_VALIDITY_YEARS);
        when.to_rfc3339()
    }

    async fn add_key(&self, user_id: &str, client: &ZitadelClient) -> Result<String, ZitadelError> {
        let resp = add_machine_user_key(
            client,
            user_id,
            &AddMachineUserKeyRequest {
                r#type: KeyType::Json,
                expiration_date: Self::expiration_rfc3339(),
            },
        )
        .await?;
        Ok(resp.key_details)
    }
}

#[async_trait]
impl EnsureOp for ServiceAccountEnsureOp {
    type Desired = ServiceAccountSpec;
    type Observed = ServiceAccountObserved;

    fn name(&self) -> &str {
        "service-account"
    }

    fn desired(&self) -> &Self::Desired {
        &self.desired
    }

    async fn observe(&self, client: &ZitadelClient) -> Result<Option<ServiceAccountObserved>, ZitadelError> {
        let req = ListUsersRequest {
            queries: Some(vec![UserQuery::UserName {
                user_name: self.desired.username.clone(),
                method: None,
            }]),
            ..Default::default()
        };
        let resp = list_users(client, &req).await?;
        // Name-query is a server-side filter; still re-check locally because
        // Zitadel accepts substring matches in some query-method modes.
        let matched: Option<User> = resp
            .result
            .into_iter()
            .find(|u| u.user_name == self.desired.username && u.machine.is_some());
        Ok(matched.map(|u| ServiceAccountObserved {
            user_id: u.id,
            user_name: u.user_name,
            machine: u.machine.expect("filter above ensures machine is Some"),
        }))
    }

    fn classify(&self, desired: &ServiceAccountSpec, observed: Option<&ServiceAccountObserved>) -> EnsureState {
        match observed {
            None => EnsureState::Missing,
            Some(o) => {
                let desired_desc = desired.description.clone().unwrap_or_default();
                if o.machine.description == desired_desc {
                    EnsureState::Match
                } else {
                    EnsureState::Drift
                }
            }
        }
    }

    async fn act(
        &self,
        state: EnsureState,
        flags: Flags,
        observed: Option<ServiceAccountObserved>,
        client: &ZitadelClient,
    ) -> Result<EnsureOutcome, EnsureError> {
        match state {
            EnsureState::Missing => {
                let resp = create_machine_user(
                    client,
                    &CreateMachineUserRequest {
                        user_name: self.desired.username.clone(),
                        name: self.desired.username.clone(),
                        description: self.desired.description.clone().unwrap_or_default(),
                        access_token_type: AccessTokenType::Jwt,
                    },
                )
                .await?;
                self.stash_user_id(&resp.user_id);
                // Fresh machine user → always issue a key so downstream sinks
                // have material to write. `add_machine_user_key` is the only
                // way to obtain the JSON key blob.
                let blob = self.add_key(&resp.user_id, client).await?;
                self.stash_key_blob(&blob);
                Ok(EnsureOutcome::SecretsEmitted { count: 1 })
            }
            EnsureState::Match => {
                let o = observed.expect("Match implies observed Some");
                self.stash_user_id(&o.user_id);
                if flags.rotate_keys {
                    let blob = self.add_key(&o.user_id, client).await?;
                    self.stash_key_blob(&blob);
                    Ok(EnsureOutcome::SecretsEmitted { count: 1 })
                } else {
                    Ok(EnsureOutcome::NoChange { id: o.user_id })
                }
            }
            EnsureState::Drift => {
                let o = observed.expect("Drift implies observed Some");
                self.stash_user_id(&o.user_id);
                // MachineUser has no update endpoint in ADR-0016 §3. Report
                // a blocked outcome so the operator can delete + recreate.
                Ok(EnsureOutcome::Blocked {
                    reason: "machine user description drift cannot be updated via REST; \
                             delete the user in Zitadel UI and re-run"
                        .into(),
                })
            }
            EnsureState::Conflict => Err(EnsureError::Fatal {
                op: "service-account".into(),
                reason: "service-account username collides with unrelated user".into(),
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

    fn spec() -> ServiceAccountSpec {
        ServiceAccountSpec {
            username: "user-service-sa".into(),
            description: None,
        }
    }

    fn mk_client(uri: String) -> ZitadelClient {
        ZitadelClient::new(uri, AdminCredential::Pat("t".into())).unwrap()
    }

    fn observed_match() -> ServiceAccountObserved {
        ServiceAccountObserved {
            user_id: "u-1".into(),
            user_name: "user-service-sa".into(),
            machine: MachineUser {
                name: "user-service-sa".into(),
                description: String::new(),
                access_token_type: Some("ACCESS_TOKEN_TYPE_JWT".into()),
            },
        }
    }

    #[tokio::test]
    async fn observe_returns_none_when_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/users/_search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": []})))
            .mount(&server)
            .await;
        let op = ServiceAccountEnsureOp::new(&spec(), new_shared_context());
        assert!(op.observe(&mk_client(server.uri())).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn observe_ignores_human_users_with_same_name() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/users/_search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": [{
                    "id": "u-human",
                    "userName": "user-service-sa",
                    "human": {"profile": {}}
                }]
            })))
            .mount(&server)
            .await;
        let op = ServiceAccountEnsureOp::new(&spec(), new_shared_context());
        // Human user with colliding username: classify does not concern this op;
        // observe must skip it so `Missing` is reported and create can proceed.
        assert!(op.observe(&mk_client(server.uri())).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn missing_state_creates_user_and_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/users/machine"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"userId": "u-new"})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/users/u-new/keys"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "keyId": "k-1", "keyDetails": "YmxvYg=="
            })))
            .mount(&server)
            .await;
        let ctx = new_shared_context();
        let op = ServiceAccountEnsureOp::new(&spec(), ctx.clone());
        let outcome = op
            .act(EnsureState::Missing, Flags::default(), None, &mk_client(server.uri()))
            .await
            .unwrap();
        assert_eq!(outcome, EnsureOutcome::SecretsEmitted { count: 1 });
        let guard = ctx.lock().unwrap();
        assert_eq!(guard.sa_user_id.as_deref(), Some("u-new"));
        assert_eq!(guard.jwt_key_blob.as_deref(), Some("YmxvYg=="));
    }

    #[tokio::test]
    async fn match_without_rotate_is_noop() {
        let ctx = new_shared_context();
        let op = ServiceAccountEnsureOp::new(&spec(), ctx.clone());
        let outcome = op
            .act(
                EnsureState::Match,
                Flags::default(),
                Some(observed_match()),
                &mk_client("http://localhost:1/".into()),
            )
            .await
            .unwrap();
        assert_eq!(outcome, EnsureOutcome::NoChange { id: "u-1".into() });
        let guard = ctx.lock().unwrap();
        assert_eq!(guard.sa_user_id.as_deref(), Some("u-1"));
        assert!(guard.jwt_key_blob.is_none(), "no new key on plain Match");
    }

    #[tokio::test]
    async fn match_with_rotate_flag_issues_new_key() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/users/u-1/keys"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "keyId": "k-2", "keyDetails": "cm90YXRlZA=="
            })))
            .mount(&server)
            .await;
        let ctx = new_shared_context();
        let op = ServiceAccountEnsureOp::new(&spec(), ctx.clone());
        let outcome = op
            .act(
                EnsureState::Match,
                Flags {
                    sync: false,
                    rotate_keys: true,
                },
                Some(observed_match()),
                &mk_client(server.uri()),
            )
            .await
            .unwrap();
        assert_eq!(outcome, EnsureOutcome::SecretsEmitted { count: 1 });
        assert_eq!(ctx.lock().unwrap().jwt_key_blob.as_deref(), Some("cm90YXRlZA=="));
    }

    #[tokio::test]
    async fn drift_is_blocked_with_operator_instruction() {
        let ctx = new_shared_context();
        let op = ServiceAccountEnsureOp::new(
            &ServiceAccountSpec {
                username: "user-service-sa".into(),
                description: Some("new description".into()),
            },
            ctx.clone(),
        );
        let outcome = op
            .act(
                EnsureState::Drift,
                Flags::default(),
                Some(observed_match()),
                &mk_client("http://localhost:1/".into()),
            )
            .await
            .unwrap();
        match outcome {
            EnsureOutcome::Blocked { reason } => assert!(reason.contains("cannot be updated")),
            other => panic!("expected Blocked, got {other:?}"),
        }
        assert_eq!(ctx.lock().unwrap().sa_user_id.as_deref(), Some("u-1"));
    }

    #[test]
    fn classify_matches_when_description_equals_desired() {
        let op = ServiceAccountEnsureOp::new(&spec(), new_shared_context());
        let o = observed_match();
        assert_eq!(op.classify(&spec(), Some(&o)), EnsureState::Match);
        assert_eq!(op.classify(&spec(), None), EnsureState::Missing);

        let with_desc = ServiceAccountSpec {
            username: "user-service-sa".into(),
            description: Some("different".into()),
        };
        assert_eq!(op.classify(&with_desc, Some(&o)), EnsureState::Drift);
    }
}
