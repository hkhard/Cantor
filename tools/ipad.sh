#!/usr/bin/env bash
# Build and install the Cantor demo on an iPad.
#
#   tools/ipad.sh sim            build + run on the iPad Pro 13" simulator
#   tools/ipad.sh device [UDID]  signed build + install on a connected iPad
#
# Demo scores: those listed in tools/demo-scores.txt (public fixtures).
set -euo pipefail
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/app/src-tauri"

# Demo scores bundled into the app: only what tools/demo-scores.txt lists
# (decision 2026-11-23: Lacrymosa only, until the product owner says
# otherwise). The folder is emptied first, so nothing else slips in.
bash "$ROOT/tools/bundle_demo_scores.sh"

cd "$APP"
case "${1:-sim}" in
  sim)
    cargo tauri ios build --target aarch64-sim --debug
    BUNDLE="$(find ~/Library/Developer/Xcode/DerivedData/cantor-app-*/Build/Products/debug-iphonesimulator -maxdepth 1 -name Cantor.app | head -1)"
    SIM="$(xcrun simctl list devices available | grep -m1 'iPad Pro 13' | grep -oE '[0-9A-F-]{36}')"
    xcrun simctl boot "$SIM" 2>/dev/null || true
    open -a Simulator
    xcrun simctl install "$SIM" "$BUNDLE"
    xcrun simctl launch "$SIM" se.nikielhard.cantor
    ;;
  device)
    cargo tauri ios build --target aarch64 --export-method debugging
    IPA="$APP/gen/apple/build/arm64/Cantor.ipa"
    DEV="${2:-$(xcrun devicectl list devices | grep -i 'ipad' | grep -v simulated | grep -E 'available|connected' | grep -v unavailable | grep -oE '[0-9A-Fa-f]{8}-[0-9A-Fa-f]{16}' | head -1)}"
    if [ -z "$DEV" ]; then
      echo "No connected iPad found. Connect it (USB or same Wi-Fi, unlocked), then rerun." >&2
      echo "Signed build is ready at $IPA" >&2
      exit 1
    fi
    xcrun devicectl device install app --device "$DEV" "$IPA"
    xcrun devicectl device process launch --device "$DEV" se.nikielhard.cantor
    ;;
  *)
    echo "usage: $0 sim|device [UDID]" >&2
    exit 2
    ;;
esac
