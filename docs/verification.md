# Verification record

FakingView `0.1.0-beta.1` was checked on an Apple Silicon Mac on 19 November 2026
with Rust 1.100.0 and the committed dependency lockfile. This record describes the
checks performed and their limits; it is not a guarantee that every feature or
external service is free of defects.

## Source checks

| Check | Result |
| --- | --- |
| `cargo test --locked` | 7,841 passed, 0 failed, 131 ignored |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Passed |
| `cargo fmt --all -- --check` | Passed |
| `git diff --check` | Passed |
| Publication screenshot gallery, run explicitly with `--ignored` | Passed; chart and Pine Studio images inspected |
| Release scripts and workflow definitions | Shell syntax, version preparation/rollback, packaging contract and YAML checked |

Ignored tests include visual galleries and optional environment-dependent checks;
they are not counted as passing. HTTP and WebSocket integration tests use loopback
mock servers, not a brokerage account. CI repeats source checks on macOS and Linux;
its current outcome is visible in the repository's Actions tab.

## What was exercised

- Kite CSV catalog parsing, quoted fields, response limits, stable instrument
  identities, daily refresh and failure handling without reusing stale tokens.
- Request-token exchange, credential invalidation, redacted errors and proxy-aware
  transport routing. API secrets and request tokens are not persisted.
- Historical paging, rate gating, Indian timestamp parsing, calendar-day labels,
  bounded intraday folding and partial-range handling.
- Quote batching, previous-close changes, binary tick packet lengths and price
  divisors, reconnect behavior and aging based on exchange timestamps.
- Provider routing through search, watchlists, chart notices, Pine and MCP;
  rejection of Kite instruments from the existing USD paper-account model.
- INR chart headers, settings layout, credential-free screenshot fixtures and Pine
  overlays. The publication gallery uses the real application views and labels its
  deterministic candles **SAMPLE DATA · NOT LIVE**.

The earlier development bundle was launched natively and inspected for its arm64
Mach-O executable, bundle identities, embedded resources and signing status using
REA. The release is packaged from the tagged source commit; its downloadable
application, corresponding source and SHA-256 manifest are attached to the
[release](https://github.com/srg-sphynx/FakingView/releases/tag/v0.1.0-beta.1).
The release is not Developer ID signed or notarized. Automatic in-app binary
replacement is disabled.

## Checks that remain account- or platform-dependent

No live Kite account credentials were supplied for this work. Actual login,
exchange entitlements, market-hours tick delivery, session expiry and behavior
against the current live service must be accepted with the user's own account.
The mock protocol tests cannot establish those facts. See the
[Kite guide](kite-connect.md) for setup and a live acceptance checklist.

The distributed binary targets macOS 14 or later on Apple Silicon. Intel macOS,
Windows, Linux desktop startup, Apple notarization, background Login Items and
iCloud entitlements were not validated as distribution targets. Indian holidays,
contract specifications and exchange-specific Pine session semantics remain
unmodeled. Chart candles refresh through REST; WebSocket ticks update watchlists.

See the [architecture review](reviews/kite-integration-audit.md),
[application specifications](app-specs.md) and [release guide](releasing.md).
