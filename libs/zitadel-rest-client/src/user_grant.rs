//! User grant endpoints — ADR-0016 §3 table, rows 17–20.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::ZitadelClient;
use crate::error::ZitadelError;
use crate::pagination::PageRequest;
use crate::project::{ListDetails, ObjectDetails};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserGrant {
    pub id: String,
    pub user_id: String,
    pub project_id: String,
    #[serde(default)]
    pub role_keys: Vec<String>,
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListUserGrantsRequest {
    #[serde(flatten)]
    pub page: PageRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<UserGrantQuery>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UserGrantQuery {
    #[serde(rename = "userIdQuery")]
    UserId { user_id: String },
    #[serde(rename = "projectIdQuery")]
    ProjectId { project_id: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListUserGrantsResponse {
    #[serde(default)]
    pub result: Vec<UserGrant>,
    #[serde(default)]
    pub details: ListDetails,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserGrantRequest {
    pub user_id: String,
    pub project_id: String,
    pub role_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserGrantResponse {
    pub user_grant_id: String,
    #[serde(default)]
    pub details: ObjectDetails,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserGrantRequest {
    pub role_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserGrantResponse {
    #[serde(default)]
    pub details: ObjectDetails,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveUserGrantResponse {
    #[serde(default)]
    pub details: ObjectDetails,
}

/// `POST /management/v1/user-grants/_search` — ADR-0016 §3 row 17.
pub async fn list_user_grants(
    client: &ZitadelClient,
    req: &ListUserGrantsRequest,
) -> Result<ListUserGrantsResponse, ZitadelError> {
    client
        .send_json(Method::POST, "/management/v1/user-grants/_search", Some(req))
        .await
}

/// `POST /management/v1/user-grants` — ADR-0016 §3 row 18.
pub async fn create_user_grant(
    client: &ZitadelClient,
    req: &CreateUserGrantRequest,
) -> Result<CreateUserGrantResponse, ZitadelError> {
    client
        .send_json(Method::POST, "/management/v1/user-grants", Some(req))
        .await
}

/// `PUT /management/v1/user-grants/{grant_id}` — ADR-0016 §3 row 19.
pub async fn update_user_grant(
    client: &ZitadelClient,
    grant_id: &str,
    req: &UpdateUserGrantRequest,
) -> Result<UpdateUserGrantResponse, ZitadelError> {
    let path = format!("/management/v1/user-grants/{grant_id}");
    client.send_json(Method::PUT, &path, Some(req)).await
}

/// `DELETE /management/v1/user-grants/{grant_id}` — ADR-0016 §3 row 20.
pub async fn remove_user_grant(
    client: &ZitadelClient,
    grant_id: &str,
) -> Result<RemoveUserGrantResponse, ZitadelError> {
    let path = format!("/management/v1/user-grants/{grant_id}");
    client
        .send_json::<(), RemoveUserGrantResponse>(Method::DELETE, &path, None)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AdminCredential;
    use wiremock::matchers::{header, method, path as mpath};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client_for(s: &MockServer) -> ZitadelClient {
        ZitadelClient::for_test(&s.uri(), AdminCredential::Pat("p".into()))
    }

    #[tokio::test]
    async fn create_user_grant_happy_path() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/user-grants"))
            .and(header("authorization", "Bearer p"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "userGrantId": "g-1",
                "details": {"sequence": "4"}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let req = CreateUserGrantRequest {
            user_id: "u-1".into(),
            project_id: "p-1".into(),
            role_keys: vec!["admin".into()],
        };
        let got = create_user_grant(&client_for(&server), &req).await.unwrap();
        assert_eq!(got.user_grant_id, "g-1");
    }

    #[tokio::test]
    async fn remove_grant_404_maps_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(mpath("/management/v1/user-grants/gone"))
            .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"code":5}"#))
            .mount(&server)
            .await;
        let err = remove_user_grant(&client_for(&server), "gone").await.unwrap_err();
        assert!(matches!(err, ZitadelError::NotFound(_)));
    }
}
