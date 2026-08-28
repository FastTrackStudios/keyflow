#!/usr/bin/env bash
# Build the Keyflow iPhone app.
#
# Run on a Mac inside the repo's nix dev shell. The env dance is REQUIRED:
# nixpkgs ships a fake xcbuild `xcrun` and its SDK env breaks Xcode's, so
# iOS cross-compiles need the real xcrun first on PATH and the nix SDK vars
# unset (the flake's CARGO_TARGET_*_LINKER / CC_* handle the rest).
#
#   cd apps/mobile && ./ios/build-ios.sh [--sim <udid>]
#
# With --sim, also installs + relaunches on that simulator.
#
# The keyboard extension is a separate Xcode target and is not built here —
# see ios/README.md.
set -euo pipefail

cd "$(dirname "$0")/.."

BIN_IOS="$HOME/bin-ios"
mkdir -p "$BIN_IOS"
ln -sf /usr/bin/xcrun "$BIN_IOS/xcrun"
ln -sf /usr/bin/xcodebuild "$BIN_IOS/xcodebuild"

unset DEVELOPER_DIR SDKROOT
export PATH="$BIN_IOS:$PATH"

dx build --platform ios

APP="$(cd ../.. && pwd)/target/dx/keyflow-mobile/debug/ios/Keyflow-mobile.app"

# The chart editor wants both orientations: portrait to write, landscape to
# read a chart the way it will be read on a stand.
/usr/libexec/PlistBuddy -c \
  "Add :NSUserActivityTypes array" "$APP/Info.plist" 2>/dev/null || true

echo "built: $APP"

if [[ "${1:-}" == "--sim" && -n "${2:-}" ]]; then
  UDID="$2"
  xcrun simctl boot "$UDID" 2>/dev/null || true
  xcrun simctl install "$UDID" "$APP"
  xcrun simctl launch --console-pty "$UDID" app.fasttrackstudio.keyflow
fi
