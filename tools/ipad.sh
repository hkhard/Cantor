#!/usr/bin/env bash
# Build and install ChoirBird on an iPad.
#
#   tools/ipad.sh sim            build + run on the iPad Pro 13" simulator
#   HOMR=1 tools/ipad.sh device  ... with on-device note recognition (LIB-10)
#   tools/ipad.sh device [UDID]  signed build + install on a connected iPad
#   tools/ipad.sh release [app-store-connect|release-testing]
#                                release IPA of the tagged commit (not uploaded)
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

# LIB-10: on-device note recognition (homr, AGPL-only) only with HOMR=1:
# bundles the 123 MB of models and links ONNX Runtime for iOS.
HOMR_ARGS=()
if [ "${HOMR:-0}" = 1 ]; then
  bash "$ROOT/tools/bundle_homr_models.sh"
  bash "$ROOT/tools/omr/homr_port/fetch_ort_ios.sh"
  export ORT_IOS_XCFWK_PATH="$ROOT/vendor/onnxruntime.xcframework"
  HOMR_ARGS=(--features homr --config "$ROOT/app/src-tauri/tauri.homr.conf.json")
else
  bash "$ROOT/tools/bundle_homr_models.sh" --clear >/dev/null
fi

cd "$APP"
case "${1:-sim}" in
  sim)
    cargo tauri ios build --target aarch64-sim --debug ${HOMR_ARGS[@]+"${HOMR_ARGS[@]}"}
    BUNDLE="$(find ~/Library/Developer/Xcode/DerivedData/cantor-app-*/Build/Products/debug-iphonesimulator -maxdepth 1 -name ChoirBird.app | head -1)"
    SIM="$(xcrun simctl list devices available | grep -m1 'iPad Pro 13' | grep -oE '[0-9A-F-]{36}')"
    xcrun simctl boot "$SIM" 2>/dev/null || true
    open -a Simulator
    xcrun simctl install "$SIM" "$BUNDLE"
    xcrun simctl launch "$SIM" se.nikielhard.choirbird
    ;;
  device)
    cargo tauri ios build --target aarch64 --export-method debugging ${HOMR_ARGS[@]+"${HOMR_ARGS[@]}"}
    IPA="$APP/gen/apple/build/arm64/ChoirBird.ipa"
    DEV="${2:-$(xcrun devicectl list devices | grep -i 'ipad' | grep -v simulated | grep -E 'available|connected' | grep -v unavailable | grep -oE '[0-9A-Fa-f]{8}-[0-9A-Fa-f]{16}' | head -1)}"
    if [ -z "$DEV" ]; then
      echo "No connected iPad found. Connect it (USB or same Wi-Fi, unlocked), then rerun." >&2
      echo "Signed build is ready at $IPA" >&2
      exit 1
    fi
    xcrun devicectl device install app --device "$DEV" "$IPA"
    xcrun devicectl device process launch --device "$DEV" se.nikielhard.choirbird
    ;;
  release)
    # Release build of an exact tag (ROADMAP phase P, AGPL 6(d)): the tree
    # must be clean and HEAD tagged vX.Y.Z. Version = the tag, build number
    # = commit count, so every upload is unique and traceable. About shows
    # the tag and links its source. Not uploaded: that waits for the firm.
    cd "$ROOT"
    TAG="$(git describe --tags --exact-match --match 'v[0-9]*' 2>/dev/null || true)"
    if [ -z "$TAG" ] || [ -n "$(git status --porcelain --untracked-files=no -- . ':!app/src-tauri/Cargo.lock' ':!app/src-tauri/gen/apple/cantor-app_iOS/Info.plist')" ]; then
      echo "release: HEAD must be a clean commit tagged vX.Y.Z (git tag v0.2.0)" >&2
      exit 1
    fi
    VERSION="${TAG#v}"
    BUILD="$(git rev-list --count HEAD)"
    cd "$APP"
    CANTOR_VERSION="$TAG" cargo tauri ios build --target aarch64 \
      --export-method "${2:-release-testing}" --build-number "$BUILD" \
      --config "{\"version\":\"$VERSION\"}"
    echo "ChoirBird $TAG (build $BUILD): $APP/gen/apple/build/arm64/ChoirBird.ipa"
    ;;
  *)
    echo "usage: $0 sim|device [UDID]|release [app-store-connect|release-testing]" >&2
    exit 2
    ;;
esac
