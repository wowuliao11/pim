//! Ensure-op pipeline: the Phase B core of `pim-bootstrap`.
//!
//! Exports:
//! - [`EnsureState`] / [`Flags`] / [`Mode`] — the state-machine vocabulary
//!   (ADR-0017).
//! - [`EnsureOutcome`] / [`PipelineReport`] / [`PipelineRow`] — the data
//!   the driver accumulates and the CLI prints.
//! - [`EnsureOp`] / [`DynEnsureOp`] / [`EnsureError`] — the per-op contract
//!   and type-erased view used by the driver.
//! - [`run_pipeline`] / [`PipelineResult`] — the driver entry point.
//!
//! Concrete ops (project, api-app, service-account, roles, users) live
//! under `crate::ops` in Phase C / E and register via this module.

pub mod driver;
pub mod op;
pub mod report;
pub mod state;

pub use driver::{run_pipeline, PipelineResult};
pub use op::{DynEnsureOp, EnsureError, EnsureOp};
pub use report::{EnsureOutcome, PipelineReport, PipelineRow};
pub use state::{EnsureState, Flags, Mode};

#[cfg(test)]
mod driver_tests;
