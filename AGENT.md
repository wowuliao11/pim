# AI Agent Guidelines for PIM Project

## Overview

This document provides guidelines for AI agents working on the PIM project.

## Project Context

- **Language**: Rust
- **Framework**: actix-web for HTTP services
- **Structure**: Cargo workspace monorepo with crates under `crates/`

## Code Generation Rules

### File Structure

1. **Never bloat `main.rs`**

   - `main.rs` should only contain the server startup logic
   - Route configuration goes in `router/`
   - Business logic goes in `handlers/` directory

2. **Enterprise Modular Organization**
   ```
   crates/<crate-name>/src/
   ├── main.rs              # Entry point only (minimal)
   ├── lib.rs               # Library exports
   ├── config/              # Configuration management
   │   ├── mod.rs
   │   ├── app_config.rs    # AppConfig wrapper
   │   ├── env.rs           # Environment loading
   │   └── settings.rs      # Settings structs
   ├── api/                 # API versioning
   │   ├── mod.rs
   │   └── v1/
   │       ├── mod.rs
   │       ├── routes.rs    # Route definitions
   │       ├── dto.rs       # Request/Response DTOs
   │       └── handlers/    # HTTP handlers
   │           ├── mod.rs
   │           ├── auth.rs
   │           └── user.rs
   ├── router/              # Route registration
   │   ├── mod.rs
   │   └── register.rs
   ├── errors/              # Error handling
   │   ├── mod.rs
   │   ├── app_error.rs     # Custom error types
   │   └── error_response.rs # Error response format
   ├── middlewares/         # HTTP middlewares
   │   ├── mod.rs
   │   ├── auth.rs          # JWT authentication
   │   └── request_id.rs    # Request ID tracking
   ├── services/            # Business logic
   │   ├── mod.rs
   │   └── user_service.rs
   └── utils/               # Utility functions
       ├── mod.rs
       └── time.rs
   ```

### Dependency Management

1. **Always use workspace dependencies** when adding new dependencies:

   - First add to root `Cargo.toml` under `[workspace.dependencies]`
   - Then reference with `.workspace = true` in crate's `Cargo.toml`

2. **Example**:

   ```toml
   # Root Cargo.toml
   [workspace.dependencies]
   new-dep = "1.0"

   # Crate Cargo.toml
   [dependencies]
   new-dep.workspace = true
   ```

### Code Style

1. **Error Handling**

   - Use `thiserror` for defining error types
   - Use `anyhow` for application-level error handling
   - Implement `ResponseError` for HTTP error responses
   - Use `ErrorResponse` struct for consistent JSON error format

2. **Async Code**

   - Use `async/await` consistently
   - Use `actix-web` runtime (based on tokio)

3. **Logging**

   - Use `tracing` for structured logging
   - Add appropriate log levels (trace, debug, info, warn, error)
   - Use `tracing-actix-web` for HTTP request logging

4. **Configuration**
   - Use `config` crate for configuration management
   - Support environment variables with `APP_` prefix
   - Provide sensible defaults in Settings structs

### Testing

1. Place unit tests in the same file with `#[cfg(test)]` module
2. Integration tests go in `tests/` directory
3. Use meaningful test names that describe the scenario

## Common Tasks

### Adding JWT Authentication

1. JWT middleware is in `middlewares/auth.rs`
2. Use `JwtAuth::new(secret)` to create middleware
3. Wrap protected routes with the middleware
4. Access authenticated user via request extensions

### Adding a New API Endpoint

1. Create handler in `api/v1/handlers/<feature>.rs`
2. Add DTO structs in `api/v1/dto.rs`
3. Register route in `api/v1/routes.rs`
4. Add service logic in `services/` if needed
5. Write tests

### Adding a New Crate

1. Create directory: `crates/<name>/`
2. Create `Cargo.toml` with workspace inheritance
3. Create `src/` with appropriate structure
4. Crate auto-discovered by workspace

## Do's and Don'ts

### Do

- ✅ Keep files focused and small
- ✅ Use workspace dependencies
- ✅ Follow existing patterns in the codebase
- ✅ Add appropriate error handling with `AppError`
- ✅ Write tests for new functionality
- ✅ Use meaningful names for functions and variables
- ✅ Use DTOs for API request/response
- ✅ Add request ID middleware for tracing
- ✅ Use structured logging with tracing

### Don't

- ❌ Put all code in `main.rs`
- ❌ Add dependencies directly to crate without workspace
- ❌ Ignore compiler warnings
- ❌ Skip error handling
- ❌ Create deeply nested module structures
- ❌ Mix business logic with HTTP handlers
- ❌ Hardcode configuration values
