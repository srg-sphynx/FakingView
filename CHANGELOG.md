# Changelog

## 0.1.0-beta.2 — 2026-11-19

- Added read-only INDmoney (INDstocks) and Groww market data with secure token setup and a live quote test.
- Added provider-specific instrument search, INR charts, historical candle paging and live REST watchlist quotes.
- Added both sources to Pine Studio, Pine symbol requests, watchlist import and MCP market tools.

## 0.1.0-beta.1 — 2026-11-19

First FakingView public preview, with an independent application identity and personal Kite Connect
market-data workflow.

### Added

- Personal Kite Connect login and daily request-token exchange in Settings.
- Secure API key/access-token persistence through the platform secret store.
- Indian exchange instrument search, historical OHLCV charts and WebSocket watchlist quotes.
- Stable exchange-qualified identities, daily instrument-token refresh and previous-close changes.
- IST-aware timestamp parsing, Indian trading-date calendar folds and bounded UTC intraday aggregation.
- Typed authentication/rate-limit errors, bounded responses, reconnect invalidation and stale-quote aging.
- Kite routing in watchlists, MCP market access, Pine requests and Pine Studio previews.
- FakingView application name, icons, bundle IDs, storage scope, repository documentation and artwork.

### Scope and known limitations

- Kite supplies market data only; no Kite order execution or INR portfolio/paper-account valuation.
- Chart candles refresh through REST; watchlist prices use WebSocket ticks with snapshot fallback.
- Live account authentication, data entitlement and market-hours streaming require user verification.
- The verified local distribution is macOS 14+ on Apple Silicon, without Developer ID/notarization.
- Indian holiday calendars, contract specifications and exchange-specific Pine session semantics remain unmodeled.

The source provenance and copyright notices are retained in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Each published release records its artifacts,
checksums, source commit and final validation results.
