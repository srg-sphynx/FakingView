# Architecture and Kite Connect integration review

Date: 19 November 2026. Upstream baseline: `5950cbef2091f588ec4a1d950efcb52e2cf5200d`.

This is a source-based architecture review of FakingView, derived from DegenView, and an implementation review of the Indian
market-data integration. It covers every major subsystem at its ownership boundary and follows
the market-data path in detail. It is not a claim that every line, drawing tool, Pine program,
platform or broker has been exhaustively verified. The upstream tree contains approximately
1,240 source/assets files and more than 500,000 lines; a test suite and a review provide evidence,
not a proof that a program has no bugs.

## Evidence and method

The repository contains the implementation, so source inspection is the authoritative starting
point. The REA skill directs ordinary source-repository analysis to normal repository tools;
native artifact inspection is a separate build-verification step. Source observations below are
identified by file path. Protocol behavior comes from Zerodha's primary documentation:

- [WebSocket protocol](https://kite.trade/docs/connect/v3/websocket/)
- [Quotes and instrument directory](https://kite.trade/docs/connect/v3/market-quotes/)
- [Historical candles](https://kite.trade/docs/connect/v3/historical/)
- [Authentication](https://kite.trade/docs/connect/v3/user/)
- [Desktop authentication guidance](https://kite.trade/docs/connect/v3/apps/)
- [Official Python tick decoder](https://github.com/zerodha/pykiteconnect/blob/master/kiteconnect/ticker.py)

Observations are from code and documented wire formats. Design choices include keeping Kite
read-only and retaining REST as the chart-candle authority. Live exchange operation, credentials,
entitlements and venue calendars need separate runtime evidence.

## Whole-application map

| Subsystem | Responsibility and evidence | Implication for Kite |
| --- | --- | --- |
| Startup and native shell | `main.rs`, `lib.rs`, `native_bridge.rs`: Rust/eframe entry points, AppKit boundary, hidden coordinator viewport, application windows | Build the existing native application; a web wrapper is unnecessary |
| Coordination | `app/`, `host/`, `workspace/`: app-wide services, per-feature hosts and per-tab chart state | Add services below hosts, keep blocking network/Keychain work off frames |
| Domain model | `model/market/`, `model/watchlist/`: source identity, intervals, candles, instrument keys, quote freshness | Register a source and preserve exchange-qualified identity |
| REST market services | `service/mod.rs`, `service/market/`: provider trait, routing, search, candle fetch/normalization | Implement Kite behind the existing provider trait |
| Streaming | `service/market/live/`, `service/watchlist_quotes/`: complete-candle streams, shared quote sockets, transient quote book | Kite binary ticks belong to quote streaming; they do not contain complete intraday candles |
| Networking | `service/net/`: HTTP retries/limits, following proxy agents, WebSocket tunnels, health and backoff | Reuse proxy-aware transport and connection lifecycle |
| Caches | `service/cache/`, `service/market/catalog.rs`: transient candles and regenerable catalogs | Refresh Kite tokens daily in IST; never use an expired token directory as identity |
| Charting | `ui/chart/`, `model/chart/`, `workspace/chart/`: painter-based rendering, viewport and fetch lifecycle | Existing chart types and indicators consume normalized historical candles |
| Analysis and drawings | `model/indicators/`, `model/drawings/`, other drawing domains and `ui/`: technical indicators, geometry and interaction | Data-source integration should not change drawing or indicator execution semantics |
| Pine | `pine/`, `host/pine/`, `pine_studio/`: lexer/parser/compiler/runtime, requests, strategy engine, debugger, profiler, editor | Kite history is available through provider routing; Pine semantic conformance remains its own test surface |
| Alerts and webhooks | `model/alert/`, `host/alert/runtime.rs`, `service/webhook/`, login-item binary | Quote demand shares the coordinator; credentials and alert delivery permissions are separate |
| Portfolio | `model/portfolio/`, `host/portfolio/`, `service/portfolio/`: transaction ledger, FX projection and shared quote consumption | INR settlement must be represented explicitly before using Indian prices for portfolio valuation |
| Trading | `trading/`, `model/paper/`, `host/broker/`: common trading backend, paper ledger, external pinned broker crate | This integration does not add Kite order placement; INR-unaware paper paths must not silently accept Kite instruments |
| Persistence | `store/`: SQLite documents/settings and file-based scripts; transaction-first publication | Credentials belong only in secure storage; existing saved schemas should continue decoding |
| Sync | `sync/`, `store/sync/`, `native/swift/`: pure merge policy, outbox, CloudKit bridge | Market-data credentials remain local and outside sync |
| MCP | `mcp/`, `host/mcp/`: authenticated transport, grants, app-thread bridge, bounded tools | Expose data through existing capabilities; a market source must not imply new order permissions |
| Updates and packaging | `service/update/`, `tools/bundle.sh`, `packaging/`, workflows | A locally renamed fork must not replace itself with an upstream binary; local bundle signing differs from notarized releases |

The intended dependency direction is `model` → `service/store` → `host/workspace/app` → `ui`
where arrows mean “is consumed by.” `layout_tests.rs` checks layer violations and directory
layout. Neither the source map nor those tests prove complete runtime independence.

## Data flow and integration decisions

1. Settings or developer environment supplies an API key and daily access token. The first secret
   read and authentication requests run on workers. API secret and request token are temporary
   inputs to the checksum exchange, not persisted settings.
2. An authenticated instrument-directory request resolves `EXCHANGE:TRADINGSYMBOL`. The stable
   saved identity retains exchange and trading symbol. An instrument token is used only as a
   current routing address because derivative tokens may be reused.
3. Searches rank the cached directory locally. Historical fetches follow the source's declared
   native intervals and the existing timeframe planner, then pass through shared normalization.
4. Visible watchlists, alert consumers and other quote owners contribute demand to the same
   coordinator. Kite has one shared connection for up to 3,000 mapped instruments. Additional
   instruments remain eligible for REST fallback. The application does not open a socket per chart.
5. The stream subscribes in full mode, decodes all packets within each binary message, associates
   tokens with current instrument identities, and publishes `SourceQuote` values. Text order
   postbacks are ignored by this read-only market-data adapter.
6. The shared WebSocket session handles TLS/proxies, heartbeat health and bounded reconnect
   backoff. A session-validity callback ends an authenticated session after credentials change
   or the IST date rolls over; the owner then re-resolves tokens and creates a fresh URL.
7. Chart history is refreshed through REST. Starting a socket halfway through a minute cannot
   reconstruct the minute's true open/high/low or traded volume. Treating a tick as a complete
   candle would corrupt indicators, so Kite is not advertised as a complete-candle stream.

Historical time stamps include or imply IST and must be converted to epoch milliseconds once.
The application folds custom intervals on its documented UTC bucket grid; the provider must
only advertise native intervals that are compatible with that grid. Session-anchored exchange
bars must not be passed off as UTC-hour bars. Session days and higher folds need documented
normalization. Holidays, special sessions, auctions and exchange-specific calendars are not
established merely by converting a timezone.

## Findings and controls

| Finding | Evidence | Control or remaining action |
| --- | --- | --- |
| Price scaling differs by market segment | Official Python decoder uses token low byte; CDS, BCD and other segments have different divisors | Decode CDS with 10,000,000, BCD with 10,000 and remaining segments with 100; deterministic fixtures cover all three |
| A frame may contain many instruments | Official binary envelope has count and per-packet length | Decode every bounded packet, reject truncated/trailing frame bytes, skip unknown packet shapes |
| Index packets differ from tradable instruments | Official 28/32-byte index and 44/184-byte tradable layouts | Parse the correct offsets; no invented index volume |
| Full quotes include exchange timestamps | Official index offset28 and tradable offset60 | Preserve exchange time, fall back to last-trade time only when exchange time is absent/zero |
| Quote reference is previous close, not rolling24h | Kite OHLC close and existing `ChangeBasis` | Register Kite as previous-close basis and label it accordingly |
| Session quantity is not rolling dollar volume | Kite stream day-volume field; existing volume units | Equity volume is shares; derivative contract volume is omitted until a correct unit is supported |
| Credentials in the WebSocket query are unavoidable under the documented API | Official authentication URL | Percent-encode components, never print the URL or raw network response; keep TLS validation enabled |
| Reconnect with a stale URL can retain an expired session | `net::ws::Session` originally held a fixed URL for its whole lifetime | Generic validity guard exits before another read/connect after rotation; Kite rebuilds with current credentials |
| Reused derivative tokens can point at another contract | Official market-quotes guidance, daily directory design | Never persist token as identity or continue yesterday's token map; re-resolve at IST midnight |
| Existing monetary models do not include INR | `model/portfolio/mod.rs`, `model/paper/mod.rs`, portfolio auto-mapper defaults | Gate Kite portfolio/paper entry or add INR and FX handling end to end; never interpret rupees as dollars |
| Old exchange data can arrive in a successful response | Quote freshness previously used only receipt time | Kite freshness now uses the exchange/trade timestamp capped at receipt time; old or future-dated payloads cannot refresh an old quote just by arriving again |
| Source-wide tests cannot establish stock exchange calendar correctness | India has exchange-specific sessions and holidays | Do not guess a single India-wide opening calendar; stale data is labelled stale, while special sessions and holidays remain a live acceptance concern |
| A renamed fork can accidentally use upstream identity/update infrastructure | Package names, store paths and update service | Review app/binary/bundle/data/Keychain identities and disable upstream self-update for the local fork |

## Test design and evidence boundary

Deterministic tests are added alongside the implementation. The binary decoder fixtures cover
multi-packet delivery, all supported packet sizes, price divisors, previous-close offsets,
equity volume, index volume absence, exchange and trade timestamps, one-byte heartbeats,
unsupported packet lengths, zero tokens/prices, truncation at every boundary, oversized packet
counts, trailing bytes and arbitrary short binary inputs. The shared session guard has loopback
tests for invalidation before connecting and between reads. The existing socket suite covers
subscription delivery, heartbeat timeouts and proxy generation changes.

The independent REST review found and fixed a historical coverage issue: reaching a page limit
with fewer than the requested number of bars could retain an incomplete first folded bucket.
The pager now carries the earliest covered instant, enforces each page's exclusive upper bound,
and discards an unestablished first bucket even when it is the sole returned bucket. Tests also
verify that a legitimate non-trading gap before a session open does not discard a complete
session's first hour. OHLC ranges that exclude open/close are rejected instead of silently
repaired. CSV field/record/column limits bound malformed inputs, and duplicate header names fail.
Loopback HTTP contract tests check authentication headers, rate/auth failure types and response
body limits. The login checksum has an independently computed SHA-256 golden value.

REST/authentication tests should exercise CSV quoting and changing headers, exact symbol identity,
authentication envelopes, checksum exchange, typed failure mapping, time conversion, interval
folding and quote parsing. Shared regression tests cover saved model decoding and surrounding
behavior. Formatting and Clippy catch a different class of defects than protocol fixtures;
both are useful, neither substitutes for runtime tests.

The parent build/test report is the record of which commands actually passed. Tests listed here
describe implemented test cases, not an assertion that a particular run passed. No live Kite
session is claimed without the user's enabled Kite Connect app, valid account session and
appropriate market-data access. A live acceptance pass must verify login/expiry, instrument
search, NSE/BSE distinction, prices against the account terminal, market-open updates, quiet
market heartbeats, reconnect, credential rotation, REST history, custom intervals and graceful
permission/rate-limit handling. macOS bundle launch and visual inspection are separate checks.

## Residual scope

Kite order entry, positions, holdings, GTT and order modification are outside this read-only
integration. Full exchange calendars, option-chain analytics and guaranteed tick retention are
also outside its claims. Chart candles remain REST-refreshed; watchlist quotes are streamed.
Existing Pine semantics, every drawing interaction, broker implementations and all non-macOS
platforms have not been re-certified by this integration review.
