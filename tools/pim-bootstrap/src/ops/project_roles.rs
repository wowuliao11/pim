//! `ProjectRolesEnsureOp` — ensure the declared set of project roles exists.
//!
//! This op treats the role set as a single ensure-unit rather than running
//! one op per role, for three reasons:
//!
//! 1. Zitadel exposes a bulk endpoint (`/roles/_bulk`) that fits a single
//!    "missing-set" write.
//! 2. The pipeline report stays compact — one row instead of N.
//! 3. Drift semantics are clearer: "role set differs from desired" is a
//!    single state, not a fan-out.
//!
//! Drift is reported when a role exists but its `display_name` or `group`
//! differs. Zitadel's v1 Management API has no per-role update endpoint
//! (ADR-0016 §3), so drift is `Blocked` — operator must delete and re-add.
//!
//! Missing roles are added via `bulk_add_project_roles` on both Missing
//! (none present) and partial-presence (some present, some missing) cases.

use std::collections::HashMap;

use async_trait::async_trait;
use zitadel_rest_client::project_role::{
    bulk_add_project_roles, list_project_roles, AddProjectRoleRequest, BulkAddProjectRolesRequest,
    ListProjectRolesRequest, ProjectRole,
};
use zitadel_rest_client::{ZitadelClient, ZitadelError};

use crate::config::RoleSpec;
use crate::ensure::{EnsureError, EnsureOp, EnsureOutcome, EnsureState, Flags};

use super::context::SharedContext;

pub struct ProjectRolesEnsureOp {
    desired: Vec<RoleSpec>,
    ctx: SharedContext,
}

impl ProjectRolesEnsureOp {
    pub fn new(desired: &[RoleSpec], ctx: SharedContext) -> Self {
        Self {
            desired: desired.to_vec(),
            ctx,
        }
    }

    fn project_id(&self) -> Result<String, EnsureError> {
        self.ctx
            .lock()
            .ok()
            .and_then(|c| c.project_id.clone())
            .ok_or_else(|| EnsureError::Fatal {
                op: "project-roles".into(),
                reason: "project_id not populated — ProjectEnsureOp must run first".into(),
            })
    }

    /// Compute `(missing, drifted)` split against the observed role list.
    ///
    /// - `missing`: desired roles whose `key` is absent from observed.
    /// - `drifted`: desired roles whose `key` is present but `display_name`
    ///   or `group` differs.
    fn diff<'a>(desired: &'a [RoleSpec], observed: &[ProjectRole]) -> (Vec<&'a RoleSpec>, Vec<&'a RoleSpec>) {
        let by_key: HashMap<&str, &ProjectRole> = observed.iter().map(|r| (r.key.as_str(), r)).collect();
        let mut missing = Vec::new();
        let mut drifted = Vec::new();
        for d in desired {
            match by_key.get(d.key.as_str()) {
                None => missing.push(d),
                Some(o) => {
                    let desired_group = d.group.clone().unwrap_or_default();
                    if o.display_name != d.display_name || o.group != desired_group {
                        drifted.push(d);
                    }
                }
            }
        }
        (missing, drifted)
    }

    fn to_bulk_payload(roles: &[&RoleSpec]) -> BulkAddProjectRolesRequest {
        BulkAddProjectRolesRequest {
            roles: roles
                .iter()
                .map(|r| AddProjectRoleRequest {
                    role_key: r.key.clone(),
                    display_name: r.display_name.clone(),
                    group: r.group.clone().unwrap_or_default(),
                })
                .collect(),
        }
    }
}

/// Observed shape: the full current role list for the project.
#[derive(Debug, Clone)]
pub struct ObservedRoles(pub Vec<ProjectRole>);

#[async_trait]
impl EnsureOp for ProjectRolesEnsureOp {
    type Desired = Vec<RoleSpec>;
    type Observed = ObservedRoles;

    fn name(&self) -> &str {
        "project-roles"
    }

    fn desired(&self) -> &Self::Desired {
        &self.desired
    }

    async fn observe(&self, client: &ZitadelClient) -> Result<Option<ObservedRoles>, ZitadelError> {
        let project_id = match self.project_id() {
            Ok(id) => id,
            // No project_id → classify will be called with None and then act
            // surfaces the Fatal. Observe itself stays side-effect-free.
            Err(_) => return Ok(None),
        };
        let resp = list_project_roles(client, &project_id, &ListProjectRolesRequest::default()).await?;
        Ok(Some(ObservedRoles(resp.result)))
    }

    fn classify(&self, desired: &Vec<RoleSpec>, observed: Option<&ObservedRoles>) -> EnsureState {
        let Some(ObservedRoles(list)) = observed else {
            // No observation (e.g. missing project_id). Treat as Missing;
            // `act` surfaces the underlying Fatal when it reads project_id.
            return if desired.is_empty() {
                EnsureState::Match
            } else {
                EnsureState::Missing
            };
        };
        let (missing, drifted) = Self::diff(desired, list);
        match (missing.is_empty(), drifted.is_empty()) {
            (true, true) => EnsureState::Match,
            (false, _) if list.is_empty() => EnsureState::Missing,
            (false, _) => EnsureState::Drift, // some present, some missing → drift
            (true, false) => EnsureState::Drift,
        }
    }

