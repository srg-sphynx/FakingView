# Publishing FakingView on GitHub

The project repository and release destination are
[`srg-sphynx/FakingView`](https://github.com/srg-sphynx/FakingView). FakingView uses its own
version line, product artwork and release identity. Copyright, license and provenance notices
remain in [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) and [licensing](licensing.md).

## What the first release contains

`0.1.0-beta.1` is the first independent FakingView prerelease. Beta status is deliberate: the
local macOS application and protocol mocks are tested, but no user's live Kite session or data
entitlement has been exercised. A successful profile response alone does not establish access
to historical or streaming market data.

| Asset | Contents |
| --- | --- |
| `FakingView-v0.1.0-beta.1-macos-arm64.zip` | Finder application and alert helper for Apple Silicon, macOS 14+ |
| `FakingView-v0.1.0-beta.1-source.tar.gz` | Complete corresponding source of the release commit, including build scripts, documentation and licenses |
| `SHA256SUMS.txt` | SHA-256 checksums of both archives |

The package is a manual-install, unsigned/non-notarized build. The Rust linker's ad-hoc
signature is not Developer ID signing or Apple notarization. Background alerts use the
foreground fallback. The release adds no live Kite orders or INR portfolio/paper accounting.

## Prepare a version

Run this from a checkout with Rust available:

```sh
./tools/release.sh prepare 0.1.0 --dry-run
./tools/release.sh prepare 0.1.0
```

Preparation changes only the root package versions in `Cargo.toml` and `Cargo.lock`. It keeps
the locked dependency graph and original lock ordering, validates Cargo metadata offline, and
restores both files if validation fails. Existing unrelated edits are preserved. Preparation
creates no commits, tags or remote changes. Review the diff, then run:

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Commit the complete reviewed release on `main` and push that exact commit. Do not publish a
binary built from uncommitted application changes. Toolchain prerequisites are described in
[BUILDING.md](../BUILDING.md).

## Build and package a local release

The commands below require macOS on Apple Silicon. Set `version` to the already prepared,
committed package version. `dist/` is a release staging directory; do not commit binaries.

```sh
version=0.1.0
tag="v$version"
release_commit=$(git rev-parse HEAD)
FAKINGVIEW_BUILD_ID="$release_commit" ./tools/bundle.sh
licenses=target/FakingView.app/Contents/Resources/Licenses
cp LICENSE.md "$licenses/GPL-3.0.txt"
cp THIRD_PARTY_NOTICES.md "$licenses/THIRD_PARTY_NOTICES.md"
cp docs/getting-started.md target/FakingView.app/Contents/Resources/Getting-Started.md
mkdir -p dist
./tools/make-dmg.sh target/FakingView.app "dist/FakingView-$tag-macos-arm64.dmg"
ditto -c -k --sequesterRsrc --keepParent target/FakingView.app "dist/FakingView-$tag-macos-arm64.zip"
git archive --format=tar --prefix="FakingView-$version/" "$release_commit" | gzip -n > "dist/FakingView-$tag-source.tar.gz"
(cd dist && shasum -a 256 "FakingView-$tag-macos-arm64.dmg" "FakingView-$tag-macos-arm64.zip" "FakingView-$tag-source.tar.gz" > SHA256SUMS.txt)
```

Keep release notes in `docs/releases/vVERSION.md`, committed before building. Include the
verified test results, platform/architecture, setup link, remaining live-data validation and
signing status. Never include real API keys, access tokens, personal account screenshots or
configuration files in a source archive or release asset.

## Publish reviewed local artifacts

Authenticate GitHub CLI locally with repository contents access. Publishing source and an
application release needs no `admin:*`, `admin:ssh_signing_key` or `delete_repo` permission.
Never paste a token into a chat, repository file, shell script or release note.

```sh
./tools/release.sh publish 0.1.0 \
  --assets-dir dist --notes-file docs/releases/v0.1.0.md --dry-run
./tools/release.sh publish 0.1.0 \
  --assets-dir dist --notes-file docs/releases/v0.1.0.md
```

Publication requires clean `main`, matching Cargo version, the exact commit already at the
remote's `main`, the DMG, ZIP, source archive and valid checksums. It refuses an existing release
or a tag naming a different commit. GitHub creates the release tag at that exact commit; the
script does not create or push local tags. A hyphenated version is published as a prerelease.
The interactive prompt may be suppressed with `--yes` when publication is already authorized.

A failed publication is not permission to overwrite an existing published asset. Inspect the
remote release state before retrying. Prefer a new version for a corrected binary. Do not move
an existing release tag.

## Build future releases with Actions

After preparing, validating and pushing a new version, open
[Release FakingView](https://github.com/srg-sphynx/FakingView/actions/workflows/release.yml), select
**Run workflow**, choose `main`, and enter the exact prepared Cargo version. The equivalent CLI:

```sh
gh workflow run release.yml --repo srg-sphynx/FakingView --ref main -f version=0.1.0
```

The workflow validates one source SHA and its manifest/lock versions, runs formatting, tests
and all-feature Clippy, builds the app with Rust 1.100.0, and attaches the DMG, ZIP, matching source
and checksums. It restores no build cache and requires no Apple signing secrets. Its final
job alone has `contents: write`. It refuses to overwrite an existing release. There is no tag
push trigger, so manually publishing the first release does not start a competing release job.

`macos-15` is documented by GitHub as an arm64 runner. The job checks `uname -m` and fails if
that assumption changes. See [GitHub's hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

## Repository presentation

- Repository description: a native macOS market workspace with Kite Connect data for India.
- Homepage: the repository's getting-started guide or another FakingView URL that actually exists.
- README screenshots are captured from this application's actual interface; sample charts must
  identify fixture data and must not claim live Kite verification.
- Use `assets/social/github-social-preview.png` as the GitHub **Settings → General → Social
  preview** image. GitHub's public repository API does not provide a social-preview upload field;
  upload this image through repository settings if it has not been applied already.
- Keep social artwork separate from screenshots. Artwork must not invent an unimplemented app
  screen or imply broker affiliation, investment returns, live credentials or Apple notarization.

## Workflow inventory

Active: `ci.yml` (macOS/Linux checks), `supply-chain.yml` (read-only dependency/workflow audits),
and explicitly dispatched `release.yml` (macOS arm64 packages).

Inactive platform workflow snapshots are under
[`docs/archive/inherited-workflows`](archive/inherited-workflows). They have no active scheduled
runs and do not certify Windows, Linux binary, Flatpak, Intel macOS or disk-image releases.
FakingView source can remain portable while release support stays bounded to validated packages.
