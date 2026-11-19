# AGENTS.md — FakingView Rust

Guidance for AI coding agents working in this repository.

## Required reading

Always read [Core guidance](docs/agent-guides/core.md), then every guide whose scenario matches:

| Scenario | Read |
| --- | --- |
| Providers, symbols, timeframes, prediction markets, caches, icons, live data, or watchlists | [Market data](docs/agent-guides/market-data.md) |
| Live trading, brokers, broker credentials, or Settings ▸ Brokers | [Live trading](docs/broker-trading.md) |
| The symbol picker (⌘K, Add Chart and a watchlist's plus: markets, prediction markets, indices and portfolio cards), symbol search ranking, or how markets are named | [Symbol picker](docs/symbol-picker.md) |
| Any UI: a new or restyled window, sheet, panel, card, sidebar, icon, menu, button, link or dialog | [UI style](docs/agent-guides/ui-style.md) |
| Chart rendering, price labels, chart types, canvas, templates, drawings, portfolio cards, or snapshots | [Charts](docs/agent-guides/charts.md) |
| The cursor menu (Cross, Dot, Arrow, Demonstration, Eraser), the crosshair's parts, the pointer shape over a plot, or the values tooltip on long press | [Chart cursors](docs/chart-cursors.md) |
| The Screenshot Studio, Select portion, snapshot compositions or backgrounds, or the camera menu | [Chart snapshots](docs/snapshots.md) |
| Markets compared on a chart (Compare, overlays), their data, scales, legend or the picker's compare mode | [Chart overlays](docs/chart-overlays.md) |
| The Portfolio tab: Overview, Holdings, Transactions, Statistics, their empty states, or portfolio revaluation | [Portfolio page](docs/portfolio-page.md) |
| Text, note, table, image or post drawings | [Annotation tools](docs/annotation-tools.md) |
| Chart patterns, Elliott waves or cycle drawings | [Chart patterns, Elliott waves and cycles](docs/pattern-tools.md) |
| Position, forecast, volume profile, VWAP or measurement drawings | [Forecasting, volume and measurement tools](docs/forecast-tools.md) |
| Fibonacci or Gann drawings, level lists, fans, circles, arcs, spirals or Gann angles | [Fibonacci and Gann tools](docs/fibonacci-gann-tools.md) |
| Session statistics: gap fills, prior-day breaks, weekday effects, `fakingview_get_session_stats` | [Session statistics](docs/session-stats.md) |
| Pine language/runtime, Pine Studio, editor behavior, or example scripts | [Pine and scripts](docs/agent-guides/pine.md) |
| Adding an indicator to a chart: the toolbar's Indicators button, the indicator catalog, or the Pine indicators bundled with the app | [Indicator catalog](docs/indicator-picker.md) |
| User-defined functions or methods, overloads, call resolution, or the Pine type checker | [User-defined methods](docs/pine-user-methods.md) |
| Editor colours, syntax roles, editor themes or fonts | [Editor themes](docs/editor-themes.md) |
| Colour swatches in the Pine editor, the inline colour picker, or `color.*` rewriting | [Pine colour picker](docs/pine-color-picker.md) |
| `request.footprint()`, `footprint` or `volume_row` | [Pine volume footprints](docs/pine-footprint.md) |
| The Pine debugger: breakpoints, stepping, watches, the interpreter's debug hooks, `pine/debug/` | [Pine debugger](docs/pine-debugger.md) |
| The Pine profiler: the recorder, regions, profile sessions, the profile column and tab | [Pine profiler](docs/pine-profiler.md) |
| The top toolbar: its buttons, labels collapsing to icons, undo/redo, replay or chart-type controls | [Workspace toolbar](docs/workspace-toolbar.md) |
| The AI assistant, LLM providers, or the AI API keys | [AI assistant](docs/ai-assistant.md) |
| Price/Pine alerts or webhooks | [Alerts and webhooks](docs/agent-guides/alerts-webhooks.md) |
| Tab Linking: colour-tagged tabs that share symbol, interval, time or date range across windows | [Tab Linking](docs/tab-linking.md) |
| Proxies, VPN "system proxy", `HTTP(S)_PROXY`, building an HTTP agent, WebSocket connections, or Settings ▸ Network | [Network proxy](docs/network-proxy.md) |
| Tabs, windows, workspaces, saved layouts, session state, or app-wide state | [Tabs and state](docs/agent-guides/tabs-state.md) |
| MCP server, tools, permissions, or clients | [MCP](docs/agent-guides/mcp.md) |
| Releases, signing, packaging, self-update, or update UI | [Releases and updates](docs/agent-guides/releases-updates.md) |
| The Flatpak, Linux packaging, or the sandbox | [Flatpak](docs/flatpak.md) |
| Persisting user data: SQLite tables, `setting` keys, saved state, per-Mac IDs | [Storage and sync](docs/agent-guides/storage.md) |
| iCloud Sync, the sync outbox, or CloudKit | [iCloud Sync](docs/icloud-sync.md) |

For cross-cutting work, load all matching guides. They hold implementation constraints, not
background; never skip one because the code looks self-explanatory.

## Never blue

Links are always gray (`theme::text_secondary()`, underlined); the blue accent is not used for new
UI: no links, buttons or decoration. Details and existing exceptions:
[UI style](docs/agent-guides/ui-style.md).

## Flat style

Flat surfaces: faint fill and hairline; no gradients, lit edges or shadows (except floating menus and
dialogs). Icons bare, no tile, disc or circle. Colour only for state and data. Settings pages have no
bottom bar or Save button: inputs save as entered. Secondary windows keep the platform title bar and
window controls. Reference: the Settings window; rules and helpers in
[UI style](docs/agent-guides/ui-style.md).

## Menus fit their rows

A pull-down is exactly as wide as its longest row. Never guess a width (`set_min_width(180.0)`): use
`kit::fitted_menu` (or `kit::menu_fit`), and look at the opened menu in a picture before shipping.
Details: [UI style](docs/agent-guides/ui-style.md).

## Supply chain and FakingView releases

The active publication target is `srg-sphynx/FakingView`. FakingView uses an independent
version line starting with `0.1.0-beta.1`; retained historical tags are not FakingView releases.
The current package is a manual-download macOS arm64 build without Developer ID notarization.
Do not claim that it is signed by an inherited maintainer or enable automatic installation on
that assumption. Mandatory GPL, copyright and third-party notices stay intact.

When touching `.github/`, `tools/`, `packaging/`, `build.rs`, Cargo files or the updater:

- Pin every external `uses:` to a full commit, keep permissions minimal and use `--locked`.
- Add no git or non-crates.io dependency, and keep `cargo deny check` clean.
- Do not ask for or use a GitHub token with `admin:*`, `admin:ssh_signing_key` or `delete_repo`.
- Keep build/signing credentials out of third-party build scripts. Releases currently need no
  Apple credentials; an unsigned build must never be described as notarized.
- Release only reviewed, validated source from `main`, with the app's matching source archive,
  checksums, license and notices. Published releases and their tags are never moved or replaced.

## Choosing and preparing a release

An explicit publication request authorizes the ordinary preparation needed for that release.
Use the requested version, or choose a prerelease appropriate to the verified state when the
request leaves version selection open. The initial beta records that live Kite account and
market-data entitlements have not been exercised with user credentials.

`Cargo.toml` remains the version source. Prepare it programmatically with
`tools/release.sh prepare VERSION`, previewing with `--dry-run` when useful. That command changes
only the root manifest/lock versions; review and commit those changes before building the
release. Do not bump dependency versions as part of a release or edit/create a release tag by
hand. The publication command or explicit Release FakingView workflow creates GitHub's tag at
its exact validated commit, without a local tag push.

- Fixing an open beta: advance its number, for example `0.1.0-beta.1` to `0.1.0-beta.2`.
- Completing a beta's verified scope: prepare its stable version as a fresh release.
- New features after a stable release: use a new minor version while below 1.0.
- Corrections to a stable release: use a patch version.
- Changes to saved data, account handling or public MCP/Pine interfaces: use a prerelease first.

The definitive procedure is [GitHub publishing](docs/github-publishing.md). The old platform
workflows in `docs/archive/inherited-workflows/` are inactive references, not release steps.
Never invoke a release, upload assets, or change repository settings unless publication is in
the user's authorized task scope. Preparation and local verification are reversible.

## Running tests

The Pine corpus test (`src/pine/tests/corpus.rs`, ~295 community scripts) is most of the ~5 min
unit-test wall-clock, so it sits behind the `corpus` cargo feature, off by default:

- `cargo test` — fast suite, corpus excluded.
- `cargo test --features corpus` — full suite including the Pine corpus.

## Worktrees and build space

Each worktree builds into its own `target/` (5 GB fresh, growing every rebuild; cargo never deletes
old output). Finished worktrees are where the disk goes.

- One worktree per branch, beside the main checkout, building into `target/` or a `target-*`
  directory (both ignored). Never share one `CARGO_TARGET_DIR` between worktrees: concurrent builds
  block on its lock and overwrite each other's `target/debug/fakingview`.
- Remove a worktree when its pull request merges. `tools/clean-worktrees.sh` lists every worktree and
  whether it can go; `--remove-merged` removes finished ones, never one with uncommitted changes,
  newer commits, no pull request, or a process running from it. `--clean-idle DAYS` deletes only the
  `target/debug` of worktrees not built for that long.
- `[profile.dev]` keeps line tables but drops variable-level debug info (about a third of a fresh
  `target/`; the incremental cache is the rest of the growth). To inspect your code's variables in
  lldb, build with `CARGO_PROFILE_DEV_DEBUG=true` into a separate `CARGO_TARGET_DIR`.
- A full disk shows as `rustc-LLVM ERROR: IO failure on output stream: No space left on device`.
  Check `df -h` and run the script first.

## Deeper references

- [Architecture](docs/architecture.md) — module map, data flow.
- [Chart types](docs/chart-types.md) — construction rules, Pine semantics.
- [Chart cursors](docs/chart-cursors.md) — the cursor modes and the capability table that decides what each shows.
- [Chart navigation](docs/chart-navigation.md) — viewport, pan/axis gestures, older history, unverified items.
- [Price scale modes](docs/price-scale.md) — Regular, Percent, Indexed to 100, Logarithmic; reference price, ticks, corner menu, persistence.
- [Linked charts](docs/chart-sync.md) — position/zoom sync between a workspace's charts.
- [Chart canvas](docs/chart-canvas.md) — settings, geometry, layering, templates.
- [Chart context menus](docs/chart-context-menu.md) — right-click menus, Table view, Object tree, what is verified.
- [Drawing tools](docs/drawing-tools.md) — tools, geometry, verified vs inferred behavior, limits.
- [Pine execution-model audit](docs/reviews/pine-execution-model-audit.md) — engine vs TradingView's execution model: findings, stacked fixes, what is deferred.
- [Pine Logs](docs/pine-logs.md) — `log.*()` messages: sink, limits, batches, TradingView verification.
- [Portfolio cards](docs/portfolio-cards.md) — card design, known gaps.
- [New-tab page](docs/new-tab-page.md) — the blank tab: search launcher, tabs, sparklines, animated backdrop.
- [README hero GIF](tools/readme-gif/README.md): how the animated picture is recorded from the real app, and when to retake it.
- [Chart snapshots](docs/snapshots.md) — camera menu, shared renderer, Screenshot Studio (Select portion, composer, backdrops, remembered look), hosted sharing via providers (ImgBB, your own host, adding one), privacy.
- [MCP implementation](docs/mcp-implementation.md) and [remaining work](docs/mcp-remaining-work.md).
- Also in Required reading, with details and what is verified versus decided:
  [Live trading](docs/broker-trading.md), [iCloud Sync](docs/icloud-sync.md),
  [Flatpak](docs/flatpak.md), [Chart overlays](docs/chart-overlays.md),
  [Tab Linking](docs/tab-linking.md), [Annotation tools](docs/annotation-tools.md),
  [Chart patterns, Elliott waves and cycles](docs/pattern-tools.md),
  [Forecasting, volume and measurement tools](docs/forecast-tools.md),
  [Session statistics](docs/session-stats.md), [Fibonacci and Gann tools](docs/fibonacci-gann-tools.md),
  [Symbol picker](docs/symbol-picker.md), [Workspace toolbar](docs/workspace-toolbar.md),
  [Indicator catalog](docs/indicator-picker.md), [Pine colour picker](docs/pine-color-picker.md),
  [Pine debugger](docs/pine-debugger.md), [Pine profiler](docs/pine-profiler.md),
  [Editor themes](docs/editor-themes.md), [User-defined methods](docs/pine-user-methods.md),
  [Pine volume footprints](docs/pine-footprint.md), [AI assistant](docs/ai-assistant.md),
  [Portfolio page](docs/portfolio-page.md), [Network proxy](docs/network-proxy.md).
