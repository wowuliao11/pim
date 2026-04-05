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

/// User service proto definitions
pub mod user {
    pub mod v1 {
        tonic::include_proto!("user.v1");
    }
}
