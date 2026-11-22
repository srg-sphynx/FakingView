# Publishing a FakingView release

The release home is <https://github.com/srg-sphynx/FakingView/releases>. This guide describes the
release contract for FakingView. Follow the [GitHub publishing procedure](github-publishing.md)
for commands and asset names.

## What a release must identify

Every release states its version, source commit, supported operating systems/architectures,
signing/notarization status, installation instructions, checksums, verification performed and
known limitations. Stable releases cover the verified application and installation scope.
Authenticated market data depends on each provider account's entitlements. The application
version comes from `Cargo.toml`; a bundle's numeric version
follows the packaging script's representation of that version.

Do not publish API keys, tokens, account details, signing material, personal workspace databases,
local caches or diagnostic files containing sensitive data. Screenshots must contain no credentials
or account identifiers and must label fixture or sample market data when used.

## Prepare a reviewed source commit

1. Update [CHANGELOG.md](../CHANGELOG.md), [README.md](../README.md),
   [App specs](app-specs.md) and the [getting-started guide](getting-started.md) to match the artifacts.
2. Confirm the version and release type under the project's current release policy. Keep the tag
   and `Cargo.toml` version consistent; never reuse or move a published tag.
3. Review the complete diff, dependency lockfile, workflows and legal notices. Preserve
   [LICENSE.md](../LICENSE.md), font licenses and [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).
4. Run the required checks on the source commit:

   ```sh
   cargo test --locked
   cargo clippy --locked --all-targets --all-features -- -D warnings
   cargo fmt --all -- --check
   git diff --check
   ```

5. Exercise the app's affected native UI. Separate fixture/loopback coverage from authenticated
   live Kite checks. Record explicit skipped or unavailable checks in the release notes.

## Build the macOS artifact

Use a clean checkout of the release source and build with the committed lockfile:

```sh
./tools/bundle.sh
```

The script builds the main application and alert helper. The result is `target/FakingView.app`.
The ordinary local build is not Developer ID signed or notarized. Do not describe an ad-hoc Mach-O
signature as Apple distribution approval. Background Login Items and iCloud require their separate
signing identities, profiles and entitlements; those are not provided by a public source checkout.

Before distributing, check both bundle identities, the executable architecture, the icon, startup,
Settings and the token-entry flow. Include the application license, third-party notices and font
licenses with the bundle. Corresponding source must be available under GPL-3.0-only at the exact
release commit.

Package the arm64 app as a DMG and ZIP. Publish a corresponding-source archive and SHA-256
manifest alongside them. The release workflow builds from the version prepared on `main`
through manual workflow dispatch.

## Publish and verify

Use only the FakingView repository as the publishing destination. Do not run inherited tools
against the original project's repository or reuse another project's release credentials. Workflow
actions must be commit-pinned and permissions minimal; build scripts and dependency code must not
run after importing distribution signing secrets into a signing job.

Create the release as a draft, upload the intended artifacts and checksums, confirm its source tag
and metadata, then publish. After publication, download or inspect the hosted assets and compare
the checksum and file list with the prepared release. Verify that the README download links work.
Keep source provenance distinct from product affiliation.

Automatic in-app replacement is disabled in the current app. Publishing a GitHub release does
not enable it. A future updater needs its own trust model, repository binding, artifact validation
and migration review; it must not install binaries from an unrelated project.
