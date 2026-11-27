#!/usr/bin/env bash
# App Review screen recording (Guideline 2.1): compress the device recording,
# upload it as the App Review Attachment of the version in review, and put
# the reviewer notes from docs/store/app-store.md into the Notes field.
#
#   tools/store/review_video.sh RECORDING.mp4|.mov [--submit]
#
# --submit also resubmits the version to App Review (otherwise press
# "Resubmit to App Review" in App Store Connect after replying there).
# Uses the App Store Connect API key in ~/.appstoreconnect/choirbird-asc.json.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
IN="${1:?usage: review_video.sh RECORDING.mp4 [--submit]}"
SUBMIT="${2:-}"
[ -f "$IN" ] || { echo "no such file: $IN" >&2; exit 1; }
OUTDIR="$ROOT/store-out/review"
mkdir -p "$OUTDIR"
OUT="$OUTDIR/ChoirBird-1.0.0-review.mp4"
rm -f "$OUT"

# Compress: 1080p H.264 with the AAC sound kept (Apple's own converter).
/usr/bin/avconvert --source "$IN" --preset Preset1920x1080 --output "$OUT" --replace >/dev/null
SIZE=$(stat -f %z "$OUT")
if [ "$SIZE" -gt 52428800 ]; then
  /usr/bin/avconvert --source "$IN" --preset Preset1280x720 --output "$OUT" --replace >/dev/null
  SIZE=$(stat -f %z "$OUT")
fi
DUR=$(/usr/bin/afinfo "$OUT" 2>/dev/null | awk '/estimated duration/ {print int($3)}')
AUDIO=$(/usr/bin/afinfo "$OUT" 2>/dev/null | awk '/Data format/ {print $3}')
echo "video: $OUT, $((SIZE / 1048576)) MB, ${DUR:-?} s, audio channels: ${AUDIO:-none}"
[ -n "$AUDIO" ] || { echo "no audio track found (was the silent switch on?): not uploading" >&2; exit 1; }

FL="$(readlink -f "$(command -v fastlane || echo /opt/homebrew/bin/fastlane)")"
D="$(dirname "$FL")/../libexec"
LC_ALL=en_US.UTF-8 GEM_HOME="$D" GEM_PATH="$D" /opt/homebrew/opt/ruby/bin/ruby -e 'gem "fastlane"; require "spaceship"; load ARGV[0]' \
  "$ROOT/tools/store/review_upload.rb" "$OUT" "$ROOT/docs/store/app-store.md" "$SUBMIT"
