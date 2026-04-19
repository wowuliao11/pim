//! Machine-user key endpoints — ADR-0016 §3 table, rows 11–13.
//!
//! Phase A.2 wires only JSON key creation because `user-service`
//! authenticates to Zitadel via JWT Profile (private-key-JWT) per
//! ADR-0005; PATs are not needed from this endpoint set.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::ZitadelClient;
use crate::error::ZitadelError;
use crate::pagination::PageRequest;
use crate::project::{ListDetails, ObjectDetails};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub id: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub expiration_date: Option<String>,
}

/// Values mirror Zitadel proto enum `KeyType`. `JSON` is the
/// format stored on disk and fed to `zitadel::credentials::Application`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum KeyType {
    #[serde(rename = "KEY_TYPE_UNSPECIFIED")]
    Unspecified,
    #[serde(rename = "KEY_TYPE_JSON")]
    Json,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMachineUserKeysRequest {
    #[serde(flatten)]
    pub page: PageRequest,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMachineUserKeysResponse {
    #[serde(default)]
    pub result: Vec<Key>,
    #[serde(default)]
    pub details: ListDetails,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMachineUserKeyRequest {
    pub r#type: KeyType,
    /// RFC-3339 timestamp. Zitadel rejects missing or past values.
    pub expiration_date: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddMachineUserKeyResponse {
    pub key_id: String,
    /// Base64 string — the full JSON key material. Present once at
    /// create time; server cannot re-deliver it. Sink layer (Phase D)
    /// is responsible for persisting it atomically.
    pub key_details: String,
    #[serde(default)]
    pub details: ObjectDetails,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveMachineUserKeyResponse {
    #[serde(default)]
    pub details: ObjectDetails,
}

/// `POST /management/v1/users/{user_id}/keys/_search` — ADR-0016 §3 row 11.
pub async fn list_machine_user_keys(
    client: &ZitadelClient,
    user_id: &str,
    req: &ListMachineUserKeysRequest,
) -> Result<ListMachineUserKeysResponse, ZitadelError> {
    let path = format!("/management/v1/users/{user_id}/keys/_search");
    client.send_json(Method::POST, &path, Some(req)).await
}

/// `POST /management/v1/users/{user_id}/keys` — ADR-0016 §3 row 12.
pub async fn add_machine_user_key(
    client: &ZitadelClient,
    user_id: &str,
    req: &AddMachineUserKeyRequest,
) -> Result<AddMachineUserKeyResponse, ZitadelError> {
    let path = format!("/management/v1/users/{user_id}/keys");
    client.send_json(Method::POST, &path, Some(req)).await
}

/// `DELETE /management/v1/users/{user_id}/keys/{key_id}` — ADR-0016 §3 row 13.
pub async fn remove_machine_user_key(
    client: &ZitadelClient,
    user_id: &str,
    key_id: &str,
) -> Result<RemoveMachineUserKeyResponse, ZitadelError> {
    let path = format!("/management/v1/users/{user_id}/keys/{key_id}");
    client
        .send_json::<(), RemoveMachineUserKeyResponse>(Method::DELETE, &path, None)
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
    async fn add_key_returns_key_material() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/users/u-1/keys"))
            .and(header("authorization", "Bearer p"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "keyId": "k-1",
                "keyDetails": "e30=",
                "details": {"sequence": "1"}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let req = AddMachineUserKeyRequest {
            r#type: KeyType::Json,
            expiration_date: "2099-01-01T00:00:00Z".into(),
        };
        let got = add_machine_user_key(&client_for(&server), "u-1", &req).await.unwrap();
        assert_eq!(got.key_id, "k-1");
        assert_eq!(got.key_details, "e30=");
    }

    #[tokio::test]
    async fn remove_key_404_maps_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(mpath("/management/v1/users/u-1/keys/gone"))
            .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"code":5}"#))
            .mount(&server)
            .await;
        let err = remove_machine_user_key(&client_for(&server), "u-1", "gone")
            .await
            .unwrap_err();
        assert!(matches!(err, ZitadelError::NotFound(_)));
    }
}
