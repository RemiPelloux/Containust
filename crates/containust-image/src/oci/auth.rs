//! Registry authentication: bearer-token exchange and credential lookup.
//!
//! Credentials come from `CONTAINUST_REGISTRY_TOKEN` (used as a bearer
//! token directly), `CONTAINUST_REGISTRY_USER` / `CONTAINUST_REGISTRY_PASSWORD`,
//! or the Docker CLI config (`~/.docker/config.json`). Secrets are never
//! logged and never persisted by Containust.

use std::path::PathBuf;

use base64::Engine as _;
use containust_common::error::{ContainustError, Result};
use serde::Deserialize;

/// Docker Hub registries store credentials under this legacy key.
const DOCKER_HUB_CONFIG_KEY: &str = "https://index.docker.io/v1/";

/// A parsed `WWW-Authenticate: Bearer` challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BearerChallenge {
    /// Token endpoint URL.
    pub realm: String,
    /// Service parameter forwarded to the token endpoint.
    pub service: Option<String>,
}

/// Parses a `WWW-Authenticate` header into a bearer challenge.
#[must_use]
pub fn parse_bearer_challenge(header: &str) -> Option<BearerChallenge> {
    let parameters = header.trim().strip_prefix("Bearer ")?;
    let mut realm = None;
    let mut service = None;
    for part in parameters.split(',') {
        let (key, value) = part.trim().split_once('=')?;
        let value = value.trim_matches('"').to_string();
        match key.trim() {
            "realm" => realm = Some(value),
            "service" => service = Some(value),
            _ => {}
        }
    }
    Some(BearerChallenge {
        realm: realm?,
        service,
    })
}

/// Username/password pair for the token endpoint.
#[derive(Clone)]
pub struct BasicCredentials {
    /// Registry account name.
    pub username: String,
    /// Registry password or personal access token.
    pub password: String,
}

impl std::fmt::Debug for BasicCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never expose the password, even in debug output.
        f.debug_struct("BasicCredentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Returns a bearer token supplied directly via the environment.
#[must_use]
pub fn env_bearer_token() -> Option<String> {
    std::env::var("CONTAINUST_REGISTRY_TOKEN")
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

/// Resolves basic credentials for `registry` from the environment or
/// the Docker CLI config file.
#[must_use]
pub fn basic_credentials(registry: &str) -> Option<BasicCredentials> {
    if let (Ok(username), Ok(password)) = (
        std::env::var("CONTAINUST_REGISTRY_USER"),
        std::env::var("CONTAINUST_REGISTRY_PASSWORD"),
    ) {
        return Some(BasicCredentials { username, password });
    }
    docker_config_credentials(registry, &docker_config_path()?)
}

fn docker_config_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("DOCKER_CONFIG") {
        return Some(PathBuf::from(dir).join("config.json"));
    }
    std::env::home_dir().map(|home| home.join(".docker").join("config.json"))
}

#[derive(Deserialize)]
struct DockerConfig {
    #[serde(default)]
    auths: std::collections::HashMap<String, DockerAuth>,
}

#[derive(Deserialize)]
struct DockerAuth {
    #[serde(default)]
    auth: Option<String>,
}

fn docker_config_credentials(
    registry: &str,
    config_path: &std::path::Path,
) -> Option<BasicCredentials> {
    let content = std::fs::read_to_string(config_path).ok()?;
    let config: DockerConfig = serde_json::from_str(&content).ok()?;
    let keys = [
        registry.to_string(),
        format!("https://{registry}"),
        docker_hub_alias(registry),
    ];
    let encoded = keys
        .iter()
        .find_map(|key| config.auths.get(key))
        .and_then(|auth| auth.auth.as_deref())?;
    decode_basic_auth(encoded)
}

/// Docker Hub credentials live under the legacy index key.
fn docker_hub_alias(registry: &str) -> String {
    if registry == "registry-1.docker.io" || registry == "docker.io" {
        DOCKER_HUB_CONFIG_KEY.to_string()
    } else {
        // Fall back to the registry itself so the array stays valid.
        registry.to_string()
    }
}

fn decode_basic_auth(encoded: &str) -> Option<BasicCredentials> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let (username, password) = text.split_once(':')?;
    Some(BasicCredentials {
        username: username.to_string(),
        password: password.to_string(),
    })
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
}

