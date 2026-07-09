//! What shapes this client's behaviour. Unlike `sb1` (which wraps a sanctioned
//! personal API), `skt` has **no official personal API** to lean on: it replays
//! an authenticated Min skatt browser session against undocumented internal
//! endpoints. That difference is the whole reason for the rules below.
//!
//! - **Unofficial & unsupported.** Skatteetaten publishes no personal developer
//!   API. These endpoints are internal to the Min skatt SPA, undocumented, and
//!   can change or break without notice. This tool is not affiliated with or
//!   endorsed by Skatteetaten.
//!
//! - **Read-only.** `skt` only issues GET requests for the user's own data. It
//!   never submits a skattemelding, edits a skattekort, or mutates anything.
//!   The submit/edit endpoints exist but are deliberately not wrapped.
//!
//! - **Your own data, personal use.** The session belongs to the logged-in
//!   person; use it only to read your own tax data. Automating a government auth
//!   session is a grey area — keep it personal and read-only.
//!
//! - **Sensitive session.** The captured cookie is a bearer of full portal read
//!   access. It is stored in your chosen secret store and its value is redacted
//!   from all output.
//!
//! - **No rate-limit games.** One request per call, no retry or polling loops.

/// One-line summary shown by `skt status`.
pub const SUMMARY: &str = "Unofficial, read-only tool for your OWN Norwegian tax \
data. Not affiliated with Skatteetaten. It replays an authenticated Min skatt \
browser session against undocumented internal endpoints (which may change without \
notice) and only ever reads — it never submits or edits.";
