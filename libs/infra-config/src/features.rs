//! Runtime feature flags.
//!
//! Feature flags let unfinished work land on `main` without long-lived
//! branches (see `CONTRIBUTING.md` and `docs/design.md` §8.2).
//!
//! ## Contract
//!
//! - A flag is identified by a snake_case name (e.g. `new_user_search`).
//! - It is enabled when the environment variable
//!   `APP_FEATURE_<UPPERCASE_NAME>=true` is set at process start.
//! - The mapping is: lowercase the env value, then accept exactly `"true"`,
//!   `"1"`, or `"yes"` as enabled. Anything else (including unset) is
//!   disabled. This keeps the contract small and unambiguous.
//! - Flags are read fresh on every call. Services that need a stable value
//!   for the lifetime of a request should snapshot it at the entry point.
//!
//! ## Usage
//!
//! ```no_run
//! use infra_config::features;
//!
//! if features::is_enabled("new_user_search") {
//!     // new path
//! } else {
//!     // legacy path
//! }
//! ```
//!
//! ## Lifecycle
//!
//! Each flag is debt. The introducing PR MUST document:
//! - the flag name and the env var that controls it,
//! - the owner,
//! - the criterion for removing the flag (and its dead branch).

use std::env;

/// Environment-variable prefix for feature flags.
pub const ENV_PREFIX: &str = "APP_FEATURE_";

/// Returns `true` if the named feature flag is enabled in the current
/// process environment.
///
/// `name` is a snake_case identifier; it is uppercased and prefixed with
/// `APP_FEATURE_` to form the env var. Accepted truthy values (case-
/// insensitive): `true`, `1`, `yes`. Everything else (including unset) is
/// treated as disabled.
pub fn is_enabled(name: &str) -> bool {
    let var = env_var_for(name);
    match env::var(&var) {
        Ok(raw) => is_truthy(&raw),
        Err(_) => false,
    }
}

/// Returns the environment-variable name that controls the given flag.
///
/// Useful for logging, diagnostics, and tests.
pub fn env_var_for(name: &str) -> String {
    format!("{ENV_PREFIX}{}", name.to_ascii_uppercase())
}

fn is_truthy(raw: &str) -> bool {
    matches!(raw.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests mutate process-wide environment; serialize them with a mutex
    // so they don't race when the test binary runs them in parallel.
    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(key: &str, value: Option<&str>, f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prior = env::var(key).ok();
        match value {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
        f();
        match prior {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }

    #[test]
    fn env_var_name_is_uppercased_and_prefixed() {
        assert_eq!(env_var_for("new_user_search"), "APP_FEATURE_NEW_USER_SEARCH");
    }

    #[test]
    fn unset_flag_is_disabled() {
        with_env("APP_FEATURE_FF_UNSET", None, || {
            assert!(!is_enabled("ff_unset"));
        });
    }

    #[test]
    fn truthy_values_enable_flag() {
        for raw in ["true", "TRUE", " True ", "1", "yes", "YES"] {
            with_env("APP_FEATURE_FF_TRUTHY", Some(raw), || {
                assert!(is_enabled("ff_truthy"), "expected {raw:?} to be truthy");
            });
        }
    }

    #[test]
    fn other_values_disable_flag() {
        for raw in ["false", "0", "no", "", "on", "enabled", "2"] {
            with_env("APP_FEATURE_FF_FALSY", Some(raw), || {
                assert!(!is_enabled("ff_falsy"), "expected {raw:?} to be falsy");
            });
        }
    }
}
