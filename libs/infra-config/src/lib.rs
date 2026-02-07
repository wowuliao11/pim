//! Configuration loading utilities for PIM services
//!
//! Provides a generic configuration loader that merges:
//! 1. Default values from struct's Default impl
//! 2. Optional TOML configuration file
//! 3. Environment variables with configurable prefix
//!
//! Also includes `AppEnv` for runtime environment detection.

mod env;
mod loader;

pub use env::AppEnv;
pub use loader::{load_config, CommonConfig};
