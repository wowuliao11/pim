# ADR-0010: Use `tracing` + `thiserror`/`anyhow` for structured logging and typed errors

- **Status:** Accepted
- **Date:** 2026-02 (foundation landed before metrics integration)
- **Deciders:** PIM maintainers

## Context

Every service needs to answer two questions under load:

1. *What happened just now?* — the logging question.
2. *Why did this request fail?* — the error-handling question.

Getting either one wrong produces either unreadable log noise,
unactionable error messages, or log output that leaks secrets.

We needed to commit to a single answer for both, **before** adding
metrics (ADR-0011) or any observability tooling, because logs and
errors underlie everything else. We also needed rules that reject
certain patterns outright — string-typed errors, request bodies in
logs, `println!` in production paths — because those patterns are easy
to add and painful to retrofit out.

## Decision

**Logging: `tracing` is the only logging abstraction.**

- Initialise once per service via `infra_telemetry::init_tracing()`
  (reads `RUST_LOG`, falls back to a per-service default filter).
- Use `tracing` macros (`info!`, `warn!`, `error!`) with structured
  fields, not formatted strings.
- Use spans for request/task context; fields for metadata.
- Allowed crates: `tracing`, `tracing-subscriber`,
  optionally `tracing-error`.
- Forbidden: `println!`, `dbg!`, `log`+`env_logger` (unless bridged),
  free-form string-concatenation logs.

**Logs are an index, not an archive.** Required per-request fields:
`method`, `path` (no query string), `status`, `latency_ms`, plus
`request_id`/`trace_id` when present. Business identifiers (`id`,
`tenant_id`) are allowed when needed. **Explicitly forbidden in logs:**
request bodies, response bodies, headers (except a tiny whitelist —
prefer none), tokens, passwords, PII.

**Errors: `thiserror` for domain errors, `anyhow` at boundaries.**

- Domain errors are typed `enum`s deriving `thiserror::Error`. They
  encode business meaning and failure category. They are not strings.
- Handler-boundary code (HTTP handlers, background jobs) may use
  `anyhow::Result` to collect heterogeneous errors.
- `String` / `&str` as an error type is forbidden.
- Errors are logged **exactly once** — at the request boundary or task
  boundary. No inner-layer logging of the same error as it propagates.
- HTTP status mapping lives in the HTTP layer only. Inner layers do
  not decide status codes.
- External error messages are safe (no internal details, no stack
  traces, no DB error text).

```rust
<!-- sketch -->
// Domain: thiserror
#[derive(Debug, thiserror::Error)]
pub enum UserError {
    #[error("user not found: {0}")]
    NotFound(String),
    #[error("upstream user-service failed")]
    Upstream(#[source] tonic::Status),
}

// HTTP boundary: map once, log once, return
impl actix_web::ResponseError for AppError {
    fn status_code(&self) -> StatusCode { /* explicit match */ }
    fn error_response(&self) -> HttpResponse {
        tracing::warn!(error = %self, kind = self.kind(), "request failed");
        HttpResponse::build(self.status_code()).json(/* safe body */)
    }
}
```

**No future-proofing for observability.** At the time this ADR was
accepted, no metrics / tracing exporter / OTel code existed in the
repo. Adding any of those is a separate decision (see ADR-0011 for
metrics). This ADR does not reserve code paths, feature flags, or
abstraction layers for them.

## Consequences

**Positive:**

- One logging API across the workspace. Grep `tracing::` and you find
  every log site.
- Structured logs are field-stable: the HTTP middleware emits the same
  field set for every request, regardless of handler.
- Domain errors carry enough information that handler-layer mapping is
  a pure function (`AppError -> (Status, JSON body)`).
- The "log each error exactly once" rule prevents the common pattern
  where a single failing request produces 4-5 log lines at different
  layers.
- `thiserror` at domain + `anyhow` at boundary is the Rust community's
  dominant convention; new contributors recognise it.

**Negative / accepted trade-offs:**

