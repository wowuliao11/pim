# Enterprise Logger + Error Foundation Plan

## 1) Overview

### Goal

In the current stage, introduce **only** an enterprise-grade Logger + Error foundation to:

- Improve diagnosability.
- Establish a long-term, evolvable logging and error standard.
- Create a clean conceptual/structural baseline for future observability **without integrating any observability tooling**.

### Explicit Non-Goals (Hard Constraints)

This plan does **not** include, allow, or reserve code for:

- Metrics (Prometheus / StatsD / etc.)
- OpenTelemetry
- Tracing exporter / collector
- Jaeger / Tempo / Zipkin
- Any logging-to-observability bridge code
- Any “future gRPC/distributed system” placeholder integration

The only deliverable is a **correct, restrained, enterprise-grade logging + error infrastructure**.

## 2) Scope

### Included

- Logging framework selection and initialization
- Logging schema (field conventions)
- Error modeling (typed error design)
- Error propagation and logging strategy
- Error → HTTP response mapping (HTTP layer only)
- Rust + Actix best practices for the above

### Excluded

- Performance metrics
- Distributed tracing backends
- Observability platform integration
- Infra deployment
- Log collection/storage pipelines

## 3) Core Principles (Must Be Enforced)

### 3.1 Logging Philosophy

Logs exist to:

- Describe what happened.
- Help debug and triage issues.

Logs do **not** exist to:

- Store request/response bodies.
- Replay requests.
- Carry business payloads.

Logs are an **index**, not an archive.

### 3.2 Error Philosophy

- Errors are part of domain semantics.
- Errors are **not** strings.
- Errors are **not** HTTP responses.
- Errors must be:
  - Propagatable
  - Categorizable
  - Structurally loggable

### 3.3 No Future-Coding Rule (Hard Constraint)

It is forbidden to add any of the following “for future observability”:

- Placeholder code
- Extra abstraction layers
- Feature flags

If code has no explicit purpose **today**, it must not exist.

## 4) Logging Plan

### 4.1 Logging Framework

- Use `tracing` as the only logging abstraction.
- Output must support structured JSON logs.

Allowed crates:

- `tracing`
- `tracing-subscriber`
- `tracing-error`

Forbidden:

- `log` + `env_logger` (unless bridged into tracing)
- `println!` / `dbg!`

### 4.2 Required Logging Fields (HTTP Requests)

Default (every request):

- `request_id` / `trace_id` (only if present)
- HTTP `method`
- HTTP `path` (no query string)
- `status`
- `latency_ms`

Allowed (as needed):

- Business identifiers (e.g. `id`, `order_id`, `tenant_id`)
- Error type and error code

Explicitly forbidden:

- Request body
- Response body
- Headers (except a very small whitelist; prefer none)
- Tokens / passwords / PII
- Unredacted payloads

### 4.3 Log Level Semantics

- **INFO**: request start/end, key business events
- **WARN**: recoverable errors; degraded external dependencies already handled
- **ERROR**: request failed; non-recoverable errors
- **DEBUG/TRACE**: off by default; never relied upon by business logic

### 4.4 Span Usage

- Use spans to represent context.
- Use fields for metadata.
- Do not “build flows” by concatenating log text.

## 5) Error Plan

### 5.1 Error Modeling

- Use `thiserror` to define domain errors.
- Error types must encode:
  - Business meaning
  - Failure reason
- Do not use `String` / `&str` as error types.

### 5.2 Error Propagation

- Inner layers: domain errors.
- Boundary layers (HTTP handlers / jobs): `anyhow` is allowed.
- Errors are logged in one place only (boundary).

Forbidden:

- Swallowing errors.
- Deciding HTTP status codes deep in lower layers.

### 5.3 Error Logging Rules

- Log each error **exactly once**.
- Log at:
  - Request boundary, or
  - Task boundary

Logs must include:

- Error type
- Error code (if any)
- High-level semantic message

### 5.4 Error → HTTP Mapping

- HTTP layer owns the mapping.
- Mapping must be explicit and predictable.
- External error messages must be safe (no internal details).

## 6) Execution Rules

Implementation must be split into clear steps. For each step:

- What was done
- Why it is needed now

Stop and explain if any step requires:

- Metrics
- Tracing backend integration
- Placeholder code for future systems

## 7) Acceptance Criteria

- A single, unified logger initialization exists.
- Logs are structured and field-stable.
- Errors are typed, clear, and propagatable.
- HTTP handlers no longer rely on stringly-typed errors.
- No observability-related placeholder code exists.

## 8) Final Self-Check

Before merging any code, ask:

- Is this code strictly necessary **today**?
- Is it only for a hypothetical future observability integration?

If the answer is not the former, the code must not exist.

## 9) Status Tracking

| Section                     | Status      | Notes                                                   |
| --------------------------- | ----------- | ------------------------------------------------------- |
| Logging framework (§4.1)    | ✅ Complete | `tracing` + `tracing-subscriber` used across all services |
| Logging fields (§4.2)       | ✅ Complete | `RequestId`, `RequestLogging` middlewares in gateway     |
| Error modeling (§5.1)       | ✅ Complete | Typed errors via `thiserror` in gateway (`AppError`, `AuthError`, etc.) |
| Error → HTTP mapping (§5.4) | ✅ Complete | `ResponseError` impl on `AppError` with explicit status codes |
| Error propagation (§5.2)    | ✅ Complete | `anyhow` at boundary, `thiserror` for domain errors     |
| Structured JSON logs (§4.1) | Partial     | JSON feature enabled in `tracing-subscriber` but not activated by default |
