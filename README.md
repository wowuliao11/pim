# PIM

A Rust monorepo project.

## Project Structure

```
pim/
├── crates/
│   └── gateway/     # HTTP API gateway service
├── Cargo.toml       # Workspace configuration
├── INSTRUCTIONS.md  # Development guidelines
└── AGENT.md         # AI agent guidelines
```

## Quick Start

```bash
# Build all crates
cargo build

# Run gateway service
cargo run -p gateway

# Run tests
cargo test
```

## Documentation

- [INSTRUCTIONS.md](./INSTRUCTIONS.md) - Development guidelines
- [AGENT.md](./AGENT.md) - AI agent guidelines
