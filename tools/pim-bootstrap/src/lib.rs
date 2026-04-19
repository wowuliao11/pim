//! `pim-bootstrap` — declarative Zitadel bootstrap and dev seed CLI.
//!
//! The crate is deliberately split into two layers so operations (bootstrap,
//! seed, diff) are unit-testable without spinning up a real Zitadel:
//!
//! - [`cli`] defines the Clap surface (argument parsing only).
//! - [`config`] defines the TOML schema for `bootstrap/*.toml` and
//!   `seed.dev.toml`.
//!
//! Higher-level orchestration (idempotent ensure-project / ensure-app /
//! ensure-role operations) is layered on top in Phase 3.

pub mod cli;
pub mod config;
pub mod ensure;
