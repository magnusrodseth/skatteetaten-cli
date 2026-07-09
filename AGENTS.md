# AGENTS.md

Guidance for AI agents helping a user set up and use `skt`, the unofficial,
read-only Skatteetaten (Min skatt) CLI. If you are an agent and the user pointed
you at this repository, start here.

## What this is

`skt` reads a private person's **own** Norwegian tax data (skattekort,
skattemelding, skatteoppgjør) from Skatteetaten's Min skatt portal. There is **no
official personal API**: `skt` logs in with the user's BankID like a browser
does, captures the session cookie, and reads the portal's internal JSON
endpoints. It is unofficial, undocumented, and **read-only** — it never submits
or edits. See `TERMS.md`.

Every command supports `--json` for machine-readable output.

## Set up access (the user must do this once)

BankID is interactive; you cannot do it for them.

1. Install **Google Chrome or Chromium** (used only during login).
2. `skt login` — a browser opens; the user completes BankID; `skt` captures and
   stores the session. **Do not run `skt login` on the user's behalf without
   asking** — it opens a browser and requires their BankID.
3. Session storage backend is chosen with `SKT_STORE` (`keychain` default, `op`
   for 1Password, `file` for headless). See `README.md` → "Secret storage".

## The session model (important)

Unlike a normal OAuth CLI, **there is no refresh token**. Min skatt sessions are
short-lived and time out silently. So:

- If a read fails with an auth error, the session has expired — tell the user to
  run `skt login` again. Do **not** retry in a loop.
- `skt status` shows when the session was captured, but validity can only be
  confirmed by actually making a request (`skt whoami` is the cheapest check).

## Operating rules for agents

- Use `--json` for anything you parse; tables are for humans.
- `skt` is **read-only**. There are no money-moving or submit commands, and none
  should be added.
- `skt` only sees Skatteetaten's data. Cross-check against `sb1` for the bank
  side: `skt skattemelding --json` → `visningsdata.konto[]` gives bank name,
  account number, deposit (`innskudd`) and interest (`opptjenteRenter`) — reconcile
  those against `sb1` balances and transactions.
- Endpoints are undocumented and may change. If a command starts returning
  unexpected shapes, check `docs/api/` for the known structure and report the
  drift rather than guessing.

## Install the agent skill

```
skills/skatteetaten-shared   # runtime contract: auth, storage, commands, output, the read-only/unofficial framing
```

Read `skatteetaten-shared` before driving `skt`.
