# Saved API responses (`docs/api/`)

Example responses from the **unofficial, undocumented** Min skatt internal JSON
endpoints that `skt` reads. They exist so the response shapes are documented and
so the parsers have something to test against when the live endpoints are not
reachable.

**All values here are scrubbed.** Every personal identifier (names, account
numbers, org numbers, KID/IBAN, document ids) is replaced with `SCRUBBED`, and
every amount with `12345`. The **structure** (keys, nesting, array shapes, and
whether an amount is a number or a string) is faithful to the real responses;
only the values are fake. Nothing here is real personal data.

| File | Endpoint | Notes |
|---|---|---|
| `loginstatus.json` | `GET /api/minside/loginstatus` | Auth health check; returns `{navn}`. |
| `skattekort_config.json` | `GET /api/skattekort/config` | `features.inntekstaar` = active skattekort year. |
| `skattekort_visningsdata_v2.json` | `GET /api/skattekort/visningsdata/v2` | Line items in `visningsdata.prognoseobjekt` keyed by field name. |
| `skattekort_tekstliste_trimmed.json` | `GET /api/skattekort/tekstliste?locale=nb` | Label dictionary (trimmed to the fields `skt` renders). Join: `tekstelementer["visningsnavn.felt.<field>"].tekst`. |
| `sider_skatt.json` | `GET /api/mineskatteforhold/sider/skatt` | Skatteoppgjør / landing as a nested `blokker` tree. |
| `skattemelding_2024_hent-gjeldende.json` | `GET /api/skattemeldingen/2024/skattemelding/hent-gjeldende` | Full return; amounts are `{beloep, beloepIValuta, valutakode, valutakurs}` **strings**. `visningsdata.konto[]` is the sb1 reconciliation target. |

These endpoints are internal to the Min skatt SPA and can change without notice.
