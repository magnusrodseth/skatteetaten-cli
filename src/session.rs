//! Pure helpers for turning captured browser cookies into a session, kept
//! separate from the browser-driving IO in [`crate::auth`] so they can be
//! unit-tested without a live Chrome.

/// A cookie as read from the browser over CDP. Only the fields we need to build
/// a `Cookie:` header; the browser has already scoped these to Skatteetaten.
#[derive(Debug, Clone)]
pub struct CapturedCookie {
    pub name: String,
    pub value: String,
}

/// True if a cookie belongs to Skatteetaten (any `skatteetaten.no` host).
pub fn is_skatteetaten(domain: &str) -> bool {
    let d = domain.trim_start_matches('.').to_ascii_lowercase();
    d == "skatteetaten.no" || d.ends_with(".skatteetaten.no")
}

/// Build a `Cookie:` header value from captured cookies, in first-seen order,
/// de-duplicating by name (last value wins) and skipping nameless entries.
pub fn to_cookie_header(cookies: &[CapturedCookie]) -> String {
    // Preserve the order names first appear, but let a later cookie override an
    // earlier one with the same name (CDP can return per-path duplicates).
    let mut order: Vec<String> = Vec::new();
    let mut values: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for c in cookies {
        if c.name.is_empty() {
            continue;
        }
        if !values.contains_key(&c.name) {
            order.push(c.name.clone());
        }
        values.insert(c.name.clone(), c.value.clone());
    }
    order
        .iter()
        .map(|name| format!("{name}={}", values[name]))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Redact cookie *values* in a `Cookie:` header so it can be shown/logged
/// without leaking the session bearer. Names are kept for diagnostics.
///
/// `A=secret; B=other` -> `A=<redacted>; B=<redacted>`
pub fn redact(cookie_header: &str) -> String {
    cookie_header
        .split(';')
        .map(|pair| {
            let pair = pair.trim();
            if pair.is_empty() {
                return String::new();
            }
            match pair.split_once('=') {
                Some((name, _)) => format!("{name}=<redacted>"),
                // A valueless cookie: keep the name for diagnostics.
                None => format!("{pair}=<redacted>"),
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

/// Decide whether the `/api/minside/loginstatus` body indicates a completed
/// login. On success the portal returns `{"navn":"<full name>"}`.
pub fn login_succeeded(loginstatus_body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(loginstatus_body)
        .ok()
        .and_then(|v| {
            v.get("navn")
                .and_then(|n| n.as_str())
                .map(|s| !s.trim().is_empty())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(name: &str, value: &str, _domain: &str) -> CapturedCookie {
        CapturedCookie {
            name: name.into(),
            value: value.into(),
        }
    }

    #[test]
    fn recognises_skatteetaten_hosts() {
        assert!(is_skatteetaten("skatt.skatteetaten.no"));
        assert!(is_skatteetaten(".skatteetaten.no"));
        assert!(is_skatteetaten("skatteetaten.no"));
        assert!(!is_skatteetaten("evil-skatteetaten.no.attacker.com"));
        assert!(!is_skatteetaten("idporten.no"));
    }

    #[test]
    fn builds_cookie_header_in_order() {
        let cookies = [
            c("MRHSession", "abc", "skatt.skatteetaten.no"),
            c("F5_ST", "xyz", ".skatteetaten.no"),
        ];
        assert_eq!(to_cookie_header(&cookies), "MRHSession=abc; F5_ST=xyz");
    }

    #[test]
    fn later_duplicate_name_overrides_but_keeps_position() {
        let cookies = [
            c("S", "old", "a.skatteetaten.no"),
            c("T", "t", ".skatteetaten.no"),
            c("S", "new", "skatteetaten.no"),
        ];
        assert_eq!(to_cookie_header(&cookies), "S=new; T=t");
    }

    #[test]
    fn skips_nameless_cookies() {
        let cookies = [
            c("", "orphan", "skatteetaten.no"),
            c("A", "1", "skatteetaten.no"),
        ];
        assert_eq!(to_cookie_header(&cookies), "A=1");
    }

    #[test]
    fn redact_masks_values_keeps_names() {
        assert_eq!(
            redact("MRHSession=secret; F5_ST=deadbeef"),
            "MRHSession=<redacted>; F5_ST=<redacted>"
        );
    }

    #[test]
    fn redact_handles_empty_and_valueless() {
        assert_eq!(redact(""), "");
        assert_eq!(redact("justname"), "justname=<redacted>");
    }

    #[test]
    fn login_detected_only_with_nonempty_name() {
        assert!(login_succeeded(r#"{"navn":"Magnus Rødseth"}"#));
        assert!(!login_succeeded(r#"{"navn":""}"#));
        assert!(!login_succeeded(r#"{"navn":"   "}"#));
        assert!(!login_succeeded("{}"));
        assert!(!login_succeeded("not json"));
        assert!(!login_succeeded(""));
    }
}
