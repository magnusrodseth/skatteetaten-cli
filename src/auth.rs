//! Interactive login: drive a real, headful Chrome to Min skatt, let the user
//! complete BankID, then capture the (HttpOnly) session cookies over CDP.
//!
//! There is no OAuth and no refresh here — unlike `sb1`. Min skatt authenticates
//! with a first-party HttpOnly cookie that only a browser can obtain (BankID is
//! interactive by design). So `skt login` opens a browser, waits for the login
//! to complete, reads every `skatteetaten.no` cookie via the DevTools Protocol,
//! and stores the assembled `Cookie:` header. Everything afterwards is plain
//! `reqwest` with that header — see [`crate::client`].
//!
//! The pure parts (cookie-header assembly, login detection) live in
//! [`crate::session`] and are unit-tested; this module is the browser IO glue.

use std::time::Duration;

use futures::StreamExt;

use crate::error::{Result, SktError};
use crate::secrets::{self, StoredSession};
use crate::session::{self, CapturedCookie};

/// Where the browser lands to trigger login; if not authenticated it redirects
/// to ID-porten / BankID, and returns here afterwards.
const LANDING_URL: &str = "https://skatt.skatteetaten.no/web/mineskatteforhold/";
const LOGINSTATUS_URL: &str = "https://skatt.skatteetaten.no/api/minside/loginstatus";

/// How long to wait for the user to complete BankID before giving up. Override
/// with `SKT_LOGIN_TIMEOUT` (seconds), mainly for testing the browser mechanics.
const DEFAULT_LOGIN_TIMEOUT_SECS: u64 = 300;
/// How often to check whether login has completed.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

fn login_timeout() -> Duration {
    let secs = std::env::var("SKT_LOGIN_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(DEFAULT_LOGIN_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

/// Run the interactive login and persist the captured session (blocking).
pub fn login() -> Result<StoredSession> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| SktError::AuthFlow(format!("could not start async runtime: {e}")))?;
    let session = runtime.block_on(login_async())?;
    secrets::save_session(&session)?;
    Ok(session)
}

async fn login_async() -> Result<StoredSession> {
    use chromiumoxide::browser::{Browser, BrowserConfig};

    let config = BrowserConfig::builder()
        .with_head()
        .build()
        .map_err(SktError::AuthFlow)?;

    let (mut browser, mut handler) = Browser::launch(config).await.map_err(|e| {
        SktError::AuthFlow(format!(
            "could not launch Chrome ({e}). Is Google Chrome or Chromium installed?"
        ))
    })?;

    // Drive the CDP websocket in the background.
    let handler_task = tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    let result = capture_session(&mut browser).await;

    // Always try to close the browser, even on error.
    let _ = browser.close().await;
    handler_task.abort();

    result
}

/// Open the portal and poll until the session cookie is present and valid.
async fn capture_session(browser: &mut chromiumoxide::browser::Browser) -> Result<StoredSession> {
    let page = browser
        .new_page(LANDING_URL)
        .await
        .map_err(|e| SktError::AuthFlow(format!("could not open Min skatt: {e}")))?;
    let _ = page.bring_to_front().await;

    let timeout = login_timeout();
    eprintln!("A browser window has opened for BankID login.");
    eprintln!(
        "Log in as yourself; this tool will detect when you're done and capture the session."
    );
    eprintln!("Waiting (up to {} s)…", timeout.as_secs());

    // A private async HTTP client used only to verify login against loginstatus
    // with whatever cookies the browser currently holds.
    let verifier = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(SktError::Http)?;

    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(SktError::AuthFlow(
                "timed out waiting for BankID login".into(),
            ));
        }
        tokio::time::sleep(POLL_INTERVAL).await;

        let captured = current_skatteetaten_cookies(browser).await;
        if captured.is_empty() {
            continue;
        }
        let cookie_header = session::to_cookie_header(&captured);

        // Verify: does this cookie set authenticate loginstatus?
        match verifier
            .get(LOGINSTATUS_URL)
            .header(reqwest::header::COOKIE, &cookie_header)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                let body = resp.text().await.unwrap_or_default();
                if session::login_succeeded(&body) {
                    return Ok(StoredSession {
                        cookie_header,
                        captured_at: crate::util::now_unix(),
                        expires_at: None,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Read all cookies from the browser and keep the Skatteetaten ones.
async fn current_skatteetaten_cookies(
    browser: &chromiumoxide::browser::Browser,
) -> Vec<CapturedCookie> {
    match browser.get_cookies().await {
        Ok(cookies) => cookies
            .into_iter()
            .filter(|c| session::is_skatteetaten(&c.domain))
            .map(|c| CapturedCookie {
                name: c.name,
                value: c.value,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}
