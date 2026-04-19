# ADR-0011: Expose RED metrics via `metrics` facade + Prometheus pull with label governance

- **Status:** Accepted
- **Date:** 2026-02 (`infra-telemetry` metrics module landed after the
  logging/error baseline in ADR-0010)
- **Deciders:** PIM maintainers

## Context

ADR-0010 committed the workspace to `tracing` + typed errors. That
answers "what happened" and "why did this request fail", but not "how
often" or "how fast". We needed a metrics baseline that:

- Works for both HTTP (api-gateway) and gRPC (user-service) uniformly.
- Keeps business logic free of metric plumbing (AOP, not inline
  counters everywhere).
- Cannot accidentally produce unbounded label cardinality — the
  classic Prometheus failure mode where someone uses `user_id` as a
  label and blows up the TSDB.
- Does not drag in OpenTelemetry, Jaeger, or a push-based exporter
  prematurely.

The RED method (**R**ate, **E**rrors, **D**uration) is the minimum
useful shape: three metrics per service give request rate, error
rate, and latency distribution — enough to answer most production
questions without over-instrumenting.

## Decision

**Facade + exporter:** use the `metrics` crate as the facade in all
code (`metrics::counter!`, `metrics::histogram!`), and
`metrics-exporter-prometheus` as the only exporter. `infra-telemetry`
owns the recorder install; consumers never construct a recorder
directly.

**Transport:** Prometheus **pull** model. Every service exposes
`GET /metrics` returning Prometheus text format. No push gateway, no
OTel collector.

**Standard metrics (RED):** three shared metric names, used by both
HTTP and gRPC middleware:

| Metric                 | Type      | Labels                             |
| ---------------------- | --------- | ---------------------------------- |
| `rpc_requests_total`   | Counter   | `service`, `method`, `status_code` |
| `rpc_errors_total`     | Counter   | `service`, `method`, `error_kind`  |
| `rpc_duration_seconds` | Histogram | `service`, `method`                |

**Label allow-list (design-time enforcement):** only these labels may
ever appear on RED metrics:

- `service` — static per binary, injected globally at recorder install
- `method` — HTTP route template or gRPC full method name (bounded)
- `status_code` — HTTP status or gRPC status (bounded)
- `error_kind` — one of `"logic"` / `"system"` (bounded)
- `env` — static per deployment, injected globally

**Explicitly forbidden as labels:** user IDs, emails, tokens, request
IDs, trace IDs, or any value not drawn from a small fixed set. These
go in logs (ADR-0010), not metrics.

**Enforcement today is by convention, not runtime.** The allow-list
is encoded as `const &str` in
`libs/infra-telemetry/src/labels.rs`; middleware uses these constants.
Code review plus PR-time ADR surfacing (ADR-0002) is how we catch
violations. A runtime allow-list panic is listed as a future option
but intentionally not built — the cost (fail-open vs fail-closed
behaviour, perf overhead) did not justify it at current scale.

**Histogram buckets:** single shared bucket set for
`rpc_duration_seconds`: `[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1,
2.5, 5, 10]` seconds. Per-service bucket overrides are deferred —
they would fragment dashboards that assume shared buckets.

**Service integration:**

- HTTP: an Actix middleware in the gateway emits RED for every HTTP
  request.
- gRPC: a Tower `Layer` from `infra-telemetry`
  (`GrpcMetricsLayer`, enabled via the `grpc` feature) wraps the
  Tonic server and emits RED for every RPC.
- `/metrics` endpoint: HTTP gateway serves it inline; gRPC services
  spawn a small `hyper` HTTP server on a separate
  `*_METRICS_PORT` env var (e.g. `USER_SERVICE_METRICS_PORT`).

```rust
<!-- sketch -->
// main.rs (simplified)
let handle = infra_telemetry::install_prometheus(
    infra_telemetry::PrometheusOptions::new("user-service")
        .env(config.env())
)?;

// gRPC service
Server::builder()
    .layer(GrpcMetricsLayer::new())
    .add_service(UserServiceServer::new(svc))
    .serve(grpc_addr);

// Separate metrics HTTP server on USER_SERVICE_METRICS_PORT
tokio::spawn(serve_metrics_http(metrics_addr, handle));
```

## Consequences

**Positive:**

- One metrics API across HTTP and gRPC. Dashboards query
  `rpc_requests_total` and get data from every service.
- Middleware/layer instrumentation keeps handlers clean. A handler
  does not need to know metrics exist.
- Pull model means services are stateless regarding metrics shipping
  — Prometheus scrapes on its own schedule.
- Design-time allow-list (`LABEL_*` constants + middleware-only label
  construction) makes high-cardinality labels visible in code review.
  You have to write code that looks wrong in order to add one.
- Shared buckets make cross-service latency comparisons meaningful.

**Negative / accepted trade-offs:**

- The allow-list is not runtime-enforced. A determined or careless
  contributor can still call `counter!("custom", "user_id" => id)`
  directly. Mitigated by: `metrics` facade re-exported from
  `infra-telemetry` so `grep metrics::` in PRs catches new call sites;
  ADR-0002 PR-time surfacing of this ADR.
