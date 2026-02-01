mod grpc_metrics;
mod labels;
mod metrics_http;

pub use grpc_metrics::*;
pub use labels::*;
pub use metrics_http::*;

use anyhow::Context;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;

static PROMETHEUS: OnceLock<PrometheusHandle> = OnceLock::new();

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

pub fn render() -> String {
    PROMETHEUS.get().map(|h| h.render()).unwrap_or_default()
}
