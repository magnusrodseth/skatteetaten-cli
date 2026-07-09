# Skatteetaten CLI

<p align="center">
  <img src="https://raw.githubusercontent.com/magnusrodseth/skatteetaten-cli/main/docs/banner.png" alt="A person at a laptop and an AI agent reviewing a tax return together, with a magnifying glass, a checkmarked form and kroner coins" width="640">
</p>

**Your tax data, agent-ready.** `skt` is a **read-only** CLI that reads **your
own** Norwegian tax data from Skatteetaten's
[Min skatt](https://skatt.skatteetaten.no) portal (skattekort, skattemelding,
skatteoppgjør) and hands it to an AI agent to reason over. Same spirit as
[`sb1`](https://github.com/magnusrodseth/sparebank1-cli): log in once with
BankID, then get clean `--json` output.

Written in Rust. The binary is `skt`; the crate is `skatteetaten-cli`.

> [!WARNING]
> **Unofficial, undocumented, read-only, your own data.** Skatteetaten publishes
> no personal developer API. `skt` logs in with your BankID like a browser does,
> then reads the Min skatt SPA's **internal JSON endpoints** with the session
> cookie. Those endpoints are undocumented and can change or break without
> notice, and automating a government auth session is a grey area. `skt` only
> ever **reads your own data** — it never submits or edits anything. Keep it
> personal and read-only. Not affiliated with or endorsed by Skatteetaten. See
> [`TERMS.md`](TERMS.md).

## How it differs from `sb1`

`sb1` wraps a sanctioned OAuth API with a 90-day refresh token. `skt` cannot:
Min skatt authenticates with a **first-party, HttpOnly session cookie** that only
a browser can obtain (BankID is interactive by design). So:

- **`skt login` opens a real browser.** You complete BankID; `skt` then captures
  the session cookies over the Chrome DevTools Protocol and stores them.
- **There is no refresh.** Portal sessions are short-lived and time out silently.
  When reads start failing, run `skt login` again.
- Everything after login is plain HTTPS with a `Cookie:` header.

Requires **Google Chrome or Chromium** installed (for the login step only).

## Install

Prebuilt binary (no Rust toolchain, does not call the GitHub API so it is not
affected by API rate limits):

```bash
curl -fsSL https://raw.githubusercontent.com/magnusrodseth/skatteetaten-cli/main/install.sh | bash
```

- Installs to `~/.local/bin` (override with `SKT_INSTALL_DIR`).
- Pin a version with `SKT_VERSION=v0.1.0`.
- macOS and Linux only.

Or from crates.io / source:

```bash
cargo install skatteetaten-cli   # from crates.io
cargo install --path .           # from a local checkout
```

The login step needs **Google Chrome or Chromium** installed.

## Quick start

```bash
skt login                        # opens a browser for BankID, captures the session
skt whoami                       # confirm the session (prints your name)
skt skattekort                   # tax-card line items for the active year
skt skattemelding                # most recent tax return (add --year 2024)
skt oppgjor                      # skatteoppgjør / Min skatt landing
skt whatif --field pensjonsinnbetaling --add 50000   # recompute tax with one change
skt status                       # session + storage backend
skt logout                       # remove the stored session
```

Add `--json` to **any** command for machine-readable output (use this when an
agent parses it). Add `--mask` to hide amounts and account numbers in tables
(for screenshots).

```bash
skt skattemelding --year 2024 --json | jq '.visningsdata.konto'
```

## Secret storage

The captured session cookie is a bearer of full **read** access to your tax
data, so it is stored like any secret. Choose the backend with `SKT_STORE`:

| `SKT_STORE` | Where |
|---|---|
| _unset_ / `keychain` | OS keychain (macOS Keychain / Linux keyutils / Windows Credential Manager), **default** |
| `op` / `1password` | 1Password via the `op` CLI (`SKT_OP_VAULT`, `SKT_OP_ACCOUNT`) |
| `file` | `~/.config/skatteetaten-cli/*.json` (0600), **plaintext, opt-in** for headless use |

## What-if: optimize before the deadline

`skt whatif` asks the portal to recompute your tax with a single skattekort
field changed — a pure calculation that **saves nothing**. Great for
"if I raise my pension by X, what happens to my tax?":

```bash
skt whatif --field pensjonsinnbetaling --add 50000   # add 50 000 kr of pension
skt whatif --field innskudd --set 200000             # set bank deposits to an amount
skt whatif --field paaloepteRenterGenerell --add -10000 --json
```

Use the field names from the rightmost "Felt" column of `skt skattekort`. Output
shows the changed field and your **sum skatt** before vs after, with the delta.

## Reconciling against your bank (the point)

`skt skattemelding --json` exposes `visningsdata.konto[]` — each with the bank
name, account number, `innskudd` (deposit) and `opptjenteRenter` (interest
earned). That is the direct reconciliation target against `sb1` account balances
and transactions: check that the interest and balances the tax office has on file
match what your bank actually reports.

## Environment variables

| Variable | Purpose |
|---|---|
| `SKT_STORE` | Secret backend (`keychain` default, `op`, `file`). |
| `SKT_OP_VAULT` / `SKT_OP_ACCOUNT` | 1Password vault / account for the `op` backend. |
| `SKT_USER_AGENT` | Override the HTTP User-Agent (e.g. if a WAF rejects the default). |
| `SKT_LOGIN_TIMEOUT` | Seconds to wait for BankID during `skt login` (default 300). |
| `SKT_BASE_URL` | Override the API host (for testing against a mock). |

## Scope

- **In scope:** `login` / `logout` / `status` / `whoami`, the read commands
  `skattekort`, `skattemelding [--year]`, `oppgjor`, and the pure-calculation
  `whatif` — each with `--json`.
- **Not in scope:** submitting a skattemelding, editing a skattekort, or any
  other mutation. Those endpoints exist but are deliberately not wrapped.
  `whatif` uses the portal's own what-if calculator and persists nothing.

## Development

```bash
cargo test          # unit tests (pure logic: cookie assembly, tekstliste join, parsing, errors)
cargo clippy        # lints
```

Example (scrubbed) API responses live in [`docs/api/`](docs/api/). See also the
agent skill `skatteetaten-shared` under [`skills/`](skills/).

## License

MIT.
