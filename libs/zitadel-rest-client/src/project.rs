//! Project endpoints — ADR-0016 §3 table, rows 1–4.
//!
//! Wire shapes come from the Zitadel Management API v1 proto
//! (`zitadel/management.proto` — `GetProjectByID`, `ListProjects`,
//! `AddProject`, `UpdateProject`). Only the fields the ensure-ops
//! (ADR-0017) actually read are modelled here; unknown fields are
//! tolerated via `#[serde(default)]` on collections and by never
//! using `deny_unknown_fields`.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::ZitadelClient;
use crate::error::ZitadelError;
use crate::pagination::PageRequest;

/// A Zitadel Project resource — fields used by ensure-ops only.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub project_role_assertion: bool,
    #[serde(default)]
    pub project_role_check: bool,
    #[serde(default)]
    pub has_project_check: bool,
}

/// Query filter for `POST /projects/_search`.
///
/// The full Zitadel API supports a typed `queries[]` array; ensure-ops
/// match projects by exact name so we expose only `name_query`. Drop
/// to raw JSON if a future op needs richer predicates.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectsRequest {
    #[serde(flatten)]
    pub page: PageRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<ProjectQuery>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectQuery {
    #[serde(rename = "nameQuery")]
    Name {
        name: String,
        /// Defaults to `TEXT_QUERY_METHOD_EQUALS` (0) when omitted.
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectsResponse {
    #[serde(default)]
    pub result: Vec<Project>,
    #[serde(default)]
    pub details: ListDetails,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDetails {
    #[serde(default)]
    pub total_result: String,
    #[serde(default)]
    pub processed_sequence: String,
    #[serde(default)]
    pub view_timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub project_role_assertion: bool,
    #[serde(default)]
    pub project_role_check: bool,
    #[serde(default)]
    pub has_project_check: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_labeling_setting: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectResponse {
    pub id: String,
    #[serde(default)]
    pub details: ObjectDetails,
}

/// Response envelope for updates and deletes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectDetails {
    #[serde(default)]
    pub sequence: String,
    #[serde(default)]
    pub creation_date: Option<String>,
    #[serde(default)]
    pub change_date: Option<String>,
    #[serde(default)]
    pub resource_owner: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub project_role_assertion: bool,
    #[serde(default)]
    pub project_role_check: bool,
    #[serde(default)]
    pub has_project_check: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_labeling_setting: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectResponse {
    #[serde(default)]
    pub details: ObjectDetails,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetProjectResponse {
    project: Project,
}

/// `GET /management/v1/projects/{id}` — ADR-0016 §3 row 1.
pub async fn get_project(client: &ZitadelClient, id: &str) -> Result<Project, ZitadelError> {
    let path = format!("/management/v1/projects/{id}");
    let resp: GetProjectResponse = client
        .send_json::<(), GetProjectResponse>(Method::GET, &path, None)
        .await?;
    Ok(resp.project)
}

/// `POST /management/v1/projects/_search` — ADR-0016 §3 row 2.
pub async fn list_projects(
    client: &ZitadelClient,
    req: &ListProjectsRequest,
) -> Result<ListProjectsResponse, ZitadelError> {
    client
        .send_json(Method::POST, "/management/v1/projects/_search", Some(req))
        .await
}

/// `POST /management/v1/projects` — ADR-0016 §3 row 3.
pub async fn create_project(
    client: &ZitadelClient,
    req: &CreateProjectRequest,
) -> Result<CreateProjectResponse, ZitadelError> {
    client
        .send_json(Method::POST, "/management/v1/projects", Some(req))
        .await
}

/// `PUT /management/v1/projects/{id}` — ADR-0016 §3 row 4.
pub async fn update_project(
    client: &ZitadelClient,
    id: &str,
    req: &UpdateProjectRequest,
) -> Result<UpdateProjectResponse, ZitadelError> {
    let path = format!("/management/v1/projects/{id}");
    client.send_json(Method::PUT, &path, Some(req)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AdminCredential;
    use wiremock::matchers::{body_json, header, method, path as mpath};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_for(server: &MockServer) -> ZitadelClient {
        ZitadelClient::for_test(&server.uri(), AdminCredential::Pat("dev-pat".into()))
    }

    #[tokio::test]
    async fn get_project_happy_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(mpath("/management/v1/projects/p-123"))
            .and(header("authorization", "Bearer dev-pat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "project": {
                    "id": "p-123",
                    "name": "pim",
                    "state": "PROJECT_STATE_ACTIVE",
                    "projectRoleAssertion": true
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let got = get_project(&client_for(&server), "p-123").await.unwrap();
        assert_eq!(got.id, "p-123");
        assert_eq!(got.name, "pim");
        assert!(got.project_role_assertion);
    }

    #[tokio::test]
    async fn create_project_maps_409_to_already_exists() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects"))
            .and(body_json(serde_json::json!({
                "name": "pim",
                "projectRoleAssertion": false,
                "projectRoleCheck": false,
                "hasProjectCheck": false
            })))
            .respond_with(ResponseTemplate::new(409).set_body_string(r#"{"code":6,"message":"exists"}"#))
            .expect(1)
            .mount(&server)
            .await;

        let req = CreateProjectRequest {
            name: "pim".into(),
            project_role_assertion: false,
            project_role_check: false,
            has_project_check: false,
            private_labeling_setting: None,
        };
        let err = create_project(&client_for(&server), &req).await.unwrap_err();
        assert!(matches!(err, ZitadelError::AlreadyExists(_)));
    }
}
