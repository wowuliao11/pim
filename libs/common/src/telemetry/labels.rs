pub const LABEL_SERVICE: &str = "service";
pub const LABEL_ENV: &str = "env";

pub const LABEL_METHOD: &str = "method";
pub const LABEL_STATUS_CODE: &str = "status_code";
pub const LABEL_ERROR_KIND: &str = "error_kind";

pub const ERROR_KIND_LOGIC: &str = "logic";
pub const ERROR_KIND_SYSTEM: &str = "system";

pub const METRIC_RPC_REQUESTS_TOTAL: &str = "rpc_requests_total";
pub const METRIC_RPC_ERRORS_TOTAL: &str = "rpc_errors_total";
pub const METRIC_RPC_DURATION_SECONDS: &str = "rpc_duration_seconds";

pub const RPC_DURATION_SECONDS_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];
