//! Infrastructure authentication library — Zitadel OIDC integration
//!
//! Provides re-exports from the `zitadel` crate for actix-web Token Introspection.
//! The API Gateway uses `IntrospectedUser` as an actix extractor to validate
//! Bearer tokens against Zitadel's introspection endpoint.

// Re-export the actix introspection types that consumers need
pub use zitadel::actix::introspection::{IntrospectedUser, IntrospectionConfig, IntrospectionConfigBuilder};
