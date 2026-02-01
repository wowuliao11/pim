# Plan: Unified Observability & Metrics

## 1) Context & Goal

Provide production-grade, pull-based metrics across all services in this monorepo.

Core goals:

- Standardize RED metrics (Rate, Errors, Duration) across HTTP + gRPC.
- Keep business logic clean (AOP via middleware/layers).
- Enforce label governance by design (no high-cardinality labels).
- Ensure every service exposes `GET /metrics` in Prometheus text format.

## 2) Decisions (locked for this plan)

1. **Metrics port strategy (gRPC services):** use explicit env var `*_METRICS_PORT`.
   - `AUTH_SERVICE_METRICS_PORT`
   - `USER_SERVICE_METRICS_PORT`
2. **Histogram buckets:** fixed, shared buckets for `rpc_duration_seconds`.
3. **Runtime label validation:** not implemented now; keep as a follow-up option.

## 3) Architecture

| Component       | Selection                        | Notes                                   |
| --------------- | -------------------------------- | --------------------------------------- |
| Facade API      | `metrics`                        | Used everywhere for counters/histograms |
| Exporter        | `metrics-exporter-prometheus`    | Pull model via `/metrics`               |
| Process metrics | `metrics-process`                | Adds process/resource gauges            |
| HTTP middleware | Actix middleware (gateway-local) | Emits RED for HTTP                      |
| gRPC layer      | Tower `Layer` for Tonic          | Emits RED for gRPC                      |

### 3.1 Contract: Standard metrics

| Metric                 | Type      | Labels                             | Description              |
| ---------------------- | --------- | ---------------------------------- | ------------------------ |
| `rpc_requests_total`   | Counter   | `service`, `method`, `status_code` | Total requests by status |
| `rpc_errors_total`     | Counter   | `service`, `method`, `error_kind`  | Classified errors        |
| `rpc_duration_seconds` | Histogram | `service`, `method`                | Latency distribution     |

### 3.2 Label governance (design-time)

Allowed labels only:

- `service` (static per binary)
- `method` (route name / gRPC full method)
- `status_code` (HTTP status code or gRPC status code)
- `error_kind` (`logic` / `system`)

Explicitly forbidden labels: user identifiers, emails, tokens, request IDs, or any unbounded values.

### 3.3 Fixed buckets (shared)

`rpc_duration_seconds` buckets (seconds):

`[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10]`

## 4) Phased implementation

### Phase 1 — Infrastructure (libs/common)

Goal: Provide a single reusable telemetry facade for all apps.

- [ ] Add dependencies to `libs/common/Cargo.toml`:
  - `metrics`
  - `metrics-exporter-prometheus`
  - `metrics-process`
- [ ] Add module `common::telemetry`:
  - Files:
    - `libs/common/src/telemetry/mod.rs`
    - `libs/common/src/telemetry/labels.rs`
  - APIs:
    - `telemetry::init(service_name: &str) -> anyhow::Result<()>`
    - `telemetry::render() -> String` (Prometheus text)
    - `telemetry::fixed_buckets()` (internal)
- [ ] Export from `libs/common/src/lib.rs`.

Acceptance:

- Calling `common::telemetry::init("test")` once installs a global recorder.
- `common::telemetry::render()` returns non-empty Prometheus output.

### Phase 2 — Application integration (apps/\*)

Goal: Expose `/metrics` everywhere with minimal app changes.

#### API Gateway (apps/api-gateway)

- [ ] Call `common::telemetry::init("api-gateway")` at startup.
- [ ] Add/enable HTTP metrics middleware.
- [ ] Expose `GET /metrics` returning `common::telemetry::render()`.

Acceptance:

- `curl -s localhost:8080/metrics | head` returns Prometheus text.

#### gRPC services (apps/auth-service, apps/user-service)

- [ ] Call `common::telemetry::init("auth-service")` / `init("user-service")`.
- [ ] Wrap Tonic server with a `GrpcMetricsLayer`.
- [ ] Spawn an HTTP server for `/metrics` on `*_METRICS_PORT`.

Acceptance:

- `curl -s localhost:$AUTH_SERVICE_METRICS_PORT/metrics | head` returns Prometheus text.
- `curl -s localhost:$USER_SERVICE_METRICS_PORT/metrics | head` returns Prometheus text.

### Phase 3 — Business metrics & validation

Goal: Add 1–2 business counters and verify cardinality safety.

- [ ] Add at least one domain metric in `auth-service` (example: `user_registration_total`).
- [ ] Validate we do not use dynamic IDs as labels.

Acceptance:

- The business counter appears in `/metrics` output.
- No new unbounded labels introduced.

### Phase 4 — Ops (optional)

Goal: Dashboard-as-code (Grafana).

- [ ] Create `ops/monitoring` with Grafonnet templates.

## 5) Follow-up options (not in scope now)

- Runtime label whitelist enforcement (panic/log/deny unknown labels).
- Per-service histogram bucket overrides (only if justified + reviewed).
