//! Error types for the skatteetaten CLI.

use thiserror::Error;

/// Errors raised by the client and auth layers.
///
/// Application glue (`main`, command handlers) uses [`anyhow::Result`]; this enum
/// is for the cases where callers need to branch on the *kind* of failure (e.g.
/// "session cookie expired, tell the user to log in again" vs "rate limited").
#[derive(Debug, Error)]
pub enum SktError {
    /// No usable session on disk/keychain, the user must run `skt login`.
    #[error("not logged in, run `skt login` first")]
    NotAuthenticated,

    /// The stored Min skatt session expired or was rejected (HTTP 401/403).
    /// Portal sessions are short-lived and cannot be refreshed silently, so the
    /// only fix is to log in again.
    #[error(
        "your Min skatt session has expired (portal sessions are short-lived and \
         cannot be refreshed). Run `skt login` again."
    )]
    SessionExpired,

    /// The API returned HTTP 429. We do not circumvent rate limits; back off.
    #[error("rate limited by Skatteetaten (HTTP 429){}; please wait and try again. This tool does not bypass rate limits", retry_after.map(|s| format!(", retry after {s}s")).unwrap_or_default())]
    RateLimited { retry_after: Option<u64> },

    /// A non-success HTTP response that is not one of the special cases above.
    #[error("Skatteetaten API error (HTTP {status}): {message}")]
    Api { status: u16, message: String },

    /// The interactive BankID / cookie-capture login flow failed.
    #[error("BankID login flow failed: {0}")]
    AuthFlow(String),

    /// Keychain access failed.
    #[error("keychain error: {0}")]
    Keyring(#[from] keyring::Error),

    /// Network/transport error.
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON (de)serialisation error.
    #[error("could not parse response: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SktError>;

/// Map an HTTP status and (optional) body to a typed [`SktError`]. Pure so the
/// classification rules can be unit-tested without a live `reqwest::Response`.
///
/// `retry_after` is the parsed `Retry-After` header value in seconds, if any.
pub fn classify_status(status: u16, retry_after: Option<u64>, body: &str) -> SktError {
    match status {
        401 | 403 => SktError::SessionExpired,
        429 => SktError::RateLimited { retry_after },
        _ => SktError::Api {
            status,
            message: if body.trim().is_empty() {
                format!("HTTP {status}")
            } else {
                body.trim().to_string()
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthorized_maps_to_session_expired() {
        assert!(matches!(
            classify_status(401, None, ""),
            SktError::SessionExpired
        ));
        assert!(matches!(
            classify_status(403, None, "forbidden"),
            SktError::SessionExpired
        ));
    }

    #[test]
    fn too_many_requests_carries_retry_after() {
        match classify_status(429, Some(30), "") {
            SktError::RateLimited { retry_after } => assert_eq!(retry_after, Some(30)),
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[test]
    fn other_status_keeps_status_and_trimmed_body() {
        match classify_status(500, None, "  boom  ") {
            SktError::Api { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "boom");
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }

    #[test]
    fn empty_body_falls_back_to_status_text() {
        match classify_status(502, None, "   ") {
            SktError::Api { status, message } => {
                assert_eq!(status, 502);
                assert_eq!(message, "HTTP 502");
            }
            other => panic!("expected Api, got {other:?}"),
        }
    }
}
