//! Auth-related commands: login, logout, status, whoami.

use anyhow::Context;
use chrono::{Local, TimeZone};

use crate::format::OutputMode;
use crate::{auth, secrets, session, terms};

/// Print the active storage backend and every alternative, so the user (and any
/// agent) sees what is supported and how to switch via SKT_STORE.
fn print_storage_overview() {
    eprintln!("Secret storage (choose with the SKT_STORE environment variable):");
    for (name, desc, active) in secrets::backend_options() {
        let marker = if active { "→ active" } else { "        " };
        eprintln!("  {marker}  {name:<9} {desc}");
    }
    eprintln!();
}

pub fn login() -> anyhow::Result<()> {
    print_storage_overview();

    if let Some(dir) = secrets::file_store_dir() {
        eprintln!(
            "⚠ Storing the session as a file in {}\n\
             \x20 This cookie is a SECRET (full read access to your tax data). Make sure that\n\
             \x20 directory is NOT in a git repo and is excluded from cloud backups. Use\n\
             \x20 `SKT_STORE=keychain` or `SKT_STORE=op` to keep it out of plaintext files.\n",
            dir.display()
        );
    }

    let stored = auth::login().context("BankID login failed")?;
    let captured = Local
        .timestamp_opt(stored.captured_at, 0)
        .single()
        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default();
    println!("✅ Logged in and captured the Min skatt session at {captured}.");
    println!("   Stored via {}.", secrets::backend_name());
    println!(
        "   Note: portal sessions are short-lived and cannot be refreshed. Run `skt login` again when reads start failing."
    );
    Ok(())
}

pub fn logout() -> anyhow::Result<()> {
    secrets::delete_session()?;
    println!("✅ Logged out (session removed).");
    Ok(())
}

pub fn status(mode: OutputMode) -> anyhow::Result<()> {
    let stored = secrets::load_session()?;

    let (logged_in, captured_at, age_seconds, definitely_expired, redacted) = match &stored {
        Some(s) => (
            true,
            Some(s.captured_at),
            Some(s.age_seconds()),
            s.is_definitely_expired(),
            Some(session::redact(&s.cookie_header)),
        ),
        None => (false, None, None, false, None),
    };
    let captured_str = captured_at
        .and_then(|c| Local.timestamp_opt(c, 0).single())
        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string());

    if mode == OutputMode::Json {
        return crate::format::print_json(&serde_json::json!({
            "loggedIn": logged_in,
            "capturedAt": captured_str,
            "ageSeconds": age_seconds,
            "definitelyExpired": definitely_expired,
            "cookies": redacted,
            "storageBackend": secrets::backend_name(),
            "supportedBackends": secrets::backend_options()
                .iter()
                .map(|(name, _, _)| *name)
                .collect::<Vec<_>>(),
        }));
    }

    println!("Logged in:           {}", yesno(logged_in));
    if let Some(c) = captured_str {
        let age = age_seconds
            .map(|s| format!(" ({} min ago)", s / 60))
            .unwrap_or_default();
        println!("Session captured:    {c}{age}");
    }
    if logged_in {
        println!(
            "Session validity:    {}",
            if definitely_expired {
                "expired (re-login needed)"
            } else {
                "unknown until a request is made (portal sessions time out silently)"
            }
        );
    }
    println!("Storage backend:     {}", secrets::backend_name());
    println!("\nSecret storage options (set SKT_STORE):");
    for (name, desc, active) in secrets::backend_options() {
        let marker = if active { "→ active" } else { "        " };
        println!("  {marker}  {name:<9} {desc}");
    }
    println!("\n{}", terms::SUMMARY);
    Ok(())
}

pub fn whoami(mode: OutputMode) -> anyhow::Result<()> {
    let client = crate::commands::authed_client()?;
    let name = client.loginstatus().context("login status check failed")?;
    match (mode, name) {
        (OutputMode::Json, name) => crate::format::print_json(&serde_json::json!({
            "loggedIn": name.is_some(),
            "navn": name,
        })),
        (OutputMode::Table, Some(name)) => {
            println!("{name}");
            Ok(())
        }
        (OutputMode::Table, None) => Err(crate::error::SktError::SessionExpired.into()),
    }
}

fn yesno(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}
