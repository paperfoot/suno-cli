use crate::errors::CliError;
use serde::de::DeserializeOwned;

/// Report the failing operation and schema detail without printing response
/// bodies, which may contain authentication secrets.
pub async fn decode<T: DeserializeOwned>(
    response: reqwest::Response,
    operation: &'static str,
) -> Result<T, CliError> {
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    let bytes = response.bytes().await?;
    decode_bytes(&bytes, status, &content_type, operation)
}

fn decode_bytes<T: DeserializeOwned>(
    bytes: &[u8],
    status: reqwest::StatusCode,
    content_type: &str,
    operation: &'static str,
) -> Result<T, CliError> {
    serde_json::from_slice(bytes).map_err(|error| CliError::Api {
        code: "response_decode_error",
        message: format!(
            "{operation}: could not decode JSON (HTTP {status}, Content-Type {content_type}): {error}"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billing_schema_error_identifies_operation_and_missing_field() {
        let result = decode_bytes::<crate::api::types::BillingInfo>(
            br#"{"subscription_type":false}"#,
            reqwest::StatusCode::OK,
            "application/json",
            "Suno billing",
        );
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Suno billing"));
        assert!(error.contains("total_credits_left"));
        assert!(error.contains("200"));
    }

    #[test]
    fn non_json_auth_response_does_not_expose_body() {
        let result = decode_bytes::<serde_json::Value>(
            b"<html>SECRET_TOKEN_SENTINEL</html>",
            reqwest::StatusCode::OK,
            "text/html",
            "Clerk token",
        );
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Clerk token"));
        assert!(error.contains("text/html"));
        assert!(!error.contains("SECRET_TOKEN_SENTINEL"));
    }
}
