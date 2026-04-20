//! Concrete [`EnsureOp`](crate::ensure::EnsureOp) implementations.
//!
//! Registration order in `main.rs` is load-bearing: later ops read values
//! (`project_id`, `api_app_id`, `sa_user_id`, `jwt_key_blob`) that earlier
//! ops stash into [`PipelineContext`]. The documented order is:
//!
//! 1. [`ProjectEnsureOp`]           — produces `project_id`
//! 2. [`ApiAppEnsureOp`]            — consumes `project_id`, produces `api_app_id`
//! 3. [`ServiceAccountEnsureOp`]    — produces `sa_user_id` + `jwt_key_blob`
//! 4. [`ProjectRolesEnsureOp`]      — consumes `project_id`
//!
//! Phase E adds human-user and user-grant ops; neither exists yet.

pub mod api_app;
pub mod context;
pub mod project;
pub mod project_roles;
pub mod service_account;

pub use api_app::ApiAppEnsureOp;
pub use context::{new_shared_context, PipelineContext, SharedContext};
pub use project::ProjectEnsureOp;
pub use project_roles::ProjectRolesEnsureOp;
pub use service_account::ServiceAccountEnsureOp;
