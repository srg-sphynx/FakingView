#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO_BIN:-cargo}"
identity="${FAKINGVIEW_CODESIGN_IDENTITY:-}"
access_group="${FAKINGVIEW_KEYCHAIN_ACCESS_GROUP:-}"
# keychain-access-groups is a restricted entitlement: macOS kills a signed build at launch unless
# a provisioning profile that grants it is embedded in each bundle.
app_profile="${FAKINGVIEW_APP_PROFILE:-}"
helper_profile="${FAKINGVIEW_HELPER_PROFILE:-}"
# iCloud Sync is opt-in: set FAKINGVIEW_ICLOUD=1 to compile the CloudKit bridge and sign the app with
# the iCloud entitlements. The app profile must then grant them (see docs/icloud-sync.md); without
# the profile macOS kills the signed app at launch. The helper never gets iCloud entitlements.
icloud="${FAKINGVIEW_ICLOUD:-}"
icloud_environment="${FAKINGVIEW_ICLOUD_ENVIRONMENT:-Production}"
aps_environment=production
# A development build (FAKINGVIEW_DEV_BUILD=1, signed with an Apple Development identity and a Mac
# Development profile) talks to CloudKit's Development environment, where record types can be
# created. Debug assertions make the app expect that environment; a release never has them.
if [ -n "${FAKINGVIEW_DEV_BUILD:-}" ]; then
    icloud=1
    icloud_environment=Development
    aps_environment=development
    export CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS=true
fi
# Cargo.toml is the one place the version lives; tools/release.sh bumps it before tagging.
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
if [ -z "$version" ]; then
    echo "could not read the version from Cargo.toml" >&2
    exit 1
fi

signing_vars=0
for value in "$identity" "$access_group" "$app_profile" "$helper_profile"; do
    [ -n "$value" ] && signing_vars=$((signing_vars + 1))
done
if [ "$signing_vars" -ne 0 ] && [ "$signing_vars" -ne 4 ]; then
    echo "FAKINGVIEW_CODESIGN_IDENTITY, FAKINGVIEW_KEYCHAIN_ACCESS_GROUP, FAKINGVIEW_APP_PROFILE and FAKINGVIEW_HELPER_PROFILE must be supplied together" >&2
    exit 1
fi

features=""
[ -n "$icloud" ] && features="--features icloud"
# The release workflow compiles first, before the signing certificate is imported, so the build
# scripts of the dependency graph never run with it available; it then sets FAKINGVIEW_SKIP_BUILD.
# --locked: the build must use Cargo.lock as committed, never resolve anything new.
if [ -z "${FAKINGVIEW_SKIP_BUILD:-}" ]; then
    "$CARGO_BIN" build --release --locked $features --bin fakingview --bin fakingview-alert-agent
elif [ ! -x target/release/fakingview ] || [ ! -x target/release/fakingview-alert-agent ]; then
    echo "FAKINGVIEW_SKIP_BUILD is set but target/release has no built binaries" >&2
    exit 1
fi

app="target/FakingView.app"
helper="$app/Contents/Library/LoginItems/FakingViewAlertAgent.app"
executable="$app/Contents/MacOS/FakingView"
helper_executable="$helper/Contents/MacOS/FakingViewAlertAgent"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
mkdir -p "$helper/Contents/MacOS" "$helper/Contents/Resources"
cp target/release/fakingview "$executable"
cp target/release/fakingview-alert-agent "$helper_executable"
cp assets/brand/FakingView.icns "$app/Contents/Resources/FakingView.icns"
# The fonts compiled into the app are under the SIL OFL, which wants its text and the copyright
# notice to travel with every copy of them.
mkdir -p "$app/Contents/Resources/Licenses"
cp LICENSE.md "$app/Contents/Resources/Licenses/GPL-3.0.txt"
if [ -f THIRD_PARTY_NOTICES.md ]; then
    cp THIRD_PARTY_NOTICES.md "$app/Contents/Resources/Licenses/THIRD_PARTY_NOTICES.md"
