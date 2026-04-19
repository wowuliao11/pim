//! Pipeline driver: runs a sequence of [`DynEnsureOp`]s against a single
//! [`ZitadelClient`] in registration order.
//!
//! Semantics per ADR-0017 §Implementation notes:
//!
//! - Ops run sequentially. No parallelism — the ops depend on each other's
//!   side effects (project must exist before app, app before roles, etc.).
//! - A fatal error from any op short-circuits the pipeline; rows for
//!   already-completed ops are returned alongside the error so the operator
//!   sees what landed before the failure.
//! - In [`Mode::Plan`], only `observe` + `classify` run. No mutation.

use zitadel_rest_client::ZitadelClient;

use super::op::{DynEnsureOp, EnsureError};
use super::report::PipelineReport;
use super::state::{Flags, Mode};

/// Outcome of running the pipeline.
///
/// On success: a complete [`PipelineReport`] with one row per op.
/// On failure: the partial report (rows for ops that succeeded before the
/// failure) plus the `EnsureError`. The operator still sees what landed.
pub struct PipelineResult {
    pub report: PipelineReport,
    pub error: Option<EnsureError>,
}

impl PipelineResult {
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

/// Run `ops` in order against `client`. Short-circuits on the first
/// [`EnsureError`] and returns the partial report.
pub async fn run_pipeline(
    ops: &[Box<dyn DynEnsureOp>],
    mode: Mode,
    flags: Flags,
    client: &ZitadelClient,
) -> PipelineResult {
    let mut report = PipelineReport::default();
    for op in ops {
        match op.step(mode, flags, client).await {
            Ok(row) => report.push(row),
            Err(err) => {
                return PipelineResult {
                    report,
                    error: Some(err),
                };
            }
        }
    }
    PipelineResult { report, error: None }
}
