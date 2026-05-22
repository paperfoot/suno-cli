use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::errors::CliError;

const CLERK_BASE: &str = "https://auth.suno.com";
const CLERK_JS_VERSION: &str = "5.117.0";
const CLERK_API_VERSION: &str = "2025-11-10";

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct AuthState {
    pub jwt: Option<String>,
    pub cookie: Option<String>,
    pub session_id: Option<String>,
    pub device_id: Option<String>,
    /// The __client cookie from clerk domain — long-lived (~7 days)
    pub clerk_client_cookie: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BrowserAuth {
    pub clerk_client_cookie: String,
    pub cookie_header: String,
    pub device_id: Option<String>,
}

impl AuthState {
    pub fn load() -> Result<Self, CliError> {
        let path = Self::path();
        if !path.exists() {
            return Err(CliError::AuthMissing);
        }
        let data = std::fs::read_to_string(&path)?;
        serde_json::from_str(&data).map_err(|e| CliError::Config(format!("corrupt auth file: {e}")))
    }

    pub fn save(&self) -> Result<(), CliError> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)?;

        // Atomic write: create temp file with restricted permissions, then rename
        let tmp = path.with_extension("json.tmp");

        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .mode(0o600)
                .open(&tmp)?;
            file.write_all(data.as_bytes())?;
            file.sync_all()?;
        }

        #[cfg(not(unix))]
        {
            std::fs::write(&tmp, &data)?;
        }

        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn delete() -> Result<(), CliError> {
        let path = Self::path();
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn is_jwt_expired(&self) -> bool {
        let Some(jwt) = &self.jwt else { return true };
        let parts: Vec<&str> = jwt.split('.').collect();
        if parts.len() != 3 {
            return true;
        }
        let claims = parts[1];
        // JWT claims use Base64URL encoding, not standard Base64
        let Ok(decoded) = BASE64URL.decode(claims) else {
            return true;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&decoded) else {
            return true;
        };
        let Some(exp) = value.get("exp").and_then(|v| v.as_u64()) else {
            return true;
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Refresh aggressively: any JWT with under 30 minutes of life left.
        //
        // Suno issues 1-hour JWTs but their generation endpoint silently
        // rejects tokens older than ~30 minutes with `Token validation
        // failed.` even when the JWT's own `exp` claim says it's still
        // valid (verified 2026-04-07). The 30-minute threshold ensures we
        // always hand the API a freshly-minted JWT.
        now + 1800 >= exp
    }

    fn path() -> PathBuf {
        directories::ProjectDirs::from("com", "suno-cli", "suno-cli")
            .map(|dirs| dirs.config_dir().join("auth.json"))
            .unwrap_or_else(|| PathBuf::from("~/.config/suno-cli/auth.json"))
    }
}

fn strip_cookie_header_prefix(input: &str) -> &str {
    let trimmed = input.trim();
    if trimmed.len() >= "cookie:".len()
        && trimmed[.."cookie:".len()].eq_ignore_ascii_case("cookie:")
    {
        trimmed["cookie:".len()..].trim()
    } else {
        trimmed
    }
}

fn parse_cookie_header(input: &str) -> HashMap<String, String> {
    strip_cookie_header_prefix(input)
        .split(';')
        .filter_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some((name.to_string(), value.trim().to_string()))
        })
        .collect()
}

fn sanitize_device_id(value: &str) -> Option<String> {
    let sanitized = value
        .trim()
        .replace("%22", "\"")
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string();
    if sanitized.is_empty() || sanitized.contains(';') {
        None
    } else {
        Some(sanitized)
    }
}

