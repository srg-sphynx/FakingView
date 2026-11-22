# Building FakingView

The macOS arm64 DMG targets Apple Silicon Macs running macOS 14 or newer. The source also
retains cross-platform code.

## Requirements

- Rust **1.100.0**, matching CI and release builds, with `rustfmt` and `clippy`.
- Xcode command line tools on macOS (`xcode-select --install`).
- A Kite Connect app and eligible daily session only when using Indian market data.

Clone and validate:

```sh
git clone https://github.com/srg-sphynx/FakingView.git
cd FakingView
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Tests use local mock HTTP/WebSocket servers; they do not require broker credentials. The optional
community Pine corpus is a separate developer check (`--features corpus`) and requires its cache.

## Run and package

```sh
cargo run --locked --bin fakingview
./tools/bundle.sh
open target/FakingView.app
```

`tools/bundle.sh` builds both release executables with `--locked`, embeds the alert helper,
application icon, GPL and font licenses, third-party notices, setup guide and app specs.
The binary is `target/release/fakingview`; the Finder application is `target/FakingView.app`.
No Rust installation is required to run the packaged app.

A fresh local bundle has no Developer ID or Apple notarization. Its linker-generated Mach-O
signature is not a distribution certificate. Background alerts use foreground evaluation when
Login Item registration is unavailable. Automatic update installation is disabled; download a
new release manually.

## Optional development settings

`FAKINGVIEW_NO_KEYCHAIN=1` keeps development credentials in memory. Use it only when you do not
need persistent credentials. `FAKINGVIEW_CACHE_DIR` selects a disposable cache location. For
Kite setup and environment names, see [the Kite guide](docs/kite-connect.md).

All-features checks compile the optional Swift iCloud bridge on macOS. If a restricted build
cannot write the normal compiler cache, use a local cache:

```sh
CLANG_MODULE_CACHE_PATH="$PWD/target/swift-module-cache" \
  cargo clippy --locked --all-targets --all-features -- -D warnings
```

An iCloud-enabled compilation is not an iCloud-entitled application. Distribution signing,
shared Keychain groups, provisioning profiles and CloudKit require your own Apple setup.
[The iCloud reference](docs/icloud-sync.md) describes those separate requirements.

## Releases

Use [the release guide](docs/releasing.md) or [the publishing procedure](docs/github-publishing.md).
`Cargo.toml` is the version source; `tools/release.sh prepare VERSION` updates the manifest and
root lock entry without upgrading dependencies. Published app, corresponding source and checksum
files must refer to the same committed version.

## Other platforms

Linux/Windows source remains available for development; there are no binaries for them in the
initial macOS release. The Linux CI check installs the libraries listed in
[ci.yml](.github/workflows/ci.yml). Historical platform workflows are archived under
[docs/archive/inherited-workflows](docs/archive/inherited-workflows) and do not run automatically.