    async fn act(
        &self,
        state: EnsureState,
        _flags: Flags,
        observed: Option<ObservedRoles>,
        client: &ZitadelClient,
    ) -> Result<EnsureOutcome, EnsureError> {
        let project_id = self.project_id()?;
        match state {
            EnsureState::Missing => {
                // Either observed=None (shouldn't happen post-project) or empty list.
                let refs: Vec<&RoleSpec> = self.desired.iter().collect();
                if refs.is_empty() {
                    return Ok(EnsureOutcome::NoChange { id: project_id });
                }
                bulk_add_project_roles(client, &project_id, &Self::to_bulk_payload(&refs)).await?;
                Ok(EnsureOutcome::Created {
                    id: format!("{}:roles", project_id),
                })
            }
            EnsureState::Match => Ok(EnsureOutcome::NoChange { id: project_id }),
            EnsureState::Drift => {
                let list = observed.map(|o| o.0).unwrap_or_default();
                let (missing, drifted) = Self::diff(&self.desired, &list);
                if !missing.is_empty() {
                    bulk_add_project_roles(client, &project_id, &Self::to_bulk_payload(&missing)).await?;
                }
                if drifted.is_empty() {
                    Ok(EnsureOutcome::Updated {
                        id: format!("{}:roles", project_id),
                        fields: missing.iter().map(|r| r.key.clone()).collect(),
                    })
                } else {
                    // Display-name / group drift on existing roles cannot be
                    // updated via the current REST surface (ADR-0016 §3).
                    Ok(EnsureOutcome::Blocked {
                        reason: format!(
                            "roles {:?} exist with different display_name/group; \
                             no per-role update endpoint — delete and re-run",
                            drifted.iter().map(|r| &r.key).collect::<Vec<_>>()
                        ),
                    })
                }
            }
            EnsureState::Conflict => Err(EnsureError::Fatal {
                op: "project-roles".into(),
                reason: "role set conflicts with unrelated project roles".into(),
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

    fn roles() -> Vec<RoleSpec> {
        vec![
            RoleSpec {
                key: "admin".into(),
                display_name: "Administrator".into(),
                group: None,
            },
            RoleSpec {
                key: "member".into(),
                display_name: "Member".into(),
                group: None,
            },
        ]
    }

    fn mk_client(uri: String) -> ZitadelClient {
        ZitadelClient::new(uri, AdminCredential::Pat("t".into())).unwrap()
    }

    fn ctx_with_project() -> SharedContext {
        let c = new_shared_context();
        c.lock().unwrap().project_id = Some("p-1".into());
        c
    }

    fn pr(key: &str, display: &str, group: &str) -> ProjectRole {
        ProjectRole {
            key: key.into(),
            display_name: display.into(),
            group: group.into(),
        }
    }

    #[test]
    fn classify_all_missing() {
        let op = ProjectRolesEnsureOp::new(&roles(), ctx_with_project());
        assert_eq!(
            op.classify(&roles(), Some(&ObservedRoles(vec![]))),
            EnsureState::Missing
        );
    }

    #[test]
    fn classify_all_match() {
        let op = ProjectRolesEnsureOp::new(&roles(), ctx_with_project());
        let observed = ObservedRoles(vec![pr("admin", "Administrator", ""), pr("member", "Member", "")]);
        assert_eq!(op.classify(&roles(), Some(&observed)), EnsureState::Match);
    }

    #[test]
    fn classify_partial_missing_is_drift() {
        let op = ProjectRolesEnsureOp::new(&roles(), ctx_with_project());
        let observed = ObservedRoles(vec![pr("admin", "Administrator", "")]);
        assert_eq!(op.classify(&roles(), Some(&observed)), EnsureState::Drift);
    }

    #[test]
    fn classify_display_name_drift() {
        let op = ProjectRolesEnsureOp::new(&roles(), ctx_with_project());
        let observed = ObservedRoles(vec![
            pr("admin", "Admin", ""), // drifted
            pr("member", "Member", ""),
        ]);
        assert_eq!(op.classify(&roles(), Some(&observed)), EnsureState::Drift);
    }

    #[tokio::test]
    async fn missing_state_bulk_adds_all_roles() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects/p-1/roles/_bulk"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        let op = ProjectRolesEnsureOp::new(&roles(), ctx_with_project());
        let outcome = op
            .act(
                EnsureState::Missing,
                Flags::default(),
                Some(ObservedRoles(vec![])),
                &mk_client(server.uri()),
            )
            .await
            .unwrap();
        assert!(matches!(outcome, EnsureOutcome::Created { .. }));
    }

    #[tokio::test]
    async fn drift_with_only_missing_subset_is_updated() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects/p-1/roles/_bulk"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;
        let op = ProjectRolesEnsureOp::new(&roles(), ctx_with_project());
        let observed = ObservedRoles(vec![pr("admin", "Administrator", "")]);
        let outcome = op
            .act(
                EnsureState::Drift,
                Flags::default(),
                Some(observed),
                &mk_client(server.uri()),
            )
            .await
            .unwrap();
        match outcome {
            EnsureOutcome::Updated { fields, .. } => {
                assert_eq!(fields, vec!["member".to_string()]);
            }
            other => panic!("expected Updated, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn drift_on_display_name_is_blocked() {
        let op = ProjectRolesEnsureOp::new(&roles(), ctx_with_project());
        let observed = ObservedRoles(vec![pr("admin", "Wrong Name", ""), pr("member", "Member", "")]);
        let outcome = op
            .act(
                EnsureState::Drift,
                Flags::default(),
                Some(observed),
                &mk_client("http://localhost:1/".into()),
            )
            .await
            .unwrap();
        match outcome {
            EnsureOutcome::Blocked { reason } => assert!(reason.contains("admin")),
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn act_without_project_id_is_fatal() {
        let op = ProjectRolesEnsureOp::new(&roles(), new_shared_context());
        let err = op
            .act(
                EnsureState::Missing,
                Flags::default(),
                Some(ObservedRoles(vec![])),
                &mk_client("http://localhost:1/".into()),
            )
            .await
            .unwrap_err();
        assert!(matches!(err, EnsureError::Fatal { .. }));
    }
}
