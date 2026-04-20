//! App endpoints — ADR-0016 §3 table, rows 5–8.
//!
//! Only the `api` app type is wired. OIDC and SAML apps are out of
//! scope for Phase A.2 because no ensure-op creates them. Wire shapes
//! follow `zitadel/management.proto` AppQuery / AddAPIApp.

use reqwest::Method;
use serde::{Deserialize, Serialize};

use crate::client::ZitadelClient;
use crate::error::ZitadelError;
use crate::pagination::PageRequest;
use crate::project::{ListDetails, ObjectDetails};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetAppResponse {
    app: App,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAppsRequest {
    #[serde(flatten)]
    pub page: PageRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queries: Option<Vec<AppQuery>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AppQuery {
    #[serde(rename = "nameQuery")]
    Name {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAppsResponse {
    #[serde(default)]
    pub result: Vec<App>,
    #[serde(default)]
    pub details: ListDetails,
}

/// Values mirror Zitadel proto enum `APIAuthMethodType`.
/// `PRIVATE_KEY_JWT` is what machine clients use with JWT Profile.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApiAuthMethodType {
    #[serde(rename = "API_AUTH_METHOD_TYPE_BASIC")]
    Basic,
    #[serde(rename = "API_AUTH_METHOD_TYPE_PRIVATE_KEY_JWT")]
    PrivateKeyJwt,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiAppRequest {
    pub name: String,
    pub auth_method_type: ApiAuthMethodType,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiAppResponse {
    pub app_id: String,
    #[serde(default)]
    pub details: ObjectDetails,
    /// Present only when `auth_method_type == Basic`.
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppRequest {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppResponse {
    #[serde(default)]
    pub details: ObjectDetails,
}

/// `GET /management/v1/projects/{project_id}/apps/{app_id}` — ADR-0016 §3 row 5.
pub async fn get_app(client: &ZitadelClient, project_id: &str, app_id: &str) -> Result<App, ZitadelError> {
    let path = format!("/management/v1/projects/{project_id}/apps/{app_id}");
    let resp: GetAppResponse = client.send_json::<(), GetAppResponse>(Method::GET, &path, None).await?;
    Ok(resp.app)
}

/// `POST /management/v1/projects/{project_id}/apps/_search` — ADR-0016 §3 row 6.
pub async fn list_apps(
    client: &ZitadelClient,
    project_id: &str,
    req: &ListAppsRequest,
) -> Result<ListAppsResponse, ZitadelError> {
    let path = format!("/management/v1/projects/{project_id}/apps/_search");
    client.send_json(Method::POST, &path, Some(req)).await
}

/// `POST /management/v1/projects/{project_id}/apps/api` — ADR-0016 §3 row 7.
pub async fn create_api_app(
    client: &ZitadelClient,
    project_id: &str,
    req: &CreateApiAppRequest,
) -> Result<CreateApiAppResponse, ZitadelError> {
    let path = format!("/management/v1/projects/{project_id}/apps/api");
    client.send_json(Method::POST, &path, Some(req)).await
}

/// `PUT /management/v1/projects/{project_id}/apps/{app_id}` — ADR-0016 §3 row 8.
pub async fn update_app(
    client: &ZitadelClient,
    project_id: &str,
    app_id: &str,
    req: &UpdateAppRequest,
) -> Result<UpdateAppResponse, ZitadelError> {
    let path = format!("/management/v1/projects/{project_id}/apps/{app_id}");
    client.send_json(Method::PUT, &path, Some(req)).await
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
    async fn create_api_app_happy_path() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(mpath("/management/v1/projects/proj-1/apps/api"))
            .and(header("authorization", "Bearer p"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "appId": "app-1",
                "details": {"sequence": "7"}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let req = CreateApiAppRequest {
            name: "gateway".into(),
            auth_method_type: ApiAuthMethodType::PrivateKeyJwt,
        };
        let got = create_api_app(&client_for(&server), "proj-1", &req).await.unwrap();
        assert_eq!(got.app_id, "app-1");
    }

    #[tokio::test]
    async fn get_app_404_maps_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(mpath("/management/v1/projects/proj-1/apps/gone"))
            .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"code":5}"#))
            .mount(&server)
            .await;
        let err = get_app(&client_for(&server), "proj-1", "gone").await.unwrap_err();
        assert!(matches!(err, ZitadelError::NotFound(_)));
    }
}
