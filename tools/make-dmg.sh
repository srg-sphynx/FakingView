#!/bin/sh
# Builds a compressed, read-only drag-to-Applications disk image using macOS tools.
# Usage: tools/make-dmg.sh <path/to/FakingView.app> <out.dmg>
set -eu

app="${1:?usage: make-dmg.sh <path/to/FakingView.app> <out.dmg>}"
out="${2:?usage: make-dmg.sh <path/to/FakingView.app> <out.dmg>}"

if [ ! -d "$app" ] || [ "$(basename "$app")" != "FakingView.app" ]; then
    echo "error: $app is not a FakingView.app" >&2
    exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir "$work/volume"
/usr/bin/ditto "$app" "$work/volume/FakingView.app"
ln -s /Applications "$work/volume/Applications"

image="$work/FakingView.dmg"
/usr/bin/hdiutil create -volname FakingView -srcfolder "$work/volume" \
    -format UDZO "$image"
/usr/bin/hdiutil verify "$image" >/dev/null
mkdir -p "$(dirname "$out")"
mv "$image" "$out"
echo "Built $out"
