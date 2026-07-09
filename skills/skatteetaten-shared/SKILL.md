---
name: skatteetaten-shared
description: Runtime contract for the `skt` CLI (unofficial, read-only Skatteetaten / Min skatt). Covers install, BankID login, the short-lived session model, secret-storage backends, output formats, global flags, the command map, and reconciling tax data against sb1 bank data. Use this before any other skatteetaten skill, or whenever a user asks to read their Norwegian skattekort, skattemelding, or skatteoppgjør via skt.
compatibility: Requires the `skt` binary installed, plus Google Chrome/Chromium for the login step. Network access to skatt.skatteetaten.no. BankID is needed for every login (sessions are short-lived and cannot be refreshed).
---

# skatteetaten-shared

Foundation skill for the `skt` CLI, an **unofficial, read-only** client for a
private person's own Norwegian tax data on Skatteetaten's
[Min skatt](https://skatt.skatteetaten.no) portal. Read this before using any
other `skatteetaten` skill.

**This is real, sensitive tax data, and this tool is unofficial. Be
conservative.** `skt` has **no official API** behind it: it logs in with the
user's BankID like a browser and reads the portal's **undocumented internal JSON
endpoints** with the session cookie. It is **read-only** (never submits or edits)
and must only ever be used on the **user's own data**. Never invent amounts,
account numbers, or years; only act on what commands return or what the user
states. If the user asks for a submit/edit/mutation, refuse — it is deliberately
out of scope. See `TERMS.md`.

## Install & first-time setup

```bash
cargo install --path .        # installs the `skt` binary
```

One-time, the user must do themselves (needs their BankID and a browser):

1. Have **Google Chrome or Chromium** installed (used only during login).
2. `skt login` — opens a browser; the user completes BankID; `skt` captures the
   Min skatt session cookie over CDP and stores it.

Do **not** run `skt login` on the user's behalf without asking — it opens a
browser and triggers an interactive BankID flow.

## Authentication & the session model

```bash
skt status     # captured when? which storage backend?
skt whoami     # cheapest real check — verifies the session, prints the user's name
skt logout     # remove the stored session
```

**There is no refresh token.** Unlike a normal OAuth CLI, Min skatt sessions are
**short-lived and time out silently**. Consequences for you:

- If a read fails with an auth error (or `skt whoami` fails), the session has
  expired — tell the user to run `skt login` again. **Do not retry in a loop.**
- `skt status` shows capture time but cannot prove validity; only a request can.
  Use `skt whoami` to check before a batch of reads.

Secrets live in one of three backends, chosen by `SKT_STORE`:

| `SKT_STORE` | Where |
|---|---|
| _unset_ / `keychain` | OS keychain (macOS Keychain / Linux keyutils / Windows Credential Manager), **default** |
| `op` / `1password` | 1Password via the `op` CLI (`SKT_OP_VAULT`, `SKT_OP_ACCOUNT`) |
| `file` | `~/.config/skatteetaten-cli/*.json` (0600), **plaintext, opt-in** for headless use |

## Output & flags

- Default output is human-readable tables (Norwegian amounts: `kr 1 234,56`).
- Add `--json` to **any** command for machine-readable output — always use this
  when parsing programmatically.
- Add `--mask` to hide amounts and account numbers in tables (for screenshots).
- Errors print to stderr with a non-zero exit code.

## Command map

| Command | Purpose |
|---|---|
| `login` / `logout` / `status` / `whoami` | auth & session health |
| `skattekort` | tax-card line items for the active year (labels joined from the portal's dictionary) |
| `skattemelding [--year N]` | tax return for a year (defaults to the most recent filed) |
| `oppgjor` | skatteoppgjør / Min skatt landing (restskatt, konto, KID) — raw JSON |

Years: `skattekort` uses the active income year from the portal config;
`skattemelding` defaults to the year before that. Pass `--year` to pick another.

## Reconciling against the bank (the main use case)

The point of `skt` is to check the tax office's numbers against reality:

```bash
skt skattemelding --json | jq '.visningsdata.konto[]
  | {bank: .bankensNavn, konto: .kontonummer,
     innskudd: .innskudd.beloep, renter: .opptjenteRenter.beloep}'
```

Each `konto` entry is a bank account with its **deposit** and **interest earned**
as Skatteetaten has them. Cross-check these against `sb1` (the SpareBank 1 CLI):
do the balances and interest match what the bank actually reports? Use the
`sparebank1-*` skills for the bank side.

## Etiquette

Undocumented, unofficial endpoints. Make one request per call; **never** add
retry/polling loops. Keep everything **read-only** and on the **user's own data**.
Never expose the session cookie in output (the CLI already redacts it).
