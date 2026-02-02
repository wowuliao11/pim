// Core telemetry module - requires telemetry_prometheus feature

#[cfg(feature = "telemetry_grpc")]
mod grpc_metrics;

mod labels;

#[cfg(feature = "telemetry_http")]
mod metrics_http;

// Re-export labels (always available when any telemetry feature is enabled)
pub use labels::*;

// Re-export gRPC metrics when feature is enabled
#[cfg(feature = "telemetry_grpc")]
pub use grpc_metrics::*;

// Re-export HTTP metrics when feature is enabled
#[cfg(feature = "telemetry_http")]
pub use metrics_http::*;

#[cfg(feature = "telemetry_prometheus")]
use anyhow::Context;
#[cfg(feature = "telemetry_prometheus")]
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
#[cfg(feature = "telemetry_prometheus")]
use std::sync::OnceLock;

#[cfg(feature = "telemetry_prometheus")]
static PROMETHEUS: OnceLock<PrometheusHandle> = OnceLock::new();

/// Initialize the Prometheus metrics exporter
///
/// This is idempotent - calling it multiple times is safe.
///
/// Requires `telemetry_prometheus` feature.
#[cfg(feature = "telemetry_prometheus")]
pub fn init(service_name: &str) -> anyhow::Result<()> {
    if PROMETHEUS.get().is_some() {
        return Ok(());
    }

    let mut builder = PrometheusBuilder::new();

    builder = builder
        .set_buckets_for_metric(
            Matcher::Full(METRIC_RPC_DURATION_SECONDS.to_string()),
            RPC_DURATION_SECONDS_BUCKETS,
        )
        .context("configure fixed buckets for rpc_duration_seconds")?;

    builder = builder.add_global_label(LABEL_SERVICE, service_name.to_string());

    if let Ok(env) = std::env::var("APP_ENV") {
        builder = builder.add_global_label(LABEL_ENV, env);
    }

    let handle = builder
        .install_recorder()
        .context("install Prometheus metrics recorder")?;

    let _ = PROMETHEUS.set(handle);

    Ok(())
}

/// Render current metrics in Prometheus text exposition format
///
/// Returns empty string if metrics haven't been initialized.
///
/// Requires `telemetry_prometheus` feature.
#[cfg(feature = "telemetry_prometheus")]
pub fn render() -> String {
    PROMETHEUS.get().map(|h| h.render()).unwrap_or_default()
}
