//! HTTP client for the Min skatt internal JSON endpoints.
//!
//! These endpoints are **unofficial and undocumented**; this client only reads
//! (GET) the user's own data and never submits or mutates anything. One request
//! per call, no retry/polling loops. Authentication is the first-party session
//! cookie captured by `skt login`, replayed in a `Cookie:` header.

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{ACCEPT, COOKIE, RETRY_AFTER, USER_AGENT};

use crate::error::{classify_status, Result};
use crate::models::{SkattekortResponse, SkattemeldingResponse};
use crate::tekstliste::Tekstliste;

const BASE: &str = "https://skatt.skatteetaten.no";
const UA: &str = concat!(
    "skatteetaten-cli/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/magnusrodseth/skatteetaten-cli)"
);

/// Base host for the Min skatt API. Overridable via `SKT_BASE_URL` (for local
/// end-to-end testing against a mock, or a future test environment); defaults to
/// the real portal. A trailing slash is trimmed so URL building stays simple.
fn base() -> String {
    std::env::var("SKT_BASE_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches('/').to_string())
        .unwrap_or_else(|| BASE.to_string())
}

// ---- pure URL builders (unit-tested) -----------------------------------

fn url_loginstatus() -> String {
    format!("{}/api/minside/loginstatus", base())
}
fn url_skattekort_config() -> String {
    format!("{}/api/skattekort/config", base())
}
fn url_skattekort_visningsdata() -> String {
    format!("{}/api/skattekort/visningsdata/v2", base())
}
fn url_skattekort_tekstliste() -> String {
    format!("{}/api/skattekort/tekstliste?locale=nb", base())
}
fn url_oppgjor() -> String {
    format!("{}/api/mineskatteforhold/sider/skatt", base())
}
fn url_skattemelding_hent(year: i32) -> String {
    format!(
        "{}/api/skattemeldingen/{year}/skattemelding/hent-gjeldende",
        base()
    )
}
fn url_skattekort_beregn() -> String {
    format!("{}/api/skattekort/visningsdata/beregn/v2", base())
}

/// Honest User-Agent, overridable via `SKT_USER_AGENT` (e.g. if a WAF only
/// accepts a browser-like value).
fn user_agent() -> String {
    std::env::var("SKT_USER_AGENT").unwrap_or_else(|_| UA.to_string())
}

/// Build a bare blocking HTTP client with our identifying User-Agent.
pub fn http_agent() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(user_agent())
        .timeout(std::time::Duration::from_secs(60))
        .build()?)
}

/// Authenticated read-only API client.
pub struct ApiClient {
    http: Client,
    cookie_header: String,
}

impl ApiClient {
    pub fn new(cookie_header: String) -> Result<Self> {
        Ok(Self {
            http: http_agent()?,
            cookie_header,
        })
    }

    fn get(&self, url: &str) -> RequestBuilder {
        self.http
            .get(url)
            .header(ACCEPT, "application/json")
            .header(COOKIE, &self.cookie_header)
            .header(USER_AGENT, user_agent())
    }

    fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        let resp = check(self.get(url).send()?)?;
        Ok(resp.json()?)
    }

    fn post_json(&self, url: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let resp = check(
            self.http
                .post(url)
                .header(ACCEPT, "application/json")
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .header(COOKIE, &self.cookie_header)
                .header(USER_AGENT, user_agent())
                .json(body)
                .send()?,
        )?;
        Ok(resp.json()?)
    }

    /// `POST /api/skattekort/visningsdata/beregn/v2`: a pure what-if calculation.
    /// The body is the current `visningsdata/v2` response with one field changed;
    /// the response echoes a recomputed `beregnetSkatt`. Persists nothing.
    pub fn beregn(&self, body: &serde_json::Value) -> Result<serde_json::Value> {
        self.post_json(&url_skattekort_beregn(), body)
    }

    /// `GET /api/minside/loginstatus`: cheap auth health check. Returns the
    /// logged-in person's name on success.
    pub fn loginstatus(&self) -> Result<Option<String>> {
        let v = self.get_json(&url_loginstatus())?;
        Ok(v.get("navn")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string()))
    }

    /// Active skattekort income year from `/api/skattekort/config`.
    pub fn active_year(&self) -> Result<Option<i32>> {
        let v = self.get_json(&url_skattekort_config())?;
        Ok(v.get("features")
            .and_then(|f| f.get("inntekstaar"))
            .and_then(|y| y.as_str())
            .and_then(|s| s.parse::<i32>().ok()))
    }

    /// Typed skattekort line items (`visningsdata/v2`).
    pub fn skattekort(&self) -> Result<SkattekortResponse> {
        let resp = check(self.get(&url_skattekort_visningsdata()).send()?)?;
        Ok(resp.json()?)
    }

    /// Raw skattekort JSON (for `--json`).
    pub fn skattekort_raw(&self) -> Result<serde_json::Value> {
        self.get_json(&url_skattekort_visningsdata())
    }

    /// The label dictionary.
    pub fn tekstliste(&self) -> Result<Tekstliste> {
        let resp = check(self.get(&url_skattekort_tekstliste()).send()?)?;
        Ok(resp.json()?)
    }

    /// Raw skatteoppgjør / landing (`sider/skatt`), kept raw (nested blokk tree).
    pub fn oppgjor_raw(&self) -> Result<serde_json::Value> {
        self.get_json(&url_oppgjor())
    }

    /// Typed skattemelding for a year.
    pub fn skattemelding(&self, year: i32) -> Result<SkattemeldingResponse> {
        let resp = check(self.get(&url_skattemelding_hent(year)).send()?)?;
        Ok(resp.json()?)
    }

    /// Raw skattemelding JSON for a year (for `--json`).
    pub fn skattemelding_raw(&self, year: i32) -> Result<serde_json::Value> {
        self.get_json(&url_skattemelding_hent(year))
    }
}

/// Inspect a response: map 4xx/5xx to typed errors, pass success through.
fn check(resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let retry_after = resp
        .headers()
        .get(RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());
    let code = status.as_u16();
    let body = resp.text().unwrap_or_default();
    Err(classify_status(code, retry_after, &body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_point_at_the_min_skatt_host() {
        std::env::remove_var("SKT_BASE_URL");
        assert_eq!(
            url_loginstatus(),
            "https://skatt.skatteetaten.no/api/minside/loginstatus"
        );
        assert_eq!(
            url_skattekort_visningsdata(),
            "https://skatt.skatteetaten.no/api/skattekort/visningsdata/v2"
        );
        assert_eq!(
            url_skattekort_tekstliste(),
            "https://skatt.skatteetaten.no/api/skattekort/tekstliste?locale=nb"
        );
        assert_eq!(
            url_oppgjor(),
            "https://skatt.skatteetaten.no/api/mineskatteforhold/sider/skatt"
        );
    }

    #[test]
    fn skattemelding_urls_are_year_scoped() {
        std::env::remove_var("SKT_BASE_URL");
        assert_eq!(
            url_skattemelding_hent(2024),
            "https://skatt.skatteetaten.no/api/skattemeldingen/2024/skattemelding/hent-gjeldende"
        );
        assert_eq!(
            url_skattemelding_hent(2025),
            "https://skatt.skatteetaten.no/api/skattemeldingen/2025/skattemelding/hent-gjeldende"
        );
    }

    #[test]
    fn user_agent_defaults_to_honest_identifier() {
        std::env::remove_var("SKT_USER_AGENT");
        assert!(user_agent().starts_with("skatteetaten-cli/"));
    }
}
