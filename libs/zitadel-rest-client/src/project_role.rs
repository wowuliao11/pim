//! Project role endpoints — ADR-0016 §3 table, rows 14–16.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::ZitadelClient;
use crate::error::ZitadelError;
use crate::pagination::PageRequest;
use crate::project::{ListDetails, ObjectDetails};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRole {
    pub key: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub group: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectRolesRequest {
    #[serde(flatten)]
    pub page: PageRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<ProjectRoleQuery>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectRoleQuery {
    #[serde(rename = "keyQuery")]
    Key {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectRolesResponse {
    #[serde(default)]
    pub result: Vec<ProjectRole>,
    #[serde(default)]
    pub details: ListDetails,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProjectRoleRequest {
    pub role_key: String,
    pub display_name: String,
    /// Ensure-ops that don't group roles pass an empty string, which
    /// Zitadel accepts as "no group".
    pub group: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProjectRoleResponse {
    #[serde(default)]
    pub details: ObjectDetails,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkAddProjectRolesRequest {
    pub roles: Vec<AddProjectRoleRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkAddProjectRolesResponse {
    #[serde(default)]
    pub details: ObjectDetails,
}

/// `POST /management/v1/projects/{project_id}/roles/_search` — ADR-0016 §3 row 14.
pub async fn list_project_roles(
    client: &ZitadelClient,
    project_id: &str,
    req: &ListProjectRolesRequest,
) -> Result<ListProjectRolesResponse, ZitadelError> {
    let path = format!("/management/v1/projects/{project_id}/roles/_search");
    client.send_json(Method::POST, &path, Some(req)).await
}

/// `POST /management/v1/projects/{project_id}/roles` — ADR-0016 §3 row 15.
pub async fn add_project_role(
    client: &ZitadelClient,
    project_id: &str,
    req: &AddProjectRoleRequest,
) -> Result<AddProjectRoleResponse, ZitadelError> {
    let path = format!("/management/v1/projects/{project_id}/roles");
    client.send_json(Method::POST, &path, Some(req)).await
}

/// `POST /management/v1/projects/{project_id}/roles/_bulk` — ADR-0016 §3 row 16.
pub async fn bulk_add_project_roles(
    client: &ZitadelClient,
    project_id: &str,
    req: &BulkAddProjectRolesRequest,
) -> Result<BulkAddProjectRolesResponse, ZitadelError> {
    let path = format!("/management/v1/projects/{project_id}/roles/_bulk");
    client.send_json(Method::POST, &path, Some(req)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AdminCredential;
    use wiremock::matchers::{body_json, header, method, path as mpath};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_for(s: &MockServer) -> ZitadelClient {
        ZitadelClient::for_test(&s.uri(), AdminCredential::Pat("p".into()))
    }

    #[tokio::test]
    async fn bulk_add_roles_happy_path() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects/proj-1/roles/_bulk"))
            .and(header("authorization", "Bearer p"))
            .and(body_json(serde_json::json!({
                "roles": [
                    {"roleKey": "admin", "displayName": "Admin", "group": ""},
                    {"roleKey": "reader", "displayName": "Reader", "group": ""}
                ]
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "details": {"sequence": "2"}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let req = BulkAddProjectRolesRequest {
            roles: vec![
                AddProjectRoleRequest {
                    role_key: "admin".into(),
                    display_name: "Admin".into(),
                    group: "".into(),
                },
                AddProjectRoleRequest {
                    role_key: "reader".into(),
                    display_name: "Reader".into(),
                    group: "".into(),
                },
            ],
        };
        bulk_add_project_roles(&client_for(&server), "proj-1", &req)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn add_role_409_maps_already_exists() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects/proj-1/roles"))
            .respond_with(ResponseTemplate::new(409).set_body_string("exists"))
            .mount(&server)
            .await;
        let req = AddProjectRoleRequest {
            role_key: "admin".into(),
            display_name: "Admin".into(),
            group: "".into(),
        };
        let err = add_project_role(&client_for(&server), "proj-1", &req)
            .await
            .unwrap_err();
        assert!(matches!(err, ZitadelError::AlreadyExists(_)));
    }
}
