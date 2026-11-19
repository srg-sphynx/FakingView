# Releases and updates guidance

FakingView publishes to [`srg-sphynx/FakingView`](https://github.com/srg-sphynx/FakingView).
This document replaces the inherited signing-account and multi-platform release procedure.
The actionable procedure is [GitHub publishing](../github-publishing.md).

## Current release contract

- Source of truth: the root `Cargo.toml` version, mirrored in the root `Cargo.lock` package.
- Prepare explicit versions through `tools/release.sh prepare VERSION`; do not hand-edit tags.
- Release only a clean, reviewed, validated commit that has reached `main`.
- Initial independent version: `0.1.0-beta.1`. A beta is a GitHub prerelease. Never reuse or move a
  published version/tag; publish a new version for corrections.
- Current artifact: `FakingView-vVERSION-macos-arm64.zip`, containing `FakingView.app` and its
  alert helper. The app targets macOS 14 or newer on Apple Silicon.
- Also attach `FakingView-vVERSION-source.tar.gz` and `SHA256SUMS.txt`. Corresponding source,
  GPL text, original copyright notices, bundled-font licenses and third-party notices accompany
  distribution.
- The package is unsigned/non-notarized for manual installation. A linker-generated ad-hoc
  Mach-O signature is not an Apple Developer ID or notarization. Background Login Items use the
  foreground fallback. Live Kite validation requires the user's eligible account and session.

## Active workflows

`ci.yml` tests and lints macOS arm64 and Linux x64 on pushes to `main` and pull requests.
`supply-chain.yml` audits dependencies and workflows. `release.yml` is explicitly dispatched
with a prepared version; it does not respond to an initial publication's tag and create a race.
It validates one immutable commit, tests/builds it without a restored build cache, packages
source and checksums, then grants write access only to its final publish job.

The archived files under `docs/archive/inherited-workflows/` are not active workflows. Their
previous packaging assumptions do not certify FakingView builds for Intel macOS, Windows,
Linux binaries or Flatpak.

## Build hardening

- Pin every external Action to a full commit SHA, with an accurate version comment.
- Pin Rust to the reviewed exact toolchain (currently 1.100.0); use `--locked` on builds/checks.
- No Git or alternate-registry dependencies. Run `cargo deny check` with `deny.toml` intact.
- Default job permission is `contents: read`; only publishing gets `contents: write`.
- `actions/checkout` uses `persist-credentials: false`; never expose a GitHub token to build
  scripts. No Apple signing credentials are required or imported by the current workflow.
- Dispatch from `main`, check out and build its exact verified SHA, and bind the release tag to
  that SHA. Refuse an existing release or a tag pointing to another commit.
- Do not silently overwrite a failed or published release. Fix the cause and choose a new
  version, or rerun a failed pre-publication workflow for its unchanged source commit.
- Never request `admin:*`, `admin:ssh_signing_key` or `delete_repo` token scopes.

## Update trust

Automatic installation stays disabled for current manually installed builds. Product release
links name the FakingView repository. No signing identity is inherited merely because source
code was reused. Enabling automatic installation later requires a separately reviewed owner,
verified release/tag policy, authenticated artifact provenance and Apple signing/notarization.
A successful unit test or checksum comparison is not an Apple trust assessment.

## Future signed packages

Developer ID signing, restricted Keychain access groups, iCloud provisioning and notarization
require FakingView's own Apple account, identifiers and profiles. Introduce them through a
separate reviewed change, compile third-party build scripts before importing secrets, and test
both the application and helper on a quarantined download. Do not reuse another project's
signing identity, hosted release feed, update trust roots or certificate setup.
