# PIM

A Rust monorepo project.

## Documentation System

This project uses a structured documentation architecture to support long-term development.

- **[AGENTS.md](./AGENTS.md)**: **Start here**. The Project Constitution. Contains process rules, planning requirements, and lifecycle management.
- **[INSTRUCTIONS.md](./INSTRUCTIONS.md)**: Task-level guidance and coding standards.
- **[/docs/design.md](./docs/design.md)**: Current accepted system architecture.
- **[/plans/](./plans/)**: Active development plans and roadmaps.

## Quick Start

```bash
# Build all crates
cargo build

# Run gateway service
cargo run -p gateway

# Run tests
cargo test
```
