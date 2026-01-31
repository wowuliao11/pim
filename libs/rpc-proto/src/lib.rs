//! RPC Proto - Generated gRPC interfaces
//!
//! This crate re-exports the gRPC generated code from proto files.
//!
//! IMPORTANT: This crate is a "boundary layer" - it only contains:
//! - Generated gRPC client/server types from proto files
//! - Re-exports of those types
//!
//! FORBIDDEN in this crate:
//! - Business logic
//! - Helpers / mappers / validators
//! - Any non-generated code beyond re-exports

/// Auth service proto definitions
pub mod auth {
    pub mod v1 {
        tonic::include_proto!("auth.v1");
    }
}

/// User service proto definitions
pub mod user {
    pub mod v1 {
        tonic::include_proto!("user.v1");
    }
}
