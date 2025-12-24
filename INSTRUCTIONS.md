# PIM Project Instructions

## Project Overview

PIM is a Rust monorepo project with multiple crates organized under the `crates/` directory.

## Project Structure

```
pim/
├── Cargo.toml              # Workspace root configuration
├── Cargo.lock              # Dependency lockfile
├── README.md               # Project overview
├── INSTRUCTIONS.md         # This file - project instructions
├── AGENT.md                # AI agent guidelines
├── .gitignore              # Git ignore rules
├── crates/                 # All workspace crates
│   └── gateway/            # HTTP API gateway service
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs     # Entry point (minimal, delegates to router)
│           ├── lib.rs      # Library exports
│           ├── config/     # Configuration management
│           │   ├── mod.rs
│           │   ├── app_config.rs
│           │   ├── env.rs
│           │   └── settings.rs
│           ├── api/        # API versioning
│           │   ├── mod.rs
│           │   └── v1/
│           │       ├── mod.rs
│           │       ├── routes.rs
│           │       ├── dto.rs
│           │       └── handlers/
│           │           ├── mod.rs
│           │           ├── auth.rs
│           │           └── user.rs
│           ├── router/     # Route registration
│           │   ├── mod.rs
│           │   └── register.rs
│           ├── errors/     # Error handling
│           │   ├── mod.rs
│           │   ├── app_error.rs
│           │   └── error_response.rs
│           ├── middlewares/ # HTTP middlewares
│           │   ├── mod.rs
│           │   ├── auth.rs
│           │   └── request_id.rs
│           ├── services/   # Business logic
│           │   ├── mod.rs
│           │   └── user_service.rs
│           └── utils/      # Utility functions
│               ├── mod.rs
│               └── time.rs
└── target/                 # Build artifacts (ignored)
```

## Development Guidelines

### Adding a New Crate

1. Create directory under `crates/`:

   ```bash
   mkdir -p crates/new-crate/src
   ```

2. Create `Cargo.toml` with workspace inheritance:

   ```toml
   [package]
   name = "new-crate"
   version.workspace = true
   edition.workspace = true

   [dependencies]
   # Use workspace dependencies
   serde.workspace = true
   ```

3. The crate will be auto-discovered via `members = ["crates/*"]`

### Workspace Dependencies

All shared dependencies should be defined in the root `Cargo.toml` under `[workspace.dependencies]`. Crates reference them with `.workspace = true`.

### Code Style

- Use `cargo fmt` before committing
- Run `cargo clippy` to check for lints
- Run `cargo test` to ensure all tests pass

### Building

```bash
# Build all crates
cargo build

# Build specific crate
cargo build -p gateway

# Run gateway
cargo run -p gateway
```

### Testing

```bash
# Test all crates
cargo test

# Test specific crate
cargo test -p gateway
```

## Architecture Principles

1. **Separation of Concerns**: Keep `main.rs` minimal, delegate routing to `router/`
2. **Modular Handlers**: Group related handlers in subdirectories under `api/v1/handlers/`
3. **Shared Utilities**: Common code should be extracted to shared crates
4. **Workspace Dependencies**: Use workspace-level dependency management for consistency
5. **API Versioning**: Use `api/v1/`, `api/v2/` pattern for API versioning
6. **Error Handling**: Use `AppError` for consistent error responses
7. **Configuration**: Use `config/` module for centralized configuration management
8. **Middleware**: Use `middlewares/` for cross-cutting concerns (auth, logging, request ID)

## API Endpoints

### Health Check

- `GET /health` - Health check endpoint

### Auth (v1)

- `POST /api/v1/auth/login` - User login
- `POST /api/v1/auth/register` - User registration

### Users (v1)

- `GET /api/v1/users` - List all users
- `GET /api/v1/users/{id}` - Get user by ID
- `GET /api/v1/users/me` - Get current authenticated user

## Environment Variables

Configuration can be set via environment variables with `APP_` prefix:

```bash
APP_APP_HOST=0.0.0.0
APP_APP_PORT=8080
APP_DB_URL=postgres://localhost/pim
APP_JWT_SECRET=your-secret-key
APP_JWT_EXPIRATION_HOURS=24
```
