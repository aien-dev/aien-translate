# Audit scope

Written for an outside, independent security audit. Revision described:
`fc09dfe90e0170e422e44febb6de08c7f0c3a460` on `main`, 2026-10-26. Everything below is taken from the repository's own files at
that revision; items marked NOT_RUN were not executed while writing this document.

## What this repository is

A Rust HTTP server (axum) that implements the translate.js endpoints (`/translate.json`,
`/language.json`, `/connectTest.json`, `/init.json`, `/health`; `/ip.json` is not offered) by
sending text to an OpenAI-style chat model server (`GET /v1/models`, `POST
/v1/chat/completions`). Every translation passes a guard before it is returned, and results are
saved in a SQLite cache. Source: `src/main.rs` (flags), `src/lib.rs` (routes, admission,
429 lines), `src/engine.rs` (model client, 362), `src/guard.rs` (141), `src/cache.rs` (69),
`src/langs.rs` (31 languages). Python clients in `scripts/` translate a README, a built site, or a
LaTeX paper through a running server. `docs/i18n/README.*.md` are machine translations. Licence
AGPL-3.0-only.

## What to review, in priority order

1. Admission and authentication in `src/lib.rs`: `Admission::new`, `authorized` (length check
   plus `subtle::ConstantTimeEq`), `allow_client` (per-IP quota, `CLIENTS_MAX` 1024),
   `DefaultBodyLimit::max(MAX_REQUEST_BYTES)` (16,384), the query length check in `translate_h`,
   and which routes require the token (README: `/translate.json` and `/health`).
2. Token file handling in `src/main.rs`: refused if group or other can access it (`mode & 0o077`),
   must be a single line of at least 32 printable ASCII bytes. Without `--auth-token-file` the
   loopback server accepts unauthenticated requests; review what any local process or a browser
   page that can reach the loopback port (cross-site form `POST`, DNS rebinding) can do in that
   mode.
3. `src/guard.rs`: the rules that decide whether model output is served. Model output is
   untrusted; check that HTML or script in a translation cannot reach the page through translate.js.
4. `src/engine.rs`: batching, one request at a time, reply parsing, and the split-and-retry
   fallback when a batch reply cannot be matched to its inputs (`tests/fixtures/` holds recorded
   bad replies).
5. `src/cache.rs`: parameterised `query_row` and `execute`; cache key derivation (sha2); the cache
   file holds visitors' page text (AGENTS.md: never commit it).
6. `params` and `parse_text` in `src/lib.rs`: form, query and JSON text parsing.
7. Python clients `scripts/translate_site.py`, `scripts/translate_latex.py`,
   `scripts/translate_readme.py` and `demo/check.py`: they write translated files to output
   directories; check path handling of names taken from input sites and papers.

## Trust boundaries

- Untrusted: HTTP request bodies and query strings; model output; for the scripts, the input site,
  README or LaTeX source.
- Trusted: the reverse proxy in front of the server (README sets its duties: visitor
  authentication, cross-site rejection, per-visitor rate limiting, request size caps, stripping
  client `Authorization`, adding the backend token); the model server named by `--engine`; the
  token file.
- Behind a proxy the backend sees only the proxy IP, so native per-IP quotas do not replace proxy
  quotas (AGENTS.md).

## Threat model summary

- Assets: the model server's capacity (possibly a shared GPU); the backend token; visitors' page
  text in the cache; integrity of the translated page.
- Attackers considered: unauthenticated callers, quota exhaustion, a model reply that drops
  structure or does not match its batch.
- Defended: loopback-only listener, optional constant-time token check, size and count limits,
  per-IP quota, single active slot with 429, guard with fallback to the original text.
- Not defended / explicitly out of scope: public exposure without the proxy contract; README notes
  slow or restarting model requests can hold the single slot for a long time; machine translation
  quality is not reviewed (AGENTS.md: published as machine translation).

## Build from source at a pin

Needs a stable Rust toolchain (version not pinned). SQLite is bundled through `rusqlite` 0.32.
`Cargo.lock` committed (169 packages). reqwest 0.12 uses `rustls-tls` with default features off.

```sh
cargo build --release
./target/release/aien-translate --help
```

## Run every test

NOT_RUN while writing this document.

```sh
cargo test
```

AGENTS.md: unit tests and end-to-end tests against a fake model server, no GPU and no network.
Test attributes at this revision: `src/engine.rs` 9, `src/guard.rs` 7, `src/lib.rs` 3,
`tests/protocol.rs` 13. Expected: `test result: ok` for each target.

Not runnable without external pieces, NOT_RUN: a real server against a model server; the Python
scripts (need a running server; `translate_site.py` needs beautifulsoup4); `demo/check.py` (needs
Playwright and a downloaded translate.js).

## Known limitations and NOT_RUN

- No tracked CI workflow at this revision.
- The guard and admission behaviour are tested against a fake model only.
- Real proxy deployments are outside the repository.
- `translate_readme.py` has no `--help`; unknown arguments are read as language codes (AGENTS.md).

## Dependencies and provenance

- Direct dependencies: axum 0.8, tokio 1, reqwest 0.12 (rustls-tls), rusqlite 0.32 (bundled),
  serde, serde_json, sha2 0.10, regex 1, form_urlencoded 1, subtle 2; dev tokio-stream.
- Python scripts use the system `python3`; beautifulsoup4 and Playwright are optional extras, not
  locked.
- translate.js is not vendored; `demo/translate.js` is downloaded and ignored by git.

## Evidence

No receipts are stored in this repository. AGENTS.md asks that pull requests carry the `cargo
test` result in their body.
