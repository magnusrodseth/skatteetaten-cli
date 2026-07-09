//! Session-cookie and settings storage.
//!
//! Min skatt authenticates with a first-party, HttpOnly session cookie captured
//! during `skt login`. That cookie is a bearer of full read access to your tax
//! data, so it is treated as sensitive and stored exactly like `sb1` stores its
//! OAuth token.
//!
//! Three backends are supported, selected by the `SKT_STORE` environment
//! variable. The default is the OS keychain, so the easy path is also a secure
//! one; plaintext files are an explicit, deliberate opt-in.
//!
//! - **`keychain`** (default): the platform-native secret store (macOS Keychain,
//!   Linux kernel keyutils, Windows Credential Manager). Secure at rest.
//! - **`op`** / **`1password`**: 1Password via the `op` CLI. Items live in a
//!   vault (default `Private`, override with `SKT_OP_VAULT`). Requires `op`
//!   installed and signed in.
//! - **`file`** (opt-in): JSON files under `~/.config/skatteetaten-cli/` with
//!   `0600` permissions. Plaintext on disk; keep it out of version control.

use std::path::PathBuf;

use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Keychain "service" namespace for all entries written by this tool.
const SERVICE: &str = "skatteetaten-cli";
const ACCT_SESSION: &str = "min-skatt-session";

/// Storage backend, chosen by `SKT_STORE` (default: keychain).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    File,
    Keychain,
    OnePassword,
}

fn backend() -> Backend {
    match std::env::var("SKT_STORE").as_deref() {
        Ok("file") => Backend::File,
        Ok("op") | Ok("1password") => Backend::OnePassword,
        // keychain is the secure default; anything unset/unknown falls back to it
        // rather than silently writing plaintext.
        _ => Backend::Keychain,
    }
}

/// One-line summary of every supported backend, for status output so the user
/// (and any agent driving the CLI) sees all alternatives. The boolean marks the
/// currently active one.
pub fn backend_options() -> Vec<(&'static str, &'static str, bool)> {
    let active = backend();
    vec![
        (
            "keychain",
            "Platform-native store (macOS Keychain / Linux keyutils / Windows Credential Manager). Default, secure at rest.",
            active == Backend::Keychain,
        ),
        (
            "op",
            "1Password via the `op` CLI. Unlock enforced by 1Password (biometrics).",
            active == Backend::OnePassword,
        ),
        (
            "file",
            "Plaintext JSON in ~/.config/skatteetaten-cli (0600). For headless/automation; keep out of git and backups.",
            active == Backend::File,
        ),
    ]
}

/// 1Password vault for the `op` backend.
fn op_vault() -> String {
    std::env::var("SKT_OP_VAULT").unwrap_or_else(|_| "Private".to_string())
}

/// 1Password item title for a logical account name.
fn op_title(account: &str) -> String {
    match account {
        ACCT_SESSION => "Skatteetaten CLI session".to_string(),
        other => format!("Skatteetaten CLI {other}"),
    }
}

/// `~/.config/skatteetaten-cli` (honours `XDG_CONFIG_HOME`).
fn config_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("skatteetaten-cli");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("skatteetaten-cli")
}

fn file_path(account: &str) -> PathBuf {
    config_dir().join(format!("{account}.json"))
}

/// Read a stored value by logical account name from the active backend.
fn kv_get(account: &str) -> Result<Option<String>> {
    match backend() {
        Backend::File => match std::fs::read_to_string(file_path(account)) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(crate::error::SktError::AuthFlow(format!(
                "reading {}: {e}",
                file_path(account).display()
            ))),
        },
        Backend::Keychain => match Entry::new(SERVICE, account)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        },
        Backend::OnePassword => op_read(account),
    }
}

