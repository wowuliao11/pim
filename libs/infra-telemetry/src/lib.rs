//! Telemetry and metrics utilities for PIM services
//!
//! Provides:
//! - Prometheus metrics initialization and rendering
//! - gRPC metrics middleware (Tower layer)
//! - HTTP metrics endpoint server
//! - Standard metric labels and names

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize the tracing subscriber with env-filter and fmt layer.
///
/// Reads `RUST_LOG` env var; falls back to `default_filter` if unset.
/// Call once at service startup before any tracing macros.
pub fn init_tracing(default_filter: impl Into<String>) {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter.into().into()))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[cfg(feature = "grpc")]
mod grpc_metrics;

mod labels;

#[cfg(feature = "http")]
mod metrics_http;

// Re-export labels (always available when any telemetry feature is enabled)
pub use labels::*;

// Re-export the metrics crate so consumers use the same version as the recorder.
// Without this, consumers that depend on a different `metrics` version will silently
// write to a different global recorder and all their metrics will be lost.
#[cfg(feature = "prometheus")]
pub use metrics;

// Re-export gRPC metrics when feature is enabled
#[cfg(feature = "grpc")]
pub use grpc_metrics::*;

// Re-export HTTP metrics when feature is enabled
#[cfg(feature = "http")]
pub use metrics_http::*;

#[cfg(feature = "prometheus")]
use anyhow::Context;
#[cfg(feature = "prometheus")]
pub use metrics_exporter_prometheus::PrometheusHandle;
#[cfg(feature = "prometheus")]
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder};

/// Options for configuring the Prometheus metrics exporter.
///
/// Use the builder methods to customize labels, env, and other settings.
///
/// # Example
///
/// ```no_run
/// use infra_telemetry::PrometheusOptions;
///
/// let options = PrometheusOptions::new("my-service")
///     .env("production")
///     .label("region", "us-east-1");
/// ```
#[cfg(feature = "prometheus")]
pub struct PrometheusOptions {
    pub service_name: String,
    pub env: Option<String>,
    pub global_labels: Vec<(String, String)>,
}

#[cfg(feature = "prometheus")]
impl PrometheusOptions {
    /// Create new options with the given service name.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            env: None,
            global_labels: Vec::new(),
        }
    }

    /// Set the environment label (e.g., "dev", "staging", "prod").
    pub fn env(mut self, env: impl Into<String>) -> Self {
        self.env = Some(env.into());
        self
    }

    /// Add a custom global label to all metrics.
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.global_labels.push((key.into(), value.into()));
        self
    }
}

/// Install the Prometheus metrics recorder and return a handle for rendering.
///
/// The caller owns the returned [`PrometheusHandle`] and is responsible for
/// passing it to [`serve_metrics_http()`] or calling `handle.render()` directly.
///
/// Unlike the previous `init()`, this function does **not** read environment
/// variables — the caller is expected to provide all configuration via
/// [`PrometheusOptions`] (typically sourced from `infra-config`).
///
/// # Errors
///
/// Returns an error if the Prometheus recorder cannot be installed
/// (e.g., if another global recorder is already registered in the process).
///
/// Requires `prometheus` feature.
#[cfg(feature = "prometheus")]
pub fn install_prometheus(options: PrometheusOptions) -> anyhow::Result<PrometheusHandle> {
    let mut builder = PrometheusBuilder::new();

    builder = builder
        .set_buckets_for_metric(
            Matcher::Full(METRIC_RPC_DURATION_SECONDS.to_string()),
            RPC_DURATION_SECONDS_BUCKETS,
        )
        .context("configure fixed buckets for rpc_duration_seconds")?;

    builder = builder.add_global_label(LABEL_SERVICE, options.service_name);

    if let Some(env) = options.env {
        builder = builder.add_global_label(LABEL_ENV, env);
    }

    for (key, value) in options.global_labels {
        builder = builder.add_global_label(key, value);
    }

    let handle = builder
        .install_recorder()
        .context("install Prometheus metrics recorder")?;

    Ok(handle)
}
