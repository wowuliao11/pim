//! `EnsureState` — the four-state classification that every ensure-op returns
//! from its `classify` step.
//!
//! ADR-0017 §State machine fixes the cardinality at four: `Missing`, `Match`,
//! `Drift`, `Conflict`. Adding a fifth state is a deliberate schema decision
//! and must be preceded by an amending ADR.

use std::fmt;

/// Classification produced by [`EnsureOp::classify`](super::EnsureOp::classify).
///
/// The driver's action matrix (ADR-0017 §Implementation notes) is:
///
/// | State    | no flags | `--sync`         | `--rotate-keys` |
/// |----------|----------|------------------|-----------------|
/// | Missing  | create   | create           | create          |
/// | Match    | no-op    | no-op            | may rotate keys |
/// | Drift    | no-op    | update           | may rotate keys |
/// | Conflict | abort    | abort            | abort           |
///
/// `Conflict` always aborts the pipeline. No flag combination promotes a
/// conflict into an automatic update, because conflicts indicate the
/// operator's declarative intent disagrees with an immutable server-side fact
/// (e.g. an unrelated resource has already claimed the desired name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnsureState {
    /// Desired resource does not exist on the server.
    Missing,
    /// Desired resource exists and matches the declarative spec on every
    /// compared field.
    Match,
    /// Desired resource exists but at least one compared field differs. The
    /// driver only acts on drift when `--sync` is passed.
    Drift,
    /// Desired resource cannot be reconciled automatically — e.g. name
    /// collision with an unrelated object. Pipeline aborts.
    Conflict,
}

impl fmt::Display for EnsureState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnsureState::Missing => f.write_str("missing"),
            EnsureState::Match => f.write_str("match"),
            EnsureState::Drift => f.write_str("drift"),
            EnsureState::Conflict => f.write_str("conflict"),
        }
    }
}

/// Write-enabling flags forwarded from the CLI.
///
/// ADR-0017 §Implementation notes: "`--sync` and `--rotate-keys` are the only
/// write-enabling flags". Any future flag that unlocks a new mutation
/// requires an amending ADR.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags {
    /// Update drifted attributes (names, role display names, redirect URIs,
    /// …) of existing objects. Off by default so repeated runs stay
    /// idempotent.
    pub sync: bool,
    /// Rotate rotatable secrets (API-app JWT key, future PATs). Off by
    /// default because rotation invalidates currently-issued tokens.
    pub rotate_keys: bool,
}

/// Driver execution mode.
///
/// `Plan` runs only `observe` + `classify` and records the resulting state
/// per op without performing any writes. Maps to the `--dry-run` flag on
/// `pim-bootstrap bootstrap`. `Apply` additionally runs `act`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Plan,
    Apply,
}
