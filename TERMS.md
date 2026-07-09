# Terms, scope, and the honest caveats

`skt` is an **unofficial** tool. Unlike `sb1` (which wraps a sanctioned personal
banking API), there is **no official personal API** from Skatteetaten for a
private individual. This document states plainly what `skt` does and the
constraints that shape it. Read it before using or extending the tool.

## What `skt` actually does

Skatteetaten's Min skatt portal (`skatt.skatteetaten.no`) is a React single-page
app backed by internal JSON endpoints. When you log in with BankID via ID-porten,
the portal sets a first-party, HttpOnly session cookie. `skt`:

1. Opens a real browser so you can complete BankID (it cannot be automated — it
   is interactive by design).
2. Captures the resulting `skatteetaten.no` session cookies over the Chrome
   DevTools Protocol.
3. Replays them in a `Cookie:` header to **GET** the same internal endpoints the
   SPA calls, and parses the JSON.

That is the only route available to a private person. Every *official*
Skatteetaten/Folkeregisteret API requires a Norwegian organisasjonsnummer,
Maskinporten, and a Skatteetaten access grant — closed to individuals.

## The caveats, stated plainly

- **Unofficial & unsupported.** These endpoints are undocumented and internal.
  They can change, move, or break at any time with no notice. `skt` is not
  affiliated with, supported by, or endorsed by Skatteetaten.
- **Grey area.** Automating an authenticated government session is not something
  Skatteetaten publishes an interface for. Keep usage **personal, on your own
  data, and read-only**. Do not use it to access anyone else's data.
- **Read-only by design.** `skt` issues only GET requests. It never submits a
  skattemelding, edits a skattekort, or mutates anything. The portal's
  submit/edit endpoints exist; wrapping them would be a different and higher risk
  class, and is deliberately out of scope.
- **Sensitive session.** The captured cookie grants full **read** access to your
  tax data. It is stored in your chosen secret store (keychain / 1Password /
  `0600` file) and its value is redacted from all output. Treat it like a
  password.
- **No rate-limit games.** `skt` makes one request per call and never retries or
  polls in a loop.

## Your responsibility

You are responsible for how you use this tool and for protecting the data and
session it handles. Use it on your own tax data, for personal purposes, and stop
if Skatteetaten asks you to.