/// Write a value by logical account name to the active backend.
fn kv_set(account: &str, value: &str) -> Result<()> {
    match backend() {
        Backend::File => {
            let dir = config_dir();
            std::fs::create_dir_all(&dir).map_err(|e| {
                crate::error::SktError::AuthFlow(format!("creating {}: {e}", dir.display()))
            })?;
            let path = file_path(account);
            std::fs::write(&path, value).map_err(|e| {
                crate::error::SktError::AuthFlow(format!("writing {}: {e}", path.display()))
            })?;
            set_owner_only(&path)?;
            Ok(())
        }
        Backend::Keychain => Ok(Entry::new(SERVICE, account)?.set_password(value)?),
        Backend::OnePassword => op_write(account, value),
    }
}

/// Delete a stored value; a missing entry is not an error.
fn kv_delete(account: &str) -> Result<()> {
    match backend() {
        Backend::File => match std::fs::remove_file(file_path(account)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(crate::error::SktError::AuthFlow(format!(
                "removing file: {e}"
            ))),
        },
        Backend::Keychain => match Entry::new(SERVICE, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        },
        Backend::OnePassword => op_delete(account),
    }
}

// ---- 1Password (`op` CLI) backend --------------------------------------

fn op_err(msg: impl std::fmt::Display) -> crate::error::SktError {
    crate::error::SktError::AuthFlow(format!("1Password (op) {msg}"))
}

/// Run `op` with args, returning the process output. If `SKT_OP_ACCOUNT` is set
/// it is passed as `--account` to disambiguate multiple 1Password accounts.
fn op_run(args: &[&str]) -> Result<std::process::Output> {
    let mut cmd = std::process::Command::new("op");
    if let Ok(account) = std::env::var("SKT_OP_ACCOUNT") {
        if !account.is_empty() {
            cmd.arg("--account").arg(account);
        }
    }
    cmd.args(args).output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            op_err("CLI not found, install it from https://developer.1password.com/docs/cli/")
        } else {
            op_err(e)
        }
    })
}