pub fn normalize_cookie_input(input: &str) -> Result<BrowserAuth, CliError> {
    let normalized = strip_cookie_header_prefix(input);
    let cookies = parse_cookie_header(normalized);

    if let Some(clerk_client_cookie) = cookies.get("__client").filter(|v| !v.is_empty()) {
        let device_id = cookies
            .get("ajs_anonymous_id")
            .and_then(|v| sanitize_device_id(v));
        return Ok(BrowserAuth {
            clerk_client_cookie: clerk_client_cookie.clone(),
            cookie_header: normalized.to_string(),
            device_id,
        });
    }

    if normalized.contains(';') || normalized.contains('=') {
        return Err(CliError::Config(
            "cookie header did not contain a __client field".into(),
        ));
    }

    let clerk_client_cookie = normalized.trim().to_string();
    if clerk_client_cookie.is_empty() {
        return Err(CliError::Config("empty Clerk __client cookie".into()));
    }
    Ok(BrowserAuth {
        cookie_header: format!("__client={clerk_client_cookie}"),
        clerk_client_cookie,
        device_id: None,
    })
}

fn clerk_client_url() -> String {
    format!(
        "{CLERK_BASE}/v1/client?__clerk_api_version={CLERK_API_VERSION}&_clerk_js_version={CLERK_JS_VERSION}"
    )
}

fn clerk_token_url(session_id: &str) -> String {
    format!(
        "{CLERK_BASE}/v1/client/sessions/{session_id}/tokens?__clerk_api_version={CLERK_API_VERSION}&_clerk_js_version={CLERK_JS_VERSION}"
    )
}

fn apply_clerk_headers(
    builder: reqwest::RequestBuilder,
    clerk_cookie: &str,
) -> reqwest::RequestBuilder {
    builder
        .header("authorization", clerk_cookie)
        .header("cookie", format!("__client={clerk_cookie}"))
        .header("origin", "https://suno.com")
        .header("referer", "https://suno.com/")
}

fn response_excerpt(body: &str) -> String {
    const MAX: usize = 500;
    let body = body.replace(['\n', '\r'], " ");
    if body.len() <= MAX {
        body
    } else {
        format!("{}...", body.chars().take(MAX).collect::<String>())
    }
}

fn parse_json_value(body: &str, context: &'static str) -> Result<serde_json::Value, CliError> {
    serde_json::from_str(body).map_err(|e| CliError::Api {
        code: "clerk_json_error",
        message: format!(
            "{context} returned unexpected JSON/body ({e}): {}",
            response_excerpt(body)
        ),
    })
}

