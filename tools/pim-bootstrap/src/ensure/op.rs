//! `EnsureOp` trait — the unit of work the driver orchestrates.
//!
//! Each op follows a three-phase contract per plan `001-pim-bootstrap-phase3.md`
//! §Phase B and ADR-0017 §State machine:
//!
//! 1. `observe` — read-only probe of the live Zitadel tenant. Returns
//!    `Option<Observed>`: `None` means "not found", which will classify to
//!    [`EnsureState::Missing`].
//! 2. `classify` — pure function mapping `(Desired, Option<Observed>)` to
//!    one of the four [`EnsureState`] values. No I/O.
//! 3. `act` — perform writes appropriate for the classified state and the
//!    enabled flags. Called only by the driver in [`Mode::Apply`].
//!
//! The trait is generic over `Desired` and `Observed` so concrete ops get
//! statically typed specs and responses. The driver erases those types via
//! [`DynEnsureOp`] so a heterogeneous pipeline can be assembled.

use std::time::Instant;

use async_trait::async_trait;
use thiserror::Error;
use zitadel_rest_client::{ZitadelClient, ZitadelError};

use super::report::{EnsureOutcome, PipelineRow};
use super::state::{EnsureState, Flags, Mode};

/// Error returned by an ensure-op's `act` step.
///
/// `Fatal` aborts the pipeline — the driver returns immediately without
/// running further ops. Use it for conflicts and permission errors where
/// proceeding would produce a misleading "success" report.
///
/// `Transport` wraps a [`ZitadelError`] that surfaced during a write and
/// should propagate to the user unchanged; the driver also aborts on this.
/// The distinction matters for failure messages: `Fatal` is the op's own
/// considered decision, `Transport` is an environmental problem.
#[derive(Debug, Error)]
pub enum EnsureError {
    #[error("ensure-op `{op}` aborted: {reason}")]
    Fatal { op: String, reason: String },

    #[error(transparent)]
    Transport(#[from] ZitadelError),
}

/// Contract a concrete ensure-op implements.
///
/// `Desired` is the declarative spec slice (typically borrowed from the
/// loaded [`BootstrapConfig`](crate::config::BootstrapConfig)); `Observed`
/// is the REST response shape the op cares about. Both are op-private.
#[async_trait]
pub trait EnsureOp: Send + Sync {
    type Desired: Send + Sync;
    type Observed: Send + Sync;

    fn name(&self) -> &str;

    fn desired(&self) -> &Self::Desired;

    async fn observe(&self, client: &ZitadelClient) -> Result<Option<Self::Observed>, ZitadelError>;

    fn classify(&self, desired: &Self::Desired, observed: Option<&Self::Observed>) -> EnsureState;

    async fn act(
        &self,
        state: EnsureState,
        flags: Flags,
        observed: Option<Self::Observed>,
        client: &ZitadelClient,
    ) -> Result<EnsureOutcome, EnsureError>;
}

/// Type-erased wrapper so the driver can hold `Vec<Box<dyn DynEnsureOp>>`.
///
/// `EnsureOp` itself is not object-safe because `Desired` and `Observed`
/// are associated types. `DynEnsureOp::step` fuses observe + classify + act
/// behind a uniform signature the driver calls.
#[async_trait]
pub trait DynEnsureOp: Send + Sync {
    fn name(&self) -> &str;

    async fn step(&self, mode: Mode, flags: Flags, client: &ZitadelClient) -> Result<PipelineRow, EnsureError>;
}

#[async_trait]
impl<T> DynEnsureOp for T
where
    T: EnsureOp,
{
    fn name(&self) -> &str {
        EnsureOp::name(self)
    }

    async fn step(&self, mode: Mode, flags: Flags, client: &ZitadelClient) -> Result<PipelineRow, EnsureError> {
        let started = Instant::now();
        let observed = self.observe(client).await?;
        let state = self.classify(self.desired(), observed.as_ref());

        let outcome = match mode {
            Mode::Plan => None,
            Mode::Apply => Some(self.act(state, flags, observed, client).await?),
        };

        Ok(PipelineRow {
            op_name: self.name().to_string(),
            state,
            outcome,
            duration: started.elapsed(),
        })
    }
}
