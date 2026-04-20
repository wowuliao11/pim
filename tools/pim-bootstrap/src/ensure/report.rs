//! `EnsureOutcome` and `PipelineReport` — the values the driver accumulates
//! as it runs ensure-ops.
//!
//! Shape governed by ADR-0017 §Output. `PipelineReport` is both the data the
//! CLI prints to stdout and the value Phase F's `diff` subcommand inspects
//! to decide its exit code.

use std::time::Duration;

use super::state::EnsureState;

/// Result of running a single ensure-op end-to-end (observe → classify → act).
///
/// Variants correspond to the rows in ADR-0017 §Output table. `Blocked` is
/// distinct from a fatal error: a blocked op produced a deliberate no-op
/// (e.g. `Drift` without `--sync`), whereas a fatal error aborts the
/// pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnsureOutcome {
    /// Resource was created. `id` is the server-assigned identifier.
    Created { id: String },
    /// Resource existed and one or more fields were updated. `fields` lists
    /// the JSON keys that changed (used by `--sync` to produce a human-
    /// readable summary; not load-bearing for correctness).
    Updated { id: String, fields: Vec<String> },
    /// Resource existed and matched the spec; no write was performed.
    NoChange { id: String },
    /// Op was classified in a way that required a write but the required
    /// flag (e.g. `--sync`, `--rotate-keys`) was not passed. Drives the
    /// non-zero exit code of `diff`.
    Blocked { reason: String },
    /// Op produced secret material that was handed to a sink. `count` is the
    /// number of distinct secrets emitted (e.g. PAT + JWT key = 2).
    SecretsEmitted { count: usize },
}

impl EnsureOutcome {
    /// Short human-readable label for the output table.
    pub fn label(&self) -> &'static str {
        match self {
            EnsureOutcome::Created { .. } => "created",
            EnsureOutcome::Updated { .. } => "updated",
            EnsureOutcome::NoChange { .. } => "no-change",
            EnsureOutcome::Blocked { .. } => "blocked",
            EnsureOutcome::SecretsEmitted { .. } => "secrets",
        }
    }
}

/// A single row of the pipeline report. One row per ensure-op attempted by
/// the driver, in registration order.
#[derive(Debug, Clone)]
pub struct PipelineRow {
    pub op_name: String,
    pub state: EnsureState,
    /// `None` in `Mode::Plan`, because `act` was never called.
    pub outcome: Option<EnsureOutcome>,
    pub duration: Duration,
}

/// Aggregated pipeline report returned by the driver.
///
/// Ordering is preserved: `rows[i]` is the i-th op registered. Consumers
/// that need "any drift?" or "any created?" queries should iterate rather
/// than rely on map-style lookup.
#[derive(Debug, Clone, Default)]
pub struct PipelineReport {
    pub rows: Vec<PipelineRow>,
    /// Whether any op returned `EnsureState::Drift` without `--sync` (i.e.
    /// a `Blocked` outcome for a drift case). Used by the `diff`
    /// subcommand's exit code policy.
    pub drift_detected: bool,
}

impl PipelineReport {
    pub fn push(&mut self, row: PipelineRow) {
        if row.state == EnsureState::Drift && matches!(row.outcome, Some(EnsureOutcome::Blocked { .. }) | None) {
            self.drift_detected = true;
        }
        self.rows.push(row);
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
