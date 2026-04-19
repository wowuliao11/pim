# PIM

A Rust monorepo project.

## Documentation System

This project uses a structured documentation architecture to support long-term development.

- **[AGENTS.md](./AGENTS.md)**: **Start here**. The Project Constitution. Contains process rules, planning requirements, and lifecycle management.
- **[INSTRUCTIONS.md](./INSTRUCTIONS.md)**: Task-level guidance and coding standards.
- **[/docs/design.md](./docs/design.md)**: Current accepted system architecture.
- **[/plans/](./plans/)**: Active development plans and roadmaps.

## Quick Start

### Build

```bash
# Build all crates
cargo build --workspace

# Build specific service
cargo build -p api-gateway
```

### Run Services

```bash
# Run API Gateway (HTTP on :8080)
cargo run -p api-gateway

# Run User Service (gRPC on :50051)
cargo run -p user-service
```

> Host-published ports differ when running via `compose.yml`. See
> [docs/design.md §5](./docs/design.md#5-port-allocation) and
> [ADR-0015](./docs/decisions/0015-allocate-service-ports-with-fixed-policy.md)
> for the full port allocation policy.

### Configuration

Services load configuration from:

1. Environment variables (highest priority)
2. Optional TOML files (`config/*.toml`)
3. Default values (lowest priority)

See [docs/configuration.md](./docs/configuration.md) for detailed usage.

**Quick example:**

```bash
# Override host/port via environment
APP__APP__HOST=0.0.0.0 APP__APP__PORT=3000 cargo run -p api-gateway

# Point the gateway at a non-default user-service endpoint
APP__APP__USER_SERVICE_URL=http://pim-user-service:50051 cargo run -p api-gateway
```

### Tests

```bash
# Run all tests
cargo test --workspace
```
