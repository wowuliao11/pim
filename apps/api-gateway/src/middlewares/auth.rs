//! Authentication types for the API Gateway.
//!
//! Token validation is handled by the `zitadel` crate's `IntrospectedUser`
//! extractor (injected directly into handler functions). This module provides
//! a thin `AuthenticatedUser` wrapper that extracts the fields PIM cares about.

use infra_auth::IntrospectedUser;

/// Authenticated user data extracted from Zitadel token introspection.
///
/// Convenience wrapper. Handlers that need full introspection data
/// can use `IntrospectedUser` directly from `infra_auth`.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub roles: Vec<String>,
}

impl From<&IntrospectedUser> for AuthenticatedUser {
    fn from(user: &IntrospectedUser) -> Self {
        // Extract role names from project_roles map
        // Zitadel project_roles: HashMap<role_name, HashMap<org_id, org_name>>
        let roles = user
            .project_roles
            .as_ref()
            .map(|pr| pr.keys().cloned().collect())
            .unwrap_or_default();

        Self {
            user_id: user.user_id.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            roles,
        }
    }
}
