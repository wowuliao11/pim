//! Shared pipeline context — lets later ensure-ops read IDs that earlier ops
//! stashed (project id → api app op, service-account id → jwt key sink).
//!
//! Kept separate from the [`EnsureOp`](crate::ensure::EnsureOp) trait because
//! the trait contract in ADR-0017 is intentionally stateless. Context is
//! infrastructure, shared via `Arc<Mutex<_>>`; each op holds a clone and
//! reads/writes only the slot it owns.

use std::sync::{Arc, Mutex};

/// Values produced by earlier ops that later ops need.
///
/// Fields populate left-to-right as the pipeline progresses:
/// 1. `ProjectEnsureOp` → `project_id`
/// 2. `ApiAppEnsureOp`  → `api_app_id`
/// 3. `ServiceAccountEnsureOp` → `sa_user_id` and (on create/rotate)
///    `jwt_key_blob` — the base64 JSON key Zitadel returns exactly once.
///
/// Phase D wires `jwt_key_blob` into the sink layer. Phase C only stashes it.
#[derive(Debug, Default)]
pub struct PipelineContext {
    pub project_id: Option<String>,
    pub api_app_id: Option<String>,
    pub sa_user_id: Option<String>,
    /// Base64-encoded JSON key material (`keyDetails` from Zitadel). Only
    /// populated when a new key was created or rotated; otherwise `None`.
    pub jwt_key_blob: Option<String>,
}

/// Shared-ownership handle an ensure-op holds.
pub type SharedContext = Arc<Mutex<PipelineContext>>;

/// Construct a fresh, empty context. Ops clone the `Arc` into their own
/// fields via their constructors.
pub fn new_shared_context() -> SharedContext {
    Arc::new(Mutex::new(PipelineContext::default()))
}
