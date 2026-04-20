//! `ProjectEnsureOp` — ADR-0017 state machine over Zitadel project.
//!
//! Lookup is name-based via `list_projects` + `ProjectQuery::Name`: the id
//! isn't known on the first run. Comparison is name-only because the crate's
//! `ProjectSpec` only declares `name`. Populates `ctx.project_id` on Match /
//! Created; downstream ops depend on this.

use async_trait::async_trait;
use zitadel_rest_client::project::{
    create_project, list_projects, CreateProjectRequest, ListProjectsRequest, Project, ProjectQuery,
};
use zitadel_rest_client::{ZitadelClient, ZitadelError};

use crate::config::ProjectSpec;
use crate::ensure::{EnsureError, EnsureOp, EnsureOutcome, EnsureState, Flags};

use super::context::SharedContext;

pub struct ProjectEnsureOp {
    desired: ProjectSpec,
    ctx: SharedContext,
}

impl ProjectEnsureOp {
    pub fn new(desired: &ProjectSpec, ctx: SharedContext) -> Self {
        Self {
            desired: desired.clone(),
            ctx,
        }
    }

    fn stash_id(&self, id: &str) {
        if let Ok(mut c) = self.ctx.lock() {
            c.project_id = Some(id.to_string());
        }
    }
}

#[async_trait]
impl EnsureOp for ProjectEnsureOp {
    type Desired = ProjectSpec;
    type Observed = Project;

    fn name(&self) -> &str {
        "project"
    }

    fn desired(&self) -> &Self::Desired {
        &self.desired
    }

    async fn observe(&self, client: &ZitadelClient) -> Result<Option<Project>, ZitadelError> {
        let req = ListProjectsRequest {
            queries: Some(vec![ProjectQuery::Name {
                name: self.desired.name.clone(),
                method: None,
            }]),
            ..Default::default()
        };
        let resp = list_projects(client, &req).await?;
        Ok(resp.result.into_iter().find(|p| p.name == self.desired.name))
    }

    fn classify(&self, desired: &ProjectSpec, observed: Option<&Project>) -> EnsureState {
        match observed {
            None => EnsureState::Missing,
            Some(p) if p.name == desired.name => EnsureState::Match,
            Some(_) => EnsureState::Drift,
        }
    }

    async fn act(
        &self,
        state: EnsureState,
        _flags: Flags,
        observed: Option<Project>,
        client: &ZitadelClient,
    ) -> Result<EnsureOutcome, EnsureError> {
        match state {
            EnsureState::Missing => {
                let resp = create_project(
                    client,
                    &CreateProjectRequest {
                        name: self.desired.name.clone(),
                        project_role_assertion: false,
                        project_role_check: false,
                        has_project_check: false,
                        private_labeling_setting: None,
                    },
                )
                .await?;
                self.stash_id(&resp.id);
                Ok(EnsureOutcome::Created { id: resp.id })
            }
            EnsureState::Match => {
                let p = observed.expect("Match implies observed Some");
                self.stash_id(&p.id);
                Ok(EnsureOutcome::NoChange { id: p.id })
            }
            EnsureState::Drift => {
                let p = observed.expect("Drift implies observed Some");
                self.stash_id(&p.id);
                Ok(EnsureOutcome::Blocked {
                    reason: "project name drift ignored in Phase C (name is identity)".into(),
                })
            }
            EnsureState::Conflict => Err(EnsureError::Fatal {
                op: "project".into(),
                reason: "project name collides with unrelated object".into(),
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

    fn spec() -> ProjectSpec {
        ProjectSpec { name: "pim".into() }
    }

    fn mk_client(uri: String) -> ZitadelClient {
        ZitadelClient::new(uri, AdminCredential::Pat("t".into())).unwrap()
    }

    #[tokio::test]
    async fn observe_returns_none_when_list_empty() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects/_search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": []})))
            .mount(&server)
            .await;
        let op = ProjectEnsureOp::new(&spec(), new_shared_context());
        assert!(op.observe(&mk_client(server.uri())).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn observe_returns_project_when_name_matches() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects/_search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "result": [{"id": "p-1", "name": "pim"}]
            })))
            .mount(&server)
            .await;
        let op = ProjectEnsureOp::new(&spec(), new_shared_context());
        let got = op.observe(&mk_client(server.uri())).await.unwrap().unwrap();
        assert_eq!(got.id, "p-1");
    }

    #[tokio::test]
    async fn missing_state_creates_and_stashes_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": "p-new"})))
            .mount(&server)
            .await;
        let ctx = new_shared_context();
        let op = ProjectEnsureOp::new(&spec(), ctx.clone());
        let outcome = op
            .act(EnsureState::Missing, Flags::default(), None, &mk_client(server.uri()))
            .await
            .unwrap();
        assert_eq!(outcome, EnsureOutcome::Created { id: "p-new".into() });
        assert_eq!(ctx.lock().unwrap().project_id.as_deref(), Some("p-new"));
    }

    #[tokio::test]
    async fn match_state_stashes_existing_id_without_write() {
        let ctx = new_shared_context();
        let op = ProjectEnsureOp::new(&spec(), ctx.clone());
        let observed = Project {
            id: "p-ex".into(),
            name: "pim".into(),
            state: None,
            project_role_assertion: false,
            project_role_check: false,
            has_project_check: false,
        };
        let outcome = op
            .act(
                EnsureState::Match,
                Flags::default(),
                Some(observed),
                &mk_client("http://localhost:1/".into()),
            )
            .await
            .unwrap();
        assert_eq!(outcome, EnsureOutcome::NoChange { id: "p-ex".into() });
        assert_eq!(ctx.lock().unwrap().project_id.as_deref(), Some("p-ex"));
    }

    #[test]
    fn classify_maps_states() {
        let op = ProjectEnsureOp::new(&spec(), new_shared_context());
        assert_eq!(op.classify(&spec(), None), EnsureState::Missing);
        let p = Project {
            id: "x".into(),
            name: "pim".into(),
            state: None,
            project_role_assertion: false,
            project_role_check: false,
            has_project_check: false,
        };
        assert_eq!(op.classify(&spec(), Some(&p)), EnsureState::Match);
    }
}
