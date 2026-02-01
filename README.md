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

# Run Auth Service (gRPC on :50051)
cargo run -p auth-service

# Run User Service (gRPC on :50052)
cargo run -p user-service
```

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

# Set JWT secret for auth-service
AUTH_SERVICE__JWT_SECRET=my-secret-key cargo run -p auth-service
```

### Tests

```bash
# Run all tests
cargo test --workspace
```
