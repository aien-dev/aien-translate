# Security policy

## Reporting a vulnerability

Report privately. Do not open a public issue, pull request or discussion about it.

1. GitHub: open this repository's Security tab and choose "Report a vulnerability". This
   creates a private advisory that only the maintainers can see. If that option is not shown,
   private vulnerability reporting has not been enabled for this repository yet; use email.
2. Email: drake@aienos.com with the subject `[aien-translate] security report`.

Include the commit or release you tested, the component (path), steps to reproduce, and what an
attacker gains. If the report involves a secret, say where it was exposed and do not send the
secret itself. Encrypted mail is not required; do not post proof-of-concept payloads publicly.

## Response targets

This project has one maintainer; the targets below are best effort and stated honestly.

- Acknowledgement: within 3 working days.
- Triage and severity: within 7 days of acknowledgement.
- Fix or mitigation for confirmed issues: aim for 30 days for high and critical, 90 days
  otherwise, with a status update at least every 14 days until closed.
- Disclosure: coordinated. An advisory is published with credit (unless you ask otherwise)
  once the fix is on the default branch. Default embargo 90 days, negotiable.

## Scope

`aien-translate` is a Rust HTTP server that answers the translate.js protocol by sending page text
to an OpenAI-style chat model server, checks every translation with a guard, and caches results in
SQLite. `scripts/` and `demo/` hold Python client tools.

In scope:

- A request that reaches the model or cache without the bearer token when `--auth-token-file` is
  set, or a token comparison that leaks the token.
- The server listening on a non-loopback address (`src/main.rs` refuses this).
- A bypass of admission limits in `src/lib.rs`: 16 KiB body, 40 texts and 3,000 characters per
  request, six translation requests per minute per directly connected IP, 1,024 tracked IPs, one
  active translation with HTTP 429 instead of queueing.
- A translation that passes the guard (`src/guard.rs`) while dropping or altering a link, email
  address, backticked code span, placeholder, keep-list name or inline tag marker, or that carries
  markup able to run script in the visitor's page.
- Cache poisoning: one visitor's request causing another visitor to receive attacker-chosen text.
- SQL injection in `src/cache.rs`.

Out of scope:

- translate.js itself (not in this repository), the model server, the reverse proxy that README
  requires for public traffic, and the host operating system.

## Supported versions

Only the default branch `main`; there is no release (Cargo version `0.1.0`). Reports against older commits are welcome but fixes land on the default branch only.

## Status

No hosted CI workflow is tracked at this revision. No certification, no bug bounty. See AUDIT_SCOPE.md for what to review and how to build and test it.
