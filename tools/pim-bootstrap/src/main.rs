//! `pim-bootstrap` binary entry point.
//!
//! Phase 2 Task 2.1 scope: parse CLI, load config, log the plan, exit. The
//! real Zitadel calls are stubbed — full idempotent ensure-ops land in
//! Phase 3 (see `plans/006-dev-bootstrap.md`).

use clap::Parser;
use pim_bootstrap::cli::{Cli, Command, EnvFlag};
use pim_bootstrap::config::{BootstrapConfig, Environment, SeedConfig};
use tracing::{info, warn};

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("PIM_BOOTSTRAP_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .with_target(false)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();

    match cli.command {
        Command::Bootstrap {
            config,
            sync,
            rotate_keys,
            dry_run,
            env,
        } => {
            let cfg = BootstrapConfig::load(&config)?;
            let effective_env = resolve_env(cfg.env, env);
            info!(
                path = %config.display(),
                env = ?effective_env,
                dry_run,
                sync,
                rotate_keys,
                authority = %cli.zitadel_url.as_deref().unwrap_or(&cfg.zitadel.authority),
                project = %cfg.project.name,
                api_app = %cfg.api_app.name,
                roles = ?cfg.roles.iter().map(|r| &r.key).collect::<Vec<_>>(),
                "parsed bootstrap config",
            );

            if dry_run {
                info!("dry-run: no Zitadel calls made");
                return Ok(());
            }

            warn!("bootstrap apply is not implemented yet (Phase 3)");
        }

        Command::Seed { config, dry_run, env } => {
            if env != EnvFlag::Dev {
                anyhow::bail!("seed subcommand is dev-only; refusing env={:?}", env);
            }
            let cfg = SeedConfig::load(&config)?;
            if cfg.env != Environment::Dev {
                anyhow::bail!(
                    "seed config {} declares env={:?}; seed is dev-only",
                    config.display(),
                    cfg.env,
                );
            }
            info!(
                path = %config.display(),
                dry_run,
                users = cfg.users.len(),
                role_assignments = cfg.role_assignments.len(),
                "parsed seed config",
            );

            if dry_run {
                info!("dry-run: no Zitadel calls made");
                return Ok(());
            }

            warn!("seed apply is not implemented yet (Phase 3)");
        }

        Command::Diff { config } => {
            let cfg = BootstrapConfig::load(&config)?;
            info!(
                path = %config.display(),
                project = %cfg.project.name,
                "diff is read-only; Phase 3 will report drift",
            );
        }
    }

    Ok(())
}

/// Decide which `Environment` to operate against when the CLI passes an
/// `--env` override. The CLI value wins when set.
fn resolve_env(config_env: Environment, cli_env: Option<EnvFlag>) -> Environment {
    match cli_env {
        Some(EnvFlag::Dev) => Environment::Dev,
        Some(EnvFlag::Prod) => Environment::Prod,
        None => config_env,
    }
}