- Every new error variant needs a `thiserror` derivation. For quick
  prototypes this feels like overhead. We accept it — the alternative
  (string errors that accumulate over time) is worse.
- The "no request/response bodies in logs" rule means debugging a
  malformed-body report requires either a dev-time body-logging
  middleware (disabled in production) or reproducing the request.
  We prefer this to accidentally logging PII.
- Structured JSON log output is available via the
  `tracing-subscriber` `json` feature but not on by default. Services
  pick the format at init time; in production we expect JSON, but the
  default is human-readable for dev. A later ADR may flip this.
- No metrics / traces means no RED latency data from logs alone. That
  is deliberate — metrics are a separate concern handled by
  ADR-0011.

**Locked in:**

- `infra_telemetry::init_tracing()` is the only tracing init path.
- `thiserror` for domain, `anyhow` for boundaries. Do not add a third
  error-handling crate (e.g. `eyre`, `snafu`) without an ADR.
- The forbidden-in-logs list (bodies, headers, tokens, PII) is
  non-negotiable. New log sites must not add fields from this list.
- HTTP status mapping is centralised in
  `apps/api-gateway/src/errors/`. Inner crates do not return
  `HttpResponse` or `StatusCode`.

## Alternatives considered

### Option A — `log` + `env_logger`

Rejected. `log` is the older facade and lacks first-class structured
fields and spans. `tracing` is a superset — it can bridge from `log`
when we must (e.g. third-party crate emits via `log`) — so picking
`tracing` is strictly more capable.

### Option B — `slog`

Rejected. `slog` is structured-logging-first but less widely adopted
in the Rust async ecosystem today. `tracing` integrates directly with
tokio, tonic, actix, and Zitadel's crate. The tooling alignment is the
deciding factor.

### Option C — Stringly-typed errors with `anyhow` everywhere

Rejected. `anyhow` is excellent at boundaries but erases type
information. Using it inside domain modules means handlers cannot
pattern-match on failure kind, which forces either string matching on
error messages (fragile) or re-classifying errors at every boundary
(boilerplate). `thiserror` at the domain layer preserves the
distinction.

### Option D — Log request/response bodies by default (opt-out)

Rejected. Opt-out logging of bodies is how PII leaks happen. The
default must be "do not log bodies"; exposing bodies is a deliberate
choice per site, not a framework default.

### Option E — Introduce OTel / Jaeger now

Rejected for this ADR. Metrics and distributed tracing are separate
decisions with their own cost/benefit. Lumping them into the logger
ADR would front-load complexity and couple unrelated choices. See
ADR-0011 for the metrics decision; a distributed-tracing ADR is
deferred until there is a concrete need.

## Implementation notes

- Tracing init: `libs/infra-telemetry/src/lib.rs:15-20`
  (`init_tracing` function, used by every `apps/*/src/main.rs`).
- Workspace dependency declarations:
  `Cargo.toml:37-42` (`thiserror = "2.0.17"`, `anyhow = "1"`,
  `tracing = "0.1"`, `tracing-subscriber` with `env-filter` + `json`
  features).
- Gateway error hierarchy: `apps/api-gateway/src/errors/` —
  `app_error.rs` (top-level enum), `error_response.rs`
  (`ResponseError` impl and status-code mapping), `user_error.rs`
  (domain-specific variants).
- Per-request middleware (method/path/status/latency fields):
  `apps/api-gateway/src/middlewares/` (RequestId + RequestLogging
  middlewares).

## References

- Source code: paths above.
- External: [`tracing` crate](https://docs.rs/tracing),
  [`tracing-subscriber`](https://docs.rs/tracing-subscriber),
  [`thiserror`](https://docs.rs/thiserror),
  [`anyhow`](https://docs.rs/anyhow),
  [OWASP Logging Cheat
  Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html)
  (informs the "no bodies / no tokens / no PII" rule).
- Originated from:
  `plans/enterprise-logger-error.plan.md` (principles and scope),
  `plans/001-observability-metrics.md` §4 (tracing crate selection).
- Related: ADR-0011 (metrics, which assumes this logging/error
  baseline).
