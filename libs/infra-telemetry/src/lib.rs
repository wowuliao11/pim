//! Telemetry and metrics utilities for PIM services
//!
//! Provides:
//! - Prometheus metrics initialization and rendering
//! - gRPC metrics middleware (Tower layer)
//! - HTTP metrics endpoint server
//! - Standard metric labels and names

#[cfg(feature = "grpc")]
mod grpc_metrics;

mod labels;

#[cfg(feature = "http")]
mod metrics_http;

// Re-export labels (always available when any telemetry feature is enabled)
pub use labels::*;

// Re-export gRPC metrics when feature is enabled
#[cfg(feature = "grpc")]
pub use grpc_metrics::*;

// Re-export HTTP metrics when feature is enabled
#[cfg(feature = "http")]
pub use metrics_http::*;

#[cfg(feature = "prometheus")]
use anyhow::Context;
#[cfg(feature = "prometheus")]
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
#[cfg(feature = "prometheus")]
use std::sync::OnceLock;

#[cfg(feature = "prometheus")]
static PROMETHEUS: OnceLock<PrometheusHandle> = OnceLock::new();

/// Initialize the Prometheus metrics exporter
///
/// This is idempotent - calling it multiple times is safe.
///
/// Requires `prometheus` feature.
#[cfg(feature = "prometheus")]
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
/// Requires `prometheus` feature.
#[cfg(feature = "prometheus")]
pub fn render() -> String {
    PROMETHEUS.get().map(|h| h.render()).unwrap_or_default()
}
