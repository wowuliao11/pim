//! User endpoints — ADR-0016 §3 table, rows 9–10.
//!
//! Only machine users are creatable here; Zitadel's v2 user service
//! handles human users but ensure-ops (ADR-0017) only need machine
//! users for service-account provisioning.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::ZitadelClient;
use crate::error::ZitadelError;
use crate::pagination::PageRequest;
use crate::project::{ListDetails, ObjectDetails};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub user_name: String,
    #[serde(default)]
    pub human: Option<serde_json::Value>,
    #[serde(default)]
    pub machine: Option<MachineUser>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineUser {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// `API_AUTH_METHOD_TYPE_*` — present on users created with an
    /// access-token type explicitly set.
    #[serde(default)]
    pub access_token_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListUsersRequest {
    #[serde(flatten)]
    pub page: PageRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<UserQuery>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UserQuery {
    /// Exact-match on `user_name`. Ensure-ops look up machine users
    /// by their configured service-account username.
    #[serde(rename = "userNameQuery")]
    UserName {
        user_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListUsersResponse {
    #[serde(default)]
    pub result: Vec<User>,
    #[serde(default)]
    pub details: ListDetails,
}

/// Values mirror Zitadel proto enum `AccessTokenType`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AccessTokenType {
    #[serde(rename = "ACCESS_TOKEN_TYPE_BEARER")]
    Bearer,
    #[serde(rename = "ACCESS_TOKEN_TYPE_JWT")]
    Jwt,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMachineUserRequest {
    pub user_name: String,
    pub name: String,
    pub description: String,
    pub access_token_type: AccessTokenType,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMachineUserResponse {
    pub user_id: String,
    #[serde(default)]
    pub details: ObjectDetails,
}

/// `POST /management/v1/users/_search` — ADR-0016 §3 row 9.
pub async fn list_users(client: &ZitadelClient, req: &ListUsersRequest) -> Result<ListUsersResponse, ZitadelError> {
    client
        .send_json(Method::POST, "/management/v1/users/_search", Some(req))
        .await
}

/// `POST /management/v1/users/machine` — ADR-0016 §3 row 10.
pub async fn create_machine_user(
    client: &ZitadelClient,
    req: &CreateMachineUserRequest,
) -> Result<CreateMachineUserResponse, ZitadelError> {
    client
        .send_json(Method::POST, "/management/v1/users/machine", Some(req))
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
    async fn create_machine_user_happy_path() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/users/machine"))
            .and(header("authorization", "Bearer p"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "userId": "u-1",
                "details": {"sequence": "1"}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let req = CreateMachineUserRequest {
            user_name: "sa-gateway".into(),
            name: "SA Gateway".into(),
            description: "".into(),
            access_token_type: AccessTokenType::Jwt,
        };
        let got = create_machine_user(&client_for(&server), &req).await.unwrap();
        assert_eq!(got.user_id, "u-1");
    }

    #[tokio::test]
    async fn list_users_409_maps_already_exists() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/users/_search"))
            .respond_with(ResponseTemplate::new(409).set_body_string("dup"))
            .mount(&server)
            .await;
        let err = list_users(&client_for(&server), &ListUsersRequest::default())
            .await
            .unwrap_err();
        assert!(matches!(err, ZitadelError::AlreadyExists(_)));
    }
}