fi
if [ -f docs/getting-started.md ]; then
    cp docs/getting-started.md "$app/Contents/Resources/Getting-Started.md"
fi
if [ -f docs/app-specs.md ]; then
    cp docs/app-specs.md "$app/Contents/Resources/App-Specs.md"
fi
cp assets/fonts/OFL-JetBrainsMono.txt assets/fonts/OFL-NotoSansSymbols2.txt "$app/Contents/Resources/Licenses/"
cp assets/fonts/README.md "$app/Contents/Resources/Licenses/FONTS.md"
# The helper shares the app icon: Login Items settings and its notifications show the bundle icon.
cp assets/brand/FakingView.icns "$helper/Contents/Resources/FakingView.icns"

cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>FakingView</string>
  <key>CFBundleDisplayName</key><string>FakingView</string>
  <key>CFBundleIdentifier</key><string>local.fakingview.desktop</string>
  <key>CFBundleExecutable</key><string>FakingView</string>
  <key>CFBundleIconFile</key><string>FakingView.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>__VERSION__</string>
  <key>CFBundleShortVersionString</key><string>__VERSION__</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

cat > "$helper/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>FakingView Alert Agent</string>
  <key>CFBundleDisplayName</key><string>FakingView Alert Agent</string>
  <key>CFBundleIdentifier</key><string>local.fakingview.desktop.alert-agent</string>
  <key>CFBundleExecutable</key><string>FakingViewAlertAgent</string>
  <key>CFBundleIconFile</key><string>FakingView.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>__VERSION__</string>
  <key>CFBundleShortVersionString</key><string>__VERSION__</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST

# Info.plist versions are numeric: a pre-release tag (0.2.0-rc.1) ships as 0.2.0.
sed -i '' "s/__VERSION__/${version%%-*}/" "$app/Contents/Info.plist" "$helper/Contents/Info.plist"

if [ -n "$identity" ]; then
    # The access group is `<TEAMID>.<name>`; each bundle's application identifier is its own
    # `<TEAMID>.<bundle id>`, which its profile must grant.
    team_id="${access_group%%.*}"
    cp "$app_profile" "$app/Contents/embedded.provisionprofile"
    cp "$helper_profile" "$helper/Contents/embedded.provisionprofile"
    sign_bundle() {
        bundle="$1"
        bundle_id="$2"
        template="tools/entitlements.plist"
        # Only the app carries iCloud; the login item has no CloudKit and its profile cannot grant it.
        if [ -n "$icloud" ] && [ "$bundle_id" = "local.fakingview.desktop" ]; then
            template="tools/entitlements-app.plist"
        fi
        entitlements="target/$bundle_id.entitlements"
        sed -e "s|__KEYCHAIN_ACCESS_GROUP__|$access_group|g" \
            -e "s|__TEAM_ID__|$team_id|g" \
            -e "s|__ICLOUD_ENVIRONMENT__|$icloud_environment|g" \
            -e "s|__APS_ENVIRONMENT__|$aps_environment|g" \
            -e "s|__APPLICATION_IDENTIFIER__|$team_id.$bundle_id|g" \
            "$template" > "$entitlements"
        codesign --force --options runtime --timestamp --entitlements "$entitlements" \
            --sign "$identity" "$bundle"
    }
    sign_bundle "$helper" local.fakingview.desktop.alert-agent
    sign_bundle "$app" local.fakingview.desktop
    codesign --verify --deep --strict --verbose=2 "$app"
    if [ -n "$icloud" ]; then
        ./tools/verify-icloud.sh "$app" "$icloud_environment"
    fi
    echo "Built and signed $app (background agent available)"
else
    if [ -n "$icloud" ]; then
        echo "Built with the iCloud bridge but unsigned: iCloud Sync will report that it is unavailable" >&2
    fi
    echo "Built $app unsigned; background alerts use the foreground fallback"
fi
