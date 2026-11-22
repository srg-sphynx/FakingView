#!/bin/sh
# Prepare an explicit version, then publish reviewed artifacts from a clean, pushed main.
# No local tags are created or pushed. GitHub creates the release tag at the supplied commit.
set -eu
cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO_BIN:-cargo}"

usage() {
    cat >&2 <<'USAGE'
usage:
  tools/release.sh prepare VERSION [--dry-run]
  tools/release.sh publish VERSION --assets-dir DIR --notes-file FILE [--dry-run] [--yes]
VERSION is X.Y.Z or X.Y.Z-(alpha|beta|rc).N, with N starting at 1.
USAGE
    exit 2
}

[ "$#" -ge 2 ] || usage
command="$1"
version="${2#v}"
shift 2
case "$command" in prepare | publish) ;; *) usage ;; esac
if ! printf '%s\n' "$version" | LC_ALL=C grep -Eq '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-(alpha|beta|rc)\.[1-9][0-9]*)?$'; then
    echo "error: invalid release version '$version'" >&2
    exit 1
fi

dry_run=0
assume_yes=0
assets_dir=""
notes_file=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run) dry_run=1 ;;
        --yes) assume_yes=1 ;;
        --assets-dir) [ "$#" -ge 2 ] || usage; assets_dir="$2"; shift ;;
        --notes-file) [ "$#" -ge 2 ] || usage; notes_file="$2"; shift ;;
        *) usage ;;
    esac
    shift
done

current=$(awk '/^\[/ { package = ($0 == "[package]") } package && /^version = "/ { gsub(/^version = "|"$/, ""); print; exit }' Cargo.toml)
[ -n "$current" ] || { echo "error: no package version in Cargo.toml" >&2; exit 1; }
tag="v$version"

if [ "$command" = prepare ]; then
    [ -z "$assets_dir$notes_file" ] && [ "$assume_yes" = 0 ] || usage
    echo "Prepare FakingView $tag: Cargo.toml and the root Cargo.lock package $current -> $version"
    if [ "$dry_run" = 1 ]; then
        echo "Dry run: no files, commits, tags or remotes changed."
        exit 0
    fi
    if [ "$current" = "$version" ]; then
        echo "Already at $version; nothing changed."
        exit 0
    fi
    mkdir -p target/release-preparation
    backup=$(mktemp -d target/release-preparation/version.XXXXXX)
    cp Cargo.toml "$backup/Cargo.toml"
    cp Cargo.lock "$backup/Cargo.lock"
    prepared=0
    cleanup() {
        if [ "$prepared" != 1 ]; then
            cp "$backup/Cargo.toml" Cargo.toml
            cp "$backup/Cargo.lock" Cargo.lock
        fi
        rm -rf "$backup"
    }
    trap cleanup 0
    trap 'exit 1' 1 2 15
    awk -v new="$version" '
        /^\[/ { package = ($0 == "[package]") }
        package && /^version = "/ { print "version = \"" new "\""; next }
        { print }
    ' "$backup/Cargo.toml" > Cargo.toml
    # Only this package's locked version may change; keep the original lock ordering too.
    awk -v new="$version" '
        /^\[\[package\]\]/ { root = 0 }
        /^name = "fakingview"$/ { root = 1 }
        root && /^version = "/ { print "version = \"" new "\""; next }
        { print }
    ' "$backup/Cargo.lock" > "$backup/expected.lock"
    cp "$backup/expected.lock" Cargo.lock
    "$CARGO_BIN" metadata --locked --offline --no-deps --format-version 1 > "$backup/metadata.json"
    if ! cmp -s Cargo.lock "$backup/expected.lock"; then
        echo "error: Cargo.lock changed beyond the root package version; rolling back the preparation" >&2
        exit 1
    fi
    prepared=1
    echo "Prepared $version. Review the diff, validate, commit, and push main before publishing."
    exit 0
fi

[ -n "$assets_dir" ] && [ -n "$notes_file" ] || usage
[ "$current" = "$version" ] || { echo "error: package version is $current, not $version" >&2; exit 1; }
[ "$(git symbolic-ref --short HEAD)" = main ] || { echo "error: publish from main" >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "error: publish requires a clean working tree, including untracked files" >&2; exit 1; }
commit=$(git rev-parse HEAD)
app_asset="FakingView-$tag-macos-arm64.zip"
dmg_asset="FakingView-$tag-macos-arm64.dmg"
source_asset="FakingView-$tag-source.tar.gz"
for asset in "$dmg_asset" "$app_asset" "$source_asset" SHA256SUMS.txt; do
    [ -f "$assets_dir/$asset" ] || { echo "error: missing $assets_dir/$asset" >&2; exit 1; }
done
[ -f "$notes_file" ] || { echo "error: missing release notes $notes_file" >&2; exit 1; }
(
    cd "$assets_dir"
    shasum -a 256 -c SHA256SUMS.txt
)
echo "Publish FakingView $tag at $commit to $(git remote get-url origin)"
echo "Assets: $dmg_asset, $app_asset, $source_asset, SHA256SUMS.txt"
if [ "$dry_run" = 1 ]; then
    echo "Dry run: no files, commits, tags or remotes changed."
    exit 0
fi
command -v gh >/dev/null 2>&1 || { echo "error: GitHub CLI (gh) is required" >&2; exit 1; }
gh auth status >/dev/null
repository=$(gh repo view --json nameWithOwner --jq .nameWithOwner)
remote_main=$(gh api "repos/$repository/commits/main" --jq .sha)
[ "$remote_main" = "$commit" ] || { echo "error: push this exact main commit before publishing" >&2; exit 1; }
if gh release view "$tag" --repo "$repository" >/dev/null 2>&1; then
    echo "error: release $tag already exists; published releases are never overwritten" >&2
    exit 1
fi
if existing=$(gh api "repos/$repository/commits/$tag" --jq .sha 2>/dev/null); then
    [ "$existing" = "$commit" ] || { echo "error: existing tag $tag names another commit" >&2; exit 1; }
fi
if [ "$assume_yes" != 1 ]; then
    printf 'Publish the reviewed release and create its GitHub tag? [y/N] '
    read -r answer
    case "$answer" in y | Y) ;; *) echo "aborted"; exit 1 ;; esac
fi
if [ "$version" != "${version%%-*}" ]; then
    set -- --prerelease
else
    set -- --latest
fi
gh release create "$tag" \
    "$assets_dir/$dmg_asset" "$assets_dir/$app_asset" "$assets_dir/$source_asset" "$assets_dir/SHA256SUMS.txt" \
    --repo "$repository" --target "$commit" --title "FakingView $version" \
    --notes-file "$notes_file" "$@"