/// Generate the dynamic browser-token header value.
pub fn browser_token() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let payload = format!(r#"{{"timestamp":{ms}}}"#);
    let encoded = BASE64.encode(payload.as_bytes());
    format!(r#"{{"token":"{encoded}"}}"#)
}

/// Extract Suno auth cookies from the user's browsers.
/// Tries Chrome, Firefox, Safari, Arc, Brave, Edge in order.
pub fn extract_browser_auth() -> Result<BrowserAuth, CliError> {
    let domains = vec![
        "suno.com".into(),
        "auth.suno.com".into(),
        ".suno.com".into(),
    ];

    for (name, result) in [
        ("Chrome", rookie::chrome(Some(domains.clone()))),
        ("Arc", rookie::arc(Some(domains.clone()))),
        ("Brave", rookie::brave(Some(domains.clone()))),
        ("Firefox", rookie::firefox(Some(domains.clone()))),
        ("Edge", rookie::edge(Some(domains.clone()))),
    ] {
        if let Ok(cookies) = result {
            let mut seen = HashSet::new();
            let mut header_parts = Vec::new();
            let mut clerk_client_cookie: Option<String> = None;
            let mut auth_domain_clerk: Option<String> = None;
            let mut device_id: Option<String> = None;

            for cookie in cookies {
                if !cookie.domain.contains("suno.com") {
                    continue;
                }
                if cookie.name == "__client" && !cookie.value.is_empty() {
                    if cookie.domain.contains("auth.suno.com") {
                        auth_domain_clerk = Some(cookie.value.clone());
                    } else if clerk_client_cookie.is_none() {
                        clerk_client_cookie = Some(cookie.value.clone());
                    }
                }
                if cookie.name == "ajs_anonymous_id" && device_id.is_none() {
                    device_id = sanitize_device_id(&cookie.value);
                }
                let key = (cookie.name.clone(), cookie.domain.clone());
                if seen.insert(key) {
                    header_parts.push(format!("{}={}", cookie.name, cookie.value));
                }
            }

            if let Some(clerk_client_cookie) = auth_domain_clerk.or(clerk_client_cookie) {
                eprintln!("Found Suno session in {name}");
                return Ok(BrowserAuth {
                    clerk_client_cookie,
                    cookie_header: header_parts.join("; "),
                    device_id,
                });
            }
        }
    }

    Err(CliError::Config(
        "No Suno session found in any browser. Log into suno.com first, then retry.".into(),
    ))
}

/// Backwards-compatible helper for callers that only need the Clerk cookie.
#[allow(dead_code)]
pub fn extract_clerk_cookie() -> Result<String, CliError> {
    Ok(extract_browser_auth()?.clerk_client_cookie)
}

/// Exchange the __client cookie for a session ID and JWT via Clerk.
pub async fn clerk_token_exchange(
    client: &reqwest::Client,
    clerk_cookie: &str,
) -> Result<(String, String), CliError> {
    // Step 1: Get session ID
    let resp = apply_clerk_headers(client.get(clerk_client_url()), clerk_cookie)
        .send()
        .await
        .map_err(CliError::Http)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(CliError::Api {
            code: "clerk_exchange_failed",
            message: format!(
                "Clerk token exchange failed ({status}): {}",
                response_excerpt(&body)
            ),
        });
    }

    let raw = resp.text().await.map_err(CliError::Http)?;
    let body = parse_json_value(&raw, "Clerk session lookup")?;
    let session_id = body
        .get("response")
        .and_then(|r| {
            r.get("last_active_session_id")
                .and_then(|s| s.as_str())
                .or_else(|| {
                    r.get("sessions")
                        .and_then(|s| s.as_array())
                        .and_then(|sessions| sessions.first())
                        .and_then(|session| session.get("id"))
                        .and_then(|id| id.as_str())
                })
        })
        .ok_or_else(|| CliError::Api {
            code: "no_session",
            message: "No active session found — log into suno.com in your browser first".into(),
        })?
        .to_string();

    // Step 2: Exchange for JWT
    let jwt = clerk_refresh_jwt(client, clerk_cookie, &session_id).await?;

    Ok((session_id, jwt))
}

/// Refresh JWT using stored Clerk cookie + session ID.
pub async fn clerk_refresh_jwt(
    client: &reqwest::Client,
    clerk_cookie: &str,
    session_id: &str,
) -> Result<String, CliError> {
    let resp = apply_clerk_headers(client.post(clerk_token_url(session_id)), clerk_cookie)
        .header("content-type", "application/x-www-form-urlencoded")
        .send()
        .await
        .map_err(CliError::Http)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(CliError::Api {
            code: "clerk_refresh_failed",
            message: format!(
                "Clerk JWT refresh failed ({status}): {}",
                response_excerpt(&body)
            ),
        });
    }

    let raw = resp.text().await.map_err(CliError::Http)?;
    let body = parse_json_value(&raw, "Clerk JWT refresh")?;
    body.get("jwt")
        .and_then(|j| j.as_str())
        .map(String::from)
        .ok_or_else(|| CliError::Api {
            code: "no_jwt",
            message:
                "Clerk returned no JWT — session may have expired, run `suno auth login` again"
                    .into(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_raw_client_cookie() {
        let auth = normalize_cookie_input("client_token").unwrap();
        assert_eq!(auth.clerk_client_cookie, "client_token");
        assert_eq!(auth.cookie_header, "__client=client_token");
        assert!(auth.device_id.is_none());
    }

    #[test]
    fn normalizes_full_cookie_header_and_device() {
        let auth = normalize_cookie_input(
            "Cookie: foo=bar; __client=client_token; ajs_anonymous_id=%22device-123%22",
        )
        .unwrap();
        assert_eq!(auth.clerk_client_cookie, "client_token");
        assert_eq!(auth.device_id.as_deref(), Some("device-123"));
        assert!(auth.cookie_header.contains("__client=client_token"));
    }

    #[test]
    fn rejects_cookie_header_without_client() {
        let err = normalize_cookie_input("foo=bar; ajs_anonymous_id=device").unwrap_err();
        assert!(err.to_string().contains("__client"));
    }
}
