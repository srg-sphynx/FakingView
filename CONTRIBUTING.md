# Contributing to FakingView

FakingView is a native Rust market workspace with a read-only Kite Connect data integration.
Start with [AGENTS.md](AGENTS.md), [architecture](docs/architecture.md) and the guide matching the
subsystem you want to change.

## Development setup

Use Rust 1.95 or newer and the platform's native build tools. macOS builds require the Xcode
command line tools. Clone this repository and preserve the committed lockfile:

```sh
git clone https://github.com/srg-sphynx/FakingView.git
cd FakingView
cargo test --locked
cargo run --locked --bin fakingview
```

For repeated development launches, `FAKINGVIEW_NO_KEYCHAIN=1` can avoid secure-storage prompts.
It is a development setting, not a persistent credential store. Never add real keys to source,
fixtures, screenshots, command examples, test logs or issue reports.

## Before opening a pull request

```sh
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Read the complete diff and exercise the affected UI. Use deterministic protocol fixtures and
loopback mock servers for data-provider changes. A test can require local loopback networking;
an environment that refuses socket binding is not evidence of a provider bug.

The community Pine corpus is optional and separately enabled with `cargo test --locked --features
corpus`. Visual gallery tests are ignored by default and write only when a gallery directory is
configured. Do not claim a live integration passed unless it was actually exercised with a valid
account, entitlement and relevant market session.

## Design rules

- Keep pure domain logic under `model/`; keep I/O in services and coordination in hosts/workspaces.
- Network and secure-storage work belongs off the UI frame. Reuse existing work queues, proxy
  routing, typed errors and reconnect behavior.
- Preserve provider identity and data semantics. Do not substitute a nearby timeframe, invent a
  missing price or label session volume as a different unit.
- Keep application controls neutral and flat; verify light/dark and narrow-window states.
- Preserve licenses, copyright notices and third-party provenance. Product independence does
  not remove the redistribution obligations in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

Describe the concrete problem, final behavior and validation in your pull request. Include
before/after screenshots for visible changes and state any unverified platform or live-service path.

## Bugs and security

For a bug, include version, OS/architecture, reproduction steps, expected/actual behavior and a
minimal redacted example. Never upload credentials or personal databases. Follow
[SECURITY.md](SECURITY.md) for security reporting and [the release guide](docs/releasing.md) for
release preparation.
