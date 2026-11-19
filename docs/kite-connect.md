# Kite Connect Indian market data

FakingView adds Kite Connect as a read-only market-data provider. Search for Indian exchange
instruments, open historical OHLCV charts, and stream current watchlist prices using your own
Zerodha Kite Connect session. Prices are in the instrument's native INR units. This integration
does not submit orders. Kite instruments are excluded from the app's USD portfolio and paper-trade
paths until those paths support INR and contract specifications correctly.

The implementation builds on the upstream GPL-3.0-only DegenView project. The local application is
named FakingView and uses its own application identifier, storage directory and credential scope.
See [the integration audit](reviews/kite-integration-audit.md) for the repository architecture,
review boundaries and remaining verification requirements.

## Start the local macOS application

Requirements: macOS, Xcode command line tools, and Rust 1.95 or newer. Build the Finder app from the
repository directory:

```sh
./tools/bundle.sh
open target/FakingView.app
```

For a development run, use `cargo run --locked --bin fakingview`. The bundle script builds the main
application and its background-alert helper with the committed dependency lockfile. A local
unsigned bundle supports foreground alerts; background Login Items, distribution signing and
notarization require the separate Apple setup described in [BUILDING.md](../BUILDING.md).

## Configure a daily session

1. Create a personal application in the [Kite Connect developer console](https://developers.kite.trade/).
   Its account and subscription must permit the market-data endpoints you intend to use. A
   successful profile test alone does not establish historical or streaming data entitlement.
2. Open FakingView **Settings → Kite Connect** and enter the application's API key.
3. Select **Sign in through Kite**. Complete Zerodha's login in your browser using your own
   account. Authentication redirects to the URL configured for that Kite application.
4. Copy the `request_token` value from that redirect URL. In FakingView, enter the API secret and
   the request token, then select **Create daily session** promptly. Request tokens are short-lived
   and single-use; after a failed exchange, a new login may be needed.
5. Select **Test session**. Open the symbol picker and search the Kite source for `NSE:INFY`,
   `BSE:INFY`, `NSE:NIFTY 50`, a company name or another listed instrument.

If you already obtained an access token through Kite's supported login flow, enter the API key and
access token directly. Fields save automatically. **Remove credentials** disconnects the saved
session. Kite access tokens expire daily and can be invalidated by logout or revocation; sign in
again when the app reports an expired session. FakingView does not automate passwords, two-factor
authentication or session renewal.

The API key and access token are kept in the platform's secure credential store (Keychain on macOS),
never in the SQLite database or a configuration file. The API secret and request token are used
only for the token-exchange request and are not saved. API response errors do not display response
bodies that might echo a secret. A refused Keychain write is reported as a session-only connection.

Developers may supply a complete environment pair when no saved session takes precedence:
`FAKINGVIEW_KITE_API_KEY` and `FAKINGVIEW_KITE_ACCESS_TOKEN`. Set these privately in the launching
process; Finder does not inherit variables from a terminal shell. Do not commit credentials or put
real tokens in documentation or shell scripts. `FAKINGVIEW_NO_KEYCHAIN=1` uses in-memory credentials
for development and is not a replacement for persistent secure storage.

## Instrument identity and exchange coverage

Saved charts and watchlists use the stable textual key `exchange:tradingsymbol`, such as
`NSE:INFY`. The NSE and BSE listings of the same company remain separate instruments. Index names
with spaces and symbols containing punctuation are URI-encoded when sent to Kite.

The authenticated `/instruments` CSV directory supplies current exchange tokens for NSE, BSE, NFO,
BFO, CDS, BCD, MCX and NSEIX entries it lists. Availability still depends on your account, the
exchange and the instrument. The directory is downloaded on demand, shared across searches and
charts, and refreshed after the Indian calendar day changes. No stale disk fallback is used:
derivative tokens can be recycled after expiry. Instrument tokens are routing addresses, not
persisted market identities. Expired contracts that no longer appear in today's directory cannot
be resolved by this integration; continuous futures history is not requested.

## Charts, prices and time

Historical candles come from `/instruments/historical/{instrument_token}/{interval}`. Charts refresh
through REST. Watchlist prices use Kite's binary WebSocket protocol with a REST snapshot fallback;
ticks are not fabricated into completed OHLC candles. Chart candles therefore update on the app's
REST refresh schedule, not on every watchlist tick.

FakingView has a shared UTC aggregation policy. Kite's larger intraday bars can begin at the
exchange session open (for example 09:15 IST), so requesting Kite's 60-minute bars would shift the
app's hourly buckets. The integration instead fetches one-minute bars and folds them through the
existing chart aggregation engine. Intraday timestamps retain their true instant:
09:15 IST is 03:45 UTC. An hourly chart consequently uses UTC hourly boundaries; its first visible
session bucket can contain a shorter session fragment. This is deliberate and differs from a
broker chart anchored at the session open.

For daily and longer timeframes, Kite's daily bar is used. The Indian trading date is represented
as that calendar date at UTC midnight before weekly, monthly, quarterly and yearly folds. It is a
calendar label, not a claim that the trade happened at midnight UTC. This prevents a Monday or
first-of-month IST candle from being assigned to the preceding Sunday or month. Days without a
trading session remain gaps; the provider does not manufacture flat bars for holidays or weekends.

Quotes compare the current price with Kite's previous closing price, not a rolling 24-hour price.
Share volume is shown for cash equities where available. Derivative volume is omitted from the
watchlist because the current quote model has no contracts unit; it must not be mislabeled as
shares. Missing instruments, prices, references and timestamps stay missing rather than becoming
zero. Holiday and session-state accuracy is limited by the app's calendar model and the data Kite
returns; a received snapshot is not proof that a trade just occurred.

## Request limits and failure behavior

Historical requests share a process-wide lane spaced at least 350 ms apart. Snapshot quote requests
share a separate lane spaced at least 1,050 ms apart and carry at most 500 instruments. Historical
pages span at most 60 days of minute data or 2,000 days of daily data. One chart request gathers at
most 30,000 base candles across 20 pages. Long or sparse requests can therefore return fewer than
the requested number of displayed candles. The timeframe is never silently replaced with a
different one.

HTTP 429 remains a typed rate-limit error for the existing backoff machinery. Authentication
refusals and Kite's `TokenException` become an expired-session error. A permission refusal names
data access as the issue. Malformed JSON, CSV, candle shapes and timestamps fail explicitly instead
of looking like a valid empty market. Token exchange is a single POST without automatic retry,
since a lost response may still have consumed the one-use request token.

HTTP and WebSocket connections use FakingView's existing proxy routing. Response sizes and page
counts are bounded. Historical and snapshot payloads have separate limits; the instrument CSV has
a larger 64 MiB limit because it is a full exchange directory.

## Verification

Deterministic tests cover quoted and reordered CSV fields, exchange identity, malformed and
overflowing tokens, explicit timezone offsets, invalid dates, Indian trading-date calendar folds,
UTC intraday folds, historical query boundaries, previous-close quotes, missing prices, session
checksum/form encoding, local HTTP token exchange, binary tick parsing, credential storage and
Settings states. Run the repository's normal checks from the root:

```sh
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Protocol fixtures and loopback tests do not establish live account entitlement or a successful
market-hours stream. Final end-to-end verification needs your own valid Kite session and data
subscription: test the profile, search the instrument directory, load one minute and one daily
chart, add a watchlist symbol, observe quotes during its exchange session, and confirm that an
expired token produces the renewal message. No account credentials are bundled with the app.

## Primary protocol references

The integration was checked against these official Kite Connect v3 references:

- [Login and session flow](https://kite.trade/docs/connect/v3/user/)
- [Market quotes and the instrument CSV](https://kite.trade/docs/connect/v3/market-quotes/)
- [Historical candles](https://kite.trade/docs/connect/v3/historical/)
- [WebSocket binary protocol](https://kite.trade/docs/connect/v3/websocket/)
- [Exceptions and rate limits](https://kite.trade/docs/connect/v3/exceptions/)

Subscriptions, endpoint permissions and exchange rules are controlled by Kite and can change.

## Analysis metadata

Pine charts and alert subscriptions identify Kite prices as `INR`. Instrument kind is
`unknown`: a Kite source includes shares, indices, options, futures and currency instruments,
and the exchange-qualified symbol alone does not establish the contract type. Exchange calendars,
contract multipliers, tick/lot metadata and Indian session-specific Pine semantics are not yet
modeled. Session statistics retain the app's explicitly labeled UTC-day grouping. Scripts relying
on instrument type, trading sessions, currency conversion or strategy-account sizing require
separate validation before their results are used.
