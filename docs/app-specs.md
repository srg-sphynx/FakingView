# FakingView application specifications

These specifications describe the code and verified local macOS build. Individual releases state
their actual assets, signing status and verification results in their release notes.

## Platform and implementation

| Item | Specification |
| --- | --- |
| Verified distribution target | Apple Silicon / arm64 macOS |
| Bundle minimum | macOS 14.0; set in [`tools/bundle.sh`](../tools/bundle.sh) |
| Implementation | Native Rust application with eframe/egui and a macOS AppKit boundary; no Electron shell |
| Source toolchain | Rust 1.95 or newer, edition 2024; Xcode command line tools for macOS |
| Main executable | `fakingview` in a source build; `FakingView` inside the app bundle |
| Bundle | `FakingView.app`, ID `local.fakingview.desktop` |
| Alert helper | `FakingViewAlertAgent.app`, ID `local.fakingview.desktop.alert-agent` |
| Storage | Bundled SQLite plus local script and workspace files; secure credentials through the OS secret store |
| Release signing | Current local preview has no Developer ID or notarization; its Mach-O carries a linker-generated ad-hoc signature |
| Other source targets | Intel macOS, Windows and Linux code/packaging exist; the local verification does not certify those platforms |
| License | GPL-3.0-only application; third-party fonts and components retain their licenses |

There is no published benchmark-backed minimum RAM, CPU or storage specification. Workload cost
depends on the number of charts, history length and scripts. The app requires network access for
external market-data requests, but its rendering, editor and stored workspace are local.

## Indian market-data interface

| Capability | Behavior |
| --- | --- |
| Provider | Personal Zerodha Kite Connect v3 session |
| Login | Browser sign-in followed by manual `request_token` entry and checksum-based token exchange |
| Secret persistence | API key/access-token pair in macOS Keychain; API secret/request token transient |
| Directory | Authenticated instrument CSV, cached in memory for the Indian calendar day |
| Identity | Exchange plus trading symbol, such as `NSE:INFY`; persisted identity is not a recycled numeric token |
| Exchanges | NSE, BSE, NFO, BFO, CDS, BCD, MCX and NSEIX entries present in the current directory and available to the account |
| Chart data | Historical REST OHLCV, paged and normalized; charts refresh through REST |
| Watchlist prices | Binary WebSocket ticks with REST snapshots as fallback |
| Quote comparison | Previous close; share volume where the provider supplies it, with derivative contracts not mislabeled as shares |
| Intraday aggregation | Minute bars folded onto the app's UTC bucket boundaries |
| Daily aggregation | Indian calendar date represented as a UTC date label before weekly/monthly/longer folds |
| Per-request history bound | Up to 30,000 base candles over at most 20 pages; fewer displayed bars are possible |
| Historical pacing | Shared lane with at least 350 ms between requests |
| Quote snapshots | Up to 500 instruments with at least 1,050 ms between requests |
| WebSocket demand | One shared Kite quote socket for the app's demanded markets, up to the protocol's 3,000-instrument connection limit |
| Execution | Read-only market data; no Kite orders, INR portfolio valuation or INR paper-account fills |

Endpoint access, symbol availability, rates and subscriptions remain under Kite's control. A
fixture test or successful `/user/profile` request does not establish a live market-data entitlement.
See [implementation details](kite-connect.md) for exact parsing, paging and failure behavior.

## Charting and analysis

The [source chart-type enum](../src/model/chart/kind.rs) contains 18 styles:

| Standard OHLC | Lines and areas | Constructed bars |
| --- | --- | --- |
| Candles | Line | Heikin Ashi |
| Hollow Candles | Line with Markers | Renko |
| Bars | Step Line | Line Break |
| High-Low Bars | Area | Kagi |
| Volume Candles | HLC Area | Point & Figure |
| — | Baseline | Range |
| — | Columns | — |

The workspace includes 80+ drawing tools, chart overlays, saved layouts, bar replay, custom
timeframes, export/snapshot tools and a searchable indicator catalog. Twenty bundled Pine
indicators supplement the native studies; availability depends on the chart's data type.

Timeframes accept minutes, hours, days, weeks and months. Examples include `1m`, `22m`, `3h`,
`2D`, `1W`, `3M` and `1Y`; seconds are not supported. Providers either supply or construct the
requested timeframe from compatible bars, or report it as unsupported. See
[timeframes](../src/model/market/timeframe/mod.rs) and [chart types](chart-types.md).

## Pine and automation

Pine Studio includes a local script library, syntax-aware editor, preview chart, debugger and
profiler. Kite symbols resolve in chart requests and preview selection. Their metadata reports
INR and `unknown` instrument kind; Indian session calendars and contract specifications are not
inferred from a name. Pine runtime compatibility and strategy semantics have their own documented
limits in the [Pine architecture](architecture/pine.md).

The optional MCP interface defaults to loopback port **8765**, is off by default, and separates
35 read/write capabilities in its permission model. Destructive operations and paper orders can
require app confirmation. Live broker order execution is not exposed through this interface.
Remote access is a separately configured TLS and authorization feature; it is not enabled by
installing the app. See [MCP documentation](mcp-implementation.md).

## Verification boundaries

The release verification has 7,841 passing tests, 131 ignored optional tests, clean
Clippy/format checks and reviewed application galleries. The earlier development bundle was
also launched natively and inspected with REA.

Live Kite credentials, exchange entitlements, market-hours ticks, Indian holiday calendars,
Developer ID/notarization and other operating systems were not established by that local run.
The [verification record](verification.md) and
[architecture review](reviews/kite-integration-audit.md) retain the evidence and limitations.

## Screenshot provenance

The README chart and Pine Studio images are rendered through the application's actual egui
views with deterministic sample candles. Each frame carries a sample-data label; neither is
evidence of an authenticated live account or a particular exchange price. The Kite settings
image uses the same application's form renderer with empty credential fields.

To reproduce the two publication frames from source:

```sh
FAKINGVIEW_GALLERY_DIR="$PWD/assets/screenshots" \
  cargo test --locked --lib fakingview_publication_screenshots -- --ignored --nocapture
```

The fixture lives in [`publication_gallery.rs`](../src/pine_studio/publication_gallery.rs).