/// Exchanges a bearer challenge for a token scoped to pull `repository`.
///
/// # Errors
///
/// Returns an error when the token endpoint is unreachable, rejects
/// the credentials, or returns a body without a token.
pub fn fetch_bearer_token(
    client: &reqwest::blocking::Client,
    challenge: &BearerChallenge,
    repository: &str,
    credentials: Option<&BasicCredentials>,
) -> Result<String> {
    let network_error = |message: String| ContainustError::Network {
        url: challenge.realm.clone(),
        message,
    };
    let mut request = client
        .get(&challenge.realm)
        .query(&[("scope", format!("repository:{repository}:pull").as_str())]);
    if let Some(service) = &challenge.service {
        request = request.query(&[("service", service.as_str())]);
    }
    if let Some(credentials) = credentials {
        request = request.basic_auth(&credentials.username, Some(&credentials.password));
    }
    let response = request
        .send()
        .map_err(|error| network_error(format!("token request failed: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(network_error(format!(
            "token endpoint returned status {status}; check registry credentials"
        )));
    }
    let text = response
        .text()
        .map_err(|error| network_error(format!("failed to read token response: {error}")))?;
    let body: TokenResponse = serde_json::from_str(&text)
        .map_err(|error| network_error(format!("invalid token response: {error}")))?;
    body.token
        .or(body.access_token)
        .filter(|token| !token.is_empty())
        .ok_or_else(|| network_error("token endpoint returned no token".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bearer_challenge_extracts_realm_and_service() {
        let challenge = parse_bearer_challenge(
            "Bearer realm=\"https://auth.docker.io/token\",service=\"registry.docker.io\"",
        )
        .expect("parse");
        assert_eq!(challenge.realm, "https://auth.docker.io/token");
        assert_eq!(challenge.service.as_deref(), Some("registry.docker.io"));
    }

    #[test]
    fn parse_bearer_challenge_without_bearer_prefix_is_none() {
        assert!(parse_bearer_challenge("Basic realm=\"x\"").is_none());
    }

    #[test]
    fn parse_bearer_challenge_without_realm_is_none() {
        assert!(parse_bearer_challenge("Bearer service=\"x\"").is_none());
    }

    #[test]
    fn decode_basic_auth_splits_user_and_password() {
        let encoded = base64::engine::general_purpose::STANDARD.encode("user:pa:ss");
        let credentials = decode_basic_auth(&encoded).expect("decode");
        assert_eq!(credentials.username, "user");
        assert_eq!(credentials.password, "pa:ss");
    }

    #[test]
    fn decode_basic_auth_rejects_invalid_base64() {
        assert!(decode_basic_auth("!!!not-base64!!!").is_none());
    }

    #[test]
    fn docker_config_credentials_reads_hub_alias() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("config.json");
        let encoded = base64::engine::general_purpose::STANDARD.encode("hubuser:hubpass");
        std::fs::write(
            &config,
            format!(r#"{{"auths":{{"https://index.docker.io/v1/":{{"auth":"{encoded}"}}}}}}"#),
        )
        .expect("write config");

        let credentials =
            docker_config_credentials("registry-1.docker.io", &config).expect("credentials");
        assert_eq!(credentials.username, "hubuser");
        assert_eq!(credentials.password, "hubpass");
    }

    #[test]
    fn docker_config_credentials_missing_registry_is_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("config.json");
        std::fs::write(&config, r#"{"auths":{}}"#).expect("write config");
        assert!(docker_config_credentials("ghcr.io", &config).is_none());
    }

    #[test]
    fn basic_credentials_debug_redacts_password() {
        let credentials = BasicCredentials {
            username: "user".into(),
            password: "secret".into(),
        };
        let debug = format!("{credentials:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("<redacted>"));
    }
}