fn op_read(account: &str) -> Result<Option<String>> {
    let vault = op_vault();
    let title = op_title(account);
    let reference = format!("op://{vault}/{title}/password");
    let out = op_run(&["read", "--no-newline", &reference])?;
    if out.status.success() {
        return Ok(Some(String::from_utf8_lossy(&out.stdout).to_string()));
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("isn't an item")
        || stderr.contains("not found")
        || stderr.contains("doesn't exist")
        || stderr.contains("no item")
    {
        return Ok(None);
    }
    Err(op_err(format!("read failed: {}", stderr.trim())))
}

fn op_write(account: &str, value: &str) -> Result<()> {
    let vault = op_vault();
    let title = op_title(account);

    let exists = op_run(&["item", "get", &title, "--vault", &vault])?
        .status
        .success();

    let assignment = format!("password={value}");
    let out = if exists {
        op_run(&["item", "edit", &title, "--vault", &vault, &assignment])?
    } else {
        op_run(&[
            "item",
            "create",
            "--category",
            "Password",
            "--title",
            &title,
            "--vault",
            &vault,
            &assignment,
        ])?
    };
    if out.status.success() {
        Ok(())
    } else {
        Err(op_err(format!(
            "write failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

fn op_delete(account: &str) -> Result<()> {
    let vault = op_vault();
    let title = op_title(account);
    let out = op_run(&["item", "delete", &title, "--vault", &vault])?;
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("isn't an item") || stderr.contains("not found") {
        return Ok(());
    }
    Err(op_err(format!("delete failed: {}", stderr.trim())))
}

/// Restrict a secret file to `0600` (owner read/write only).
#[cfg(unix)]
fn set_owner_only(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|e| {
        crate::error::SktError::AuthFlow(format!("setting permissions on {}: {e}", path.display()))
    })
}

#[cfg(not(unix))]
fn set_owner_only(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

/// A captured Min skatt session: the `Cookie:` header value plus the times we
/// need to reason about staleness.
///
/// Portal sessions are short-lived and have a server-side idle timeout that we
/// cannot observe locally, so `expires_at` is only a *hint* (the latest explicit
/// cookie expiry seen at capture, if any). The authoritative check is a request
/// returning 401/403 — see [`crate::error::SktError::SessionExpired`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSession {
    /// Serialised `name=value; name2=value2` cookie header for skatteetaten.no.
    pub cookie_header: String,
    /// Unix epoch seconds when the session was captured.
    pub captured_at: i64,
    /// Optional hint: latest explicit cookie expiry (epoch seconds), if any
    /// cookie carried one. Most portal cookies are session cookies with none.
    #[serde(default)]
    pub expires_at: Option<i64>,
}

impl StoredSession {
    /// Age of the session in seconds (never negative).
    pub fn age_seconds(&self) -> i64 {
        (crate::util::now_unix() - self.captured_at).max(0)
    }

    /// True only when we can *prove* the session is past a known expiry. Unknown
    /// expiry returns `false`: we don't guess, we let the API return 401.
    pub fn is_definitely_expired(&self) -> bool {
        match self.expires_at {
            Some(exp) => crate::util::now_unix() >= exp,
            None => false,
        }
    }
}

/// Persist the captured session.
pub fn save_session(session: &StoredSession) -> Result<()> {
    kv_set(ACCT_SESSION, &serde_json::to_string_pretty(session)?)
}

/// Load the stored session, if present.
pub fn load_session() -> Result<Option<StoredSession>> {
    match kv_get(ACCT_SESSION)? {
        Some(json) => Ok(Some(serde_json::from_str(&json)?)),
        None => Ok(None),
    }
}

/// Remove the stored session (used by `logout`). Missing entry is not an error.
pub fn delete_session() -> Result<()> {
    kv_delete(ACCT_SESSION)
}

/// The on-disk storage directory when the file backend is active, so the CLI can
/// warn the user to keep these files out of version control. `None` otherwise.
pub fn file_store_dir() -> Option<PathBuf> {
    match backend() {
        Backend::File => Some(config_dir()),
        Backend::Keychain | Backend::OnePassword => None,
    }
}

/// Human-readable name of the active backend (for status output).
pub fn backend_name() -> &'static str {
    match backend() {
        Backend::File => "file (~/.config/skatteetaten-cli)",
        Backend::Keychain => "keychain",
        Backend::OnePassword => "1password (op)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_title_maps_logical_accounts() {
        assert_eq!(op_title(ACCT_SESSION), "Skatteetaten CLI session");
        assert_eq!(op_title("other"), "Skatteetaten CLI other");
    }

    #[test]
    fn session_age_is_never_negative() {
        let future = StoredSession {
            cookie_header: "a=b".into(),
            captured_at: crate::util::now_unix() + 1000,
            expires_at: None,
        };
        assert_eq!(future.age_seconds(), 0);
    }

    #[test]
    fn session_without_expiry_is_never_definitely_expired() {
        let s = StoredSession {
            cookie_header: "a=b".into(),
            captured_at: crate::util::now_unix() - 100_000,
            expires_at: None,
        };
        assert!(!s.is_definitely_expired());
    }

    #[test]
    fn session_past_known_expiry_is_definitely_expired() {
        let expired = StoredSession {
            cookie_header: "a=b".into(),
            captured_at: crate::util::now_unix() - 100,
            expires_at: Some(crate::util::now_unix() - 10),
        };
        assert!(expired.is_definitely_expired());
        let valid = StoredSession {
            expires_at: Some(crate::util::now_unix() + 600),
            ..expired
        };
        assert!(!valid.is_definitely_expired());
    }

    // Mutates the process-global SKT_STORE; kept in one test so it runs alone.
    #[test]
    fn backend_is_selected_by_env_with_keychain_default() {
        std::env::set_var("SKT_STORE", "keychain");
        assert_eq!(backend(), Backend::Keychain);
        std::env::set_var("SKT_STORE", "op");
        assert_eq!(backend(), Backend::OnePassword);
        std::env::set_var("SKT_STORE", "1password");
        assert_eq!(backend(), Backend::OnePassword);
        std::env::set_var("SKT_STORE", "file");
        assert_eq!(backend(), Backend::File);
        std::env::set_var("SKT_STORE", "bogus");
        assert_eq!(backend(), Backend::Keychain);
        std::env::remove_var("SKT_STORE");
        assert_eq!(backend(), Backend::Keychain);
    }
}
