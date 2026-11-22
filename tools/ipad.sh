#!/usr/bin/env bash
# Build and install the Cantor demo on an iPad.
#
#   tools/ipad.sh sim            build + run on the iPad Pro 13" simulator
#   tools/ipad.sh device [UDID]  signed build + install on a connected iPad
#
# Demo scores are copied from scores/out (private, git-ignored) into the app
# bundle at build time. Without them the library is empty.
set -euo pipefail
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APP="$ROOT/app/src-tauri"

mkdir -p "$APP/resources/scores"
if compgen -G "$ROOT/scores/out/*.mxl" > /dev/null; then
  cp "$ROOT"/scores/out/*.mxl "$APP/resources/scores/"
  for f in "$ROOT"/scores/out/*.musicxml; do
    stem="$(basename "$f" .musicxml)"
    # Prefer the .mxl when both exist.
    [ -e "$ROOT/scores/out/$stem.mxl" ] || cp "$f" "$APP/resources/scores/"
  done
fi

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