- Pull model requires network reachability from Prometheus to every
  service on its metrics port. Not an issue on Kubernetes; a small
  concern for future edge deployments.
- Separate `*_METRICS_PORT` per gRPC service is more env vars than a
  single shared port. We chose explicitness over magic: a deployer
  sees exactly which port exposes which service's metrics.
- No business-level counters defined in this ADR. Plan 001 Phase 3
  ("at least one domain metric, e.g. `user_registration_total`") is
  not yet shipped. When we add business counters, they must use the
  same allow-list discipline; this ADR governs the rule, not the
  specific counter set.

**Locked in:**

- `metrics` + `metrics-exporter-prometheus` are the only metrics
  crates. Do not add `prometheus` (the direct client), OTel, or
  StatsD without a new ADR.
- Pull model. Push-based transports require a new ADR.
- The four RED label names (`service`, `method`, `status_code`,
  `error_kind`) plus `env` are the entire vocabulary for RED metrics.
  Adding a fifth requires an ADR with a cardinality analysis.
- Re-export of the `metrics` crate from `infra-telemetry`
  (`libs/infra-telemetry/src/lib.rs:36-37`) is load-bearing:
  consumers must use the re-export, not their own `metrics = "..."`
  dependency, or they will register against a different global
  recorder and silently lose data.

## Alternatives considered

### Option A — OpenTelemetry (OTLP) + collector

Rejected at this stage. OTel gives a unified metrics/traces/logs
model, but at the cost of: an always-running collector sidecar, a
config surface roughly 10× larger than Prometheus pull, and a
dependency tree that noticeably slows builds. The benefit (unified
telemetry) only pays off when we have distributed tracing demand,
which we do not. When we do, OTel is the natural upgrade path and
this ADR would be superseded.

### Option B — Direct `prometheus` crate (no facade)

Rejected. The `prometheus` crate is a fine client but couples every
`counter!` call site to Prometheus specifically. The `metrics` facade
lets us swap exporters (StatsD, OTel) without touching business
code. The indirection cost is negligible.

### Option C — Runtime label whitelist (panic/log on unknown label)

Deferred. Benefit: catches allow-list violations at runtime instead of
code review. Cost: every `counter!` call pays a label-lookup
overhead, and we have to decide fail-open (drop metric) vs
fail-closed (panic). The current scale — small team, frequent
reviews, ADR-0002 surfacing — makes the design-time discipline
sufficient. Revisit when team size or contribution velocity makes
review-based enforcement unreliable.

### Option D — Per-service histogram buckets

Deferred. Argument for: a 10s-p99 upper bucket is wasted range for a
sub-millisecond gRPC call. Argument against: every bucket override
fragments dashboards, because a histogram with different buckets
cannot be aggregated naively. Current scale does not justify the
fragmentation cost. If a specific service genuinely needs finer
resolution, override with an ADR explaining why the dashboard cost is
acceptable.

### Option E — Push-based exporter (StatsD, Prometheus push gateway)

Rejected. Both introduce a state-owning middlebox (the push gateway
or StatsD server) and make "did this service emit metric X in the
last minute" a question about the middlebox, not the service. Pull
model colocates the truth with the service.

## Implementation notes

- Facade + re-export:
  `libs/infra-telemetry/src/lib.rs:36-37` (re-exports `metrics` so
  consumers share the recorder).
- Recorder install: `libs/infra-telemetry/src/lib.rs:113-138`
  (`install_prometheus` function, applies global `service` and `env`
  labels + fixed histogram buckets).
- Label constants: `libs/infra-telemetry/src/labels.rs:1-15`
  (`LABEL_*`, `METRIC_*`, `RPC_DURATION_SECONDS_BUCKETS`).
- gRPC middleware: `libs/infra-telemetry/src/grpc_metrics.rs`
  (Tower layer, `grpc` feature).
- HTTP metrics endpoint server:
  `libs/infra-telemetry/src/metrics_http.rs` (`http` feature).
- Workspace dependencies in
  `libs/infra-telemetry/Cargo.toml` (features: `prometheus`, `grpc`,
  `http`).

## References

- Source code: paths above.
- External:
  [`metrics` crate](https://docs.rs/metrics),
  [`metrics-exporter-prometheus`](https://docs.rs/metrics-exporter-prometheus),
  [Prometheus naming best
  practices](https://prometheus.io/docs/practices/naming/),
  [Prometheus on label
  cardinality](https://prometheus.io/docs/practices/instrumentation/#do-not-overuse-labels),
  [Tom Wilkie on the RED
  method](https://www.weave.works/blog/the-red-method-key-metrics-for-microservices-architecture/).
- Originated from: `plans/001-observability-metrics.md` at the
  `infra-telemetry` metrics-integration commits.
- Related: ADR-0010 (logging/error baseline this ADR builds on),
  ADR-0009 (why `libs/infra-telemetry` is the right home for this),
  ADR-0002 (PR-time ADR surfacing — how this ADR stays enforced).
