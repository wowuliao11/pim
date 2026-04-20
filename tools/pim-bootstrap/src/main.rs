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
    new_shared_context, ApiAppEnsureOp, ProjectEnsureOp, ProjectRolesEnsureOp, ServiceAccountEnsureOp, SharedContext,
};
use pim_bootstrap::sinks;
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

            // Phase D: persist pipeline outputs to the configured sinks.
            // Skipped in Plan mode so `--dry-run` stays read-only. On Apply,
            // we write the JWT key (if one was rotated/created), refresh
            // per-service `config.toml`s, and upsert env-file entries.
            if matches!(mode, Mode::Apply) {
                run_sinks(&cfg, &shared_ctx, *rotate_keys, *sync)?;
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

/// Persist pipeline outputs (Phase D). Runs only in Apply mode. Reads the
/// IDs/blob stashed by ensure-ops from `shared_ctx` and dispatches to the
/// three sinks. Empty env-file entries are expected in dev (PATs arrive
/// out-of-band); the call is still made so the sink can short-circuit.
fn run_sinks(cfg: &BootstrapConfig, shared_ctx: &SharedContext, rotate_keys: bool, sync: bool) -> anyhow::Result<()> {
    let (project_id, api_app_id, sa_user_id, jwt_key_blob) = {
        let ctx = shared_ctx
            .lock()
            .map_err(|_| anyhow::anyhow!("pipeline context mutex poisoned"))?;
        (
            ctx.project_id.clone(),
            ctx.api_app_id.clone(),
            ctx.sa_user_id.clone(),
            ctx.jwt_key_blob.clone(),
        )
    };

    let project_id = project_id
        .ok_or_else(|| anyhow::anyhow!("sinks: project_id missing; pipeline did not reach ProjectEnsureOp"))?;
    let api_app_id = api_app_id
        .ok_or_else(|| anyhow::anyhow!("sinks: api_app_id missing; pipeline did not reach ApiAppEnsureOp"))?;
    let sa_user_id = sa_user_id
        .ok_or_else(|| anyhow::anyhow!("sinks: sa_user_id missing; pipeline did not reach ServiceAccountEnsureOp"))?;

    let jwt_outcome = sinks::jwt_key::write(jwt_key_blob.as_deref(), &cfg.outputs.jwt_key_path, rotate_keys)?;
    match &jwt_outcome {
        sinks::jwt_key::JwtKeyOutcome::Written(p) => {
            info!(path = %p.display(), "wrote jwt key");
        }
        sinks::jwt_key::JwtKeyOutcome::Stdout(tag) => {
            info!(tag = %tag, "emitted jwt key to stdout sentinel");
        }
        sinks::jwt_key::JwtKeyOutcome::Skipped => {
            info!("jwt key sink skipped (no blob staged this run)");
        }
    }

    let inputs = sinks::service_config::ServiceConfigInputs {
        authority: &cfg.zitadel.authority,
        project_id: &project_id,
        api_app_id: &api_app_id,
        sa_user_id: &sa_user_id,
        jwt_key_path: &cfg.outputs.jwt_key_path,
    };
    let touched = sinks::service_config::render_all(&cfg.outputs.service_configs, &inputs)?;
    for label in &touched {
        info!(target = %label, "wrote service config");
    }

    // Dev runs emit zero env entries — PATs are sourced out-of-band via
    // `docker compose exec zitadel zitadel-cli ...`. Prod runs will pass
    // real pairs once SECRET-1..N are defined (plan 001 §Phase E/F).
    let env_entries: Vec<(&str, &str)> = Vec::new();
    let env_outcome = sinks::env_file::upsert(&env_entries, &cfg.outputs.env_file_path, sync)?;
    match env_outcome {
        sinks::env_file::EnvFileOutcome::Written { path, changed } => {
            info!(path = %path.display(), changed, "upserted env file");
        }
        sinks::env_file::EnvFileOutcome::Stdout(tag) => {
            info!(tag = %tag, "emitted env entries to stdout sentinel");
        }
        sinks::env_file::EnvFileOutcome::NoEntries => {
            info!("env file sink skipped (no entries to write)");
        }
    }

    Ok(())
}
