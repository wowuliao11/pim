# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/wowuliao11/pim/compare/v0.1.1...v0.1.2) - 2026-04-18

### Added

- add CI/CD infrastructure with Buf integration, cargo-deny, and Docker support
- *(api-gateway)* switch Zitadel introspection auth from Basic to JWT Profile
- replace JWT auth with Zitadel OIDC Token Introspection
- *(infra-auth)* extract shared JWT library and simplify service setup
- *(metrics)* add metrics port configuration and update Prometheus integration
- add taplo formatting configuration and streamline infra-telemetry features
- refactor telemetry and configuration modules into separate libraries
- *(config)* implement shared configuration loading with CommonConfig and TOML support
- *(metrics)* add Prometheus metrics support across services
- restructure monorepo with API gateway and gRPC services

### Fixed

- address code review findings (P1-P4 security, quality, and architecture)

### Other

- release v0.1.1 ([#9](https://github.com/wowuliao11/pim/pull/9))
- add unit tests and update design documentation
- update design, configuration, and config examples for Zitadel auth
- *(dependencies)* remove tracing-actix-web from Cargo.toml

## [0.1.1](https://github.com/wowuliao11/pim/compare/api-gateway-v0.1.0...api-gateway-v0.1.1) - 2026-04-18

### Added

- add CI/CD infrastructure with Buf integration, cargo-deny, and Docker support
- *(api-gateway)* switch Zitadel introspection auth from Basic to JWT Profile
- replace JWT auth with Zitadel OIDC Token Introspection
- *(infra-auth)* extract shared JWT library and simplify service setup
- *(metrics)* add metrics port configuration and update Prometheus integration
- add taplo formatting configuration and streamline infra-telemetry features
- refactor telemetry and configuration modules into separate libraries
- *(config)* implement shared configuration loading with CommonConfig and TOML support
- *(metrics)* add Prometheus metrics support across services
- restructure monorepo with API gateway and gRPC services

### Fixed

- address code review findings (P1-P4 security, quality, and architecture)

### Other

- add unit tests and update design documentation
- update design, configuration, and config examples for Zitadel auth
- *(dependencies)* remove tracing-actix-web from Cargo.toml
