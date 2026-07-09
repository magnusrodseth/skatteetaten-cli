//! Command handlers and dispatch.

pub mod auth;
pub mod read;

use anyhow::Context;

use crate::cli::{Cli, Command};
use crate::client::ApiClient;
use crate::format::OutputMode;

/// Resolved output mode from the global `--json` flag.
pub fn output_mode(cli: &Cli) -> OutputMode {
    if cli.json {
        OutputMode::Json
    } else {
        OutputMode::Table
    }
}

/// Build an authenticated API client from the stored session. Never triggers an
/// interactive login on its own — that is reserved for the explicit `login`
/// command, so non-interactive use fails loudly.
pub fn authed_client() -> anyhow::Result<ApiClient> {
    let session = crate::secrets::load_session()
        .context("reading stored session")?
        .ok_or(crate::error::SktError::NotAuthenticated)?;
    if session.is_definitely_expired() {
        return Err(crate::error::SktError::SessionExpired.into());
    }
    Ok(ApiClient::new(session.cookie_header)?)
}

/// Top-level dispatch.
pub fn run(cli: Cli) -> anyhow::Result<()> {
    let mode = output_mode(&cli);
    let mask = cli.mask;
    match cli.command {
        Command::Login => auth::login(),
        Command::Logout => auth::logout(),
        Command::Status => auth::status(mode),
        Command::Whoami => auth::whoami(mode),
        Command::Skattekort => read::skattekort(mode, mask),
        Command::Skattemelding(args) => read::skattemelding(args, mode, mask),
        Command::Oppgjor => read::oppgjor(mode),
    }
}
