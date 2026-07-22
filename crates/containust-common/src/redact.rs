//! Secret detection and redaction for state, logs, and plans.
//!
//! Secret-looking environment values must never be persisted to
//! `state.json` or emitted in debug/plan output. Values are restored
//! at container start from the host environment or `CONTAINUST_SECRET_*`.

/// Marker written to state in place of a secret value.
pub const REDACTED_MARKER: &str = "<redacted>";

const SECRET_KEY_NEEDLES: [&str; 10] = [
    "PASSWORD",
    "SECRET",
    "TOKEN",
    "API_KEY",
    "ACCESS_KEY",
    "PRIVATE_KEY",
    "CREDENTIAL",
    "AUTH_KEY",
    "PASSPHRASE",
    "CLIENT_SECRET",
];

/// Returns true when an environment variable name looks secret-bearing.
#[must_use]
pub fn is_secret_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    SECRET_KEY_NEEDLES
        .iter()
        .any(|needle| upper.contains(needle))
}

/// Redacts secret values in an environment list for persistence/display.
#[must_use]
pub fn redact_env(env: &[(String, String)]) -> Vec<(String, String)> {
    env.iter()
        .map(|(key, value)| {
            if is_secret_key(key) {
                (key.clone(), REDACTED_MARKER.to_string())
            } else {
                (key.clone(), value.clone())
            }
        })
        .collect()
}

/// Resolves redacted markers from the host environment before spawn.
///
/// Lookup order for a redacted key `NAME`:
/// 1. `CONTAINUST_SECRET_NAME`
/// 2. `NAME` in the process environment
///
/// Non-redacted values are returned unchanged. Missing secrets fail closed.
///
/// # Errors
///
/// Returns a configuration error when a redacted key cannot be resolved.
pub fn resolve_env(env: &[(String, String)]) -> Result<Vec<(String, String)>, String> {
    let mut resolved = Vec::with_capacity(env.len());
    for (key, value) in env {
        if value != REDACTED_MARKER {
            resolved.push((key.clone(), value.clone()));
            continue;
        }
        let from_secret = std::env::var(format!("CONTAINUST_SECRET_{key}")).ok();
        let from_host = std::env::var(key).ok();
        let Some(secret) = from_secret.or(from_host) else {
            return Err(format!(
                "secret '{key}' is redacted in state and was not found in \
                 CONTAINUST_SECRET_{key} or the host environment"
            ));
        };
        resolved.push((key.clone(), secret));
    }
    Ok(resolved)
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;

    #[test]
    fn is_secret_key_matches_common_patterns() {
        assert!(is_secret_key("DB_PASSWORD"));
        assert!(is_secret_key("api_token"));
        assert!(is_secret_key("MySecret"));
        assert!(is_secret_key("AWS_ACCESS_KEY_ID"));
        assert!(!is_secret_key("PATH"));
        assert!(!is_secret_key("PORT"));
        assert!(!is_secret_key("LOG_LEVEL"));
    }

    #[test]
    fn redact_env_replaces_only_secret_values() {
        let env = vec![
            ("PATH".into(), "/bin".into()),
            ("DB_PASSWORD".into(), "s3cret".into()),
        ];
        let redacted = redact_env(&env);
        assert_eq!(redacted[0].1, "/bin");
        assert_eq!(redacted[1].1, REDACTED_MARKER);
    }

    #[test]
    fn resolve_env_restores_from_containust_secret_prefix() {
        let key = "CTST_TEST_RESOLVE_SECRET";
        let env_key = format!("CONTAINUST_SECRET_{key}");
        // SAFETY: test-only env mutation scoped to this process.
        unsafe {
            std::env::set_var(&env_key, "from-prefix");
            std::env::remove_var(key);
        }
        let env = vec![(key.to_string(), REDACTED_MARKER.to_string())];
        let resolved = resolve_env(&env).expect("resolve");
        assert_eq!(resolved[0].1, "from-prefix");
        // SAFETY: cleanup of the test-only variable set above.
        unsafe {
            std::env::remove_var(&env_key);
        }
    }

    #[test]
    fn resolve_env_missing_secret_fails_closed() {
        let key = "CTST_TEST_MISSING_SECRET_XYZ";
        // SAFETY: ensure the test key is absent before asserting failure.
        unsafe {
            std::env::remove_var(format!("CONTAINUST_SECRET_{key}"));
            std::env::remove_var(key);
        }
        let env = vec![(key.to_string(), REDACTED_MARKER.to_string())];
        let error = resolve_env(&env).expect_err("must fail");
        assert!(error.contains("not found"));
    }
}
