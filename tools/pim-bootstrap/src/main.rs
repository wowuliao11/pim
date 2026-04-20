//! `pim-bootstrap` binary entry point.
//!
//! Phase 2 scope: parse CLI, load config, apply CLI overrides, log the plan,
//! exit. The real Zitadel calls are stubbed — full idempotent ensure-ops land
//! in Phase 3. Contract and output semantics are governed by ADR-0005; the
//! three-layer config/secrets model is ADR-0012; dev/prod schema parity is
//! ADR-0013.

use clap::Parser;
use pim_bootstrap::cli::{Cli, Command, EnvFlag};
use pim_bootstrap::config::{BootstrapConfig, Environment, SeedConfig};
use pim_bootstrap::ensure::{run_pipeline, DynEnsureOp, Flags, Mode};
use pim_bootstrap::ops::{
    new_shared_context, ApiAppEnsureOp, ProjectEnsureOp, ProjectRolesEnsureOp, ServiceAccountEnsureOp,
};
use tracing::{info, warn};
use zitadel_rest_client::{AdminCredential, ZitadelClient};

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

    match &cli.command {
        Command::Bootstrap {
            config,
            sync,
            rotate_keys,
            dry_run,
            env,
        } => {
            let mut cfg = BootstrapConfig::load(config)?;
            apply_cli_overrides(&mut cfg, &cli);
            let effective_env = resolve_env(cfg.env, *env);
            info!(
                path = %config.display(),
                env = ?effective_env,
                dry_run = *dry_run,
                sync = *sync,
                rotate_keys = *rotate_keys,
                authority = %cfg.zitadel.authority,
                admin_key_file = ?cfg.zitadel.admin_key_file,
                project = %cfg.project.name,
                api_app = %cfg.api_app.name,
                roles = ?cfg.roles.iter().map(|r| &r.key).collect::<Vec<_>>(),
                "parsed bootstrap config",
            );

            if *dry_run {
                info!("dry-run: running pipeline in Plan mode (no writes)");
            }

            let mode = if *dry_run { Mode::Plan } else { Mode::Apply };
            let flags = Flags {
                sync: *sync,
                rotate_keys: *rotate_keys,
            };

            // Phase C: concrete ensure-ops, wired in the order defined by ADR-0017
            // §Pipeline (project → api-app → service-account → roles). Each op
            // receives a clone of the shared context so later ops can read ids
            // produced by earlier ones (e.g. `project_id`, `sa_user_id`,
            // `jwt_key_blob`).
            let shared_ctx = new_shared_context();
            let ops: Vec<Box<dyn DynEnsureOp>> = vec![
                Box::new(ProjectEnsureOp::new(&cfg.project, shared_ctx.clone())),
                Box::new(ApiAppEnsureOp::new(&cfg.api_app, shared_ctx.clone())),
                Box::new(ServiceAccountEnsureOp::new(&cfg.service_account, shared_ctx.clone())),
                Box::new(ProjectRolesEnsureOp::new(&cfg.roles, shared_ctx.clone())),
            ];

            let client = build_zitadel_client(&cfg)?;
            let result = run_pipeline(&ops, mode, flags, &client).await;
            print_report(&result.report);
            if let Some(err) = result.error {
                return Err(err.into());
            }
        }

        Command::Seed { config, dry_run, env } => {
            if *env != EnvFlag::Dev {
                anyhow::bail!("seed subcommand is dev-only; refusing env={:?}", env);
            }
            let cfg = SeedConfig::load(config)?;
            if cfg.env != Environment::Dev {
                anyhow::bail!(
                    "seed config {} declares env={:?}; seed is dev-only",
                    config.display(),
                    cfg.env,
                );
            }
            info!(
                path = %config.display(),
                dry_run = *dry_run,
                users = cfg.users.len(),
                role_assignments = cfg.role_assignments.len(),
                "parsed seed config",
            );

            if *dry_run {
                info!("dry-run: no Zitadel calls made");
                return Ok(());
            }

            warn!("seed apply is not implemented yet (Phase 3)");
        }

        Command::Diff { config } => {
            let mut cfg = BootstrapConfig::load(config)?;
            apply_cli_overrides(&mut cfg, &cli);
            info!(
                path = %config.display(),
                authority = %cfg.zitadel.authority,
                project = %cfg.project.name,
                "diff is read-only; Phase 3 will report drift",
            );
        }
    }

    Ok(())
}

/// Apply global CLI overrides (`--zitadel-url`, `--admin-key-file`) onto the
/// loaded config. CLI wins over the file, matching the precedence declared in
/// `cli.rs` doc comments and ADR-0012 (three-layer config).
fn apply_cli_overrides(cfg: &mut BootstrapConfig, cli: &Cli) {
    if let Some(url) = &cli.zitadel_url {
        cfg.zitadel.authority = url.clone();
    }
    if let Some(key) = &cli.admin_key_file {
        cfg.zitadel.admin_key_file = Some(key.clone());
    }
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

fn build_zitadel_client(cfg: &BootstrapConfig) -> anyhow::Result<ZitadelClient> {
    use pim_bootstrap::config::AdminAuthMode;

    let credential = match cfg.zitadel.admin_auth {
        AdminAuthMode::Pat => {
            let env_var = cfg
                .zitadel
                .admin_pat_env_var
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("admin_auth = \"pat\" requires admin_pat_env_var in config"))?;
            AdminCredential::from_env_pat(env_var)?
        }
        AdminAuthMode::JwtProfile => {
            let path = cfg
                .zitadel
                .admin_key_file
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("admin_auth = \"jwt_profile\" requires admin_key_file in config"))?;
            AdminCredential::from_jwt_key_path(path, &cfg.zitadel.authority)?
        }
    };

    Ok(ZitadelClient::new(&cfg.zitadel.authority, credential)?)
}

fn print_report(report: &pim_bootstrap::ensure::PipelineReport) {
    info!(
        rows = report.rows.len(),
        drift_detected = report.drift_detected,
        "pipeline complete",
    );
    for row in &report.rows {
        info!(
            op = row.op_name,
            state = %row.state,
            outcome = ?row.outcome,
            elapsed_ms = row.duration.as_millis() as u64,
            "ensure-op result",
        );
    }
}
