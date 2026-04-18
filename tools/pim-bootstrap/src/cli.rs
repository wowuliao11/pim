//! CLI surface for `pim-bootstrap`.
//!
//! Three subcommands, per plan `006-dev-bootstrap.md` Task 2.1:
//!
//! - `bootstrap` — ensure project/app/roles/service-account exist in Zitadel
//!   and emit the resulting artifacts (IDs, JWT key, PAT) to the configured
//!   output sinks. Idempotent.
//! - `seed` — create human users and role assignments. **Refuses to run
//!   against a prod config** to keep dev-only passwords out of real tenants.
//! - `diff` — read-only drift report between the declarative config and the
//!   live Zitadel tenant.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Top-level `pim-bootstrap` invocation.
#[derive(Debug, Parser)]
#[command(
    name = "pim-bootstrap",
    version,
    about = "Declarative Zitadel bootstrap and dev seed for the PIM workspace",
    propagate_version = true
)]
pub struct Cli {
    /// Zitadel authority URL (e.g. `http://pim.localhost:18080`).
    ///
    /// Overrides the `[zitadel].authority` field in the config file when set.
    /// Primarily useful for CI runs that point at an ephemeral Zitadel.
    #[arg(long, global = true, env = "PIM_BOOTSTRAP_ZITADEL_URL")]
    pub zitadel_url: Option<String>,

    /// Path to a file containing the admin key (PAT string for dev, JWT
    /// profile JSON for prod). Falls back to the credential source declared
    /// in the config file when omitted.
    #[arg(long, global = true, env = "PIM_BOOTSTRAP_ADMIN_KEY_FILE")]
    pub admin_key_file: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

/// Target environment. `dev` is the only value accepted by `seed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum EnvFlag {
    Dev,
    Prod,
}

/// Top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ensure the declarative config is realised in Zitadel.
    Bootstrap {
        /// Path to the bootstrap config TOML (e.g. `bootstrap/dev.toml`).
        #[arg(long)]
        config: PathBuf,

        /// Re-sync attributes of existing objects (names, redirect URIs,
        /// role display names). Without this flag, already-present objects
        /// keep their current state.
        #[arg(long)]
        sync: bool,

        /// Rotate the API-app JWT key and any other rotatable secrets.
        /// Disabled by default so repeated runs stay idempotent.
        #[arg(long)]
        rotate_keys: bool,

        /// Parse the config and log the planned actions without calling
        /// Zitadel.
        #[arg(long)]
        dry_run: bool,

        /// Override the environment marker in the config file. Defaults to
        /// the value declared in the config.
        #[arg(long, value_enum)]
        env: Option<EnvFlag>,
    },

    /// Create dev human users and role assignments. Dev-only.
    Seed {
        /// Path to the seed config TOML (e.g. `bootstrap/seed.dev.toml`).
        #[arg(long)]
        config: PathBuf,

        /// Parse the config and log the planned actions without calling
        /// Zitadel.
        #[arg(long)]
        dry_run: bool,

        /// Environment guard. Only `dev` is accepted; passing `prod` causes
        /// an immediate error. Defaults to `dev`.
        #[arg(long, value_enum, default_value_t = EnvFlag::Dev)]
        env: EnvFlag,
    },

    /// Report drift between the declarative config and the live tenant.
    Diff {
        /// Path to the bootstrap config TOML.
        #[arg(long)]
        config: PathBuf,
    },
}
