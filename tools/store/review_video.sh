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
IN="${1:?usage: review_video.sh RECORDING.mov|.mp4 [--submit]}"
SUBMIT="${2:-}"
[ -f "$IN" ] || { echo "no such file: $IN" >&2; exit 1; }
OUTDIR="$ROOT/store-out/review"
mkdir -p "$OUTDIR"
OUT="$OUTDIR/ChoirBird-1.0.0-review.mp4"
rm -f "$OUT"

# Compress any .mov/.mp4 to H.264 + AAC at most 1920 px high, never
# upscaled, video capped at 3 Mbit/s (about 25 MB a minute), sound kept.
# Apple's avconvert presets scale small recordings *up* (888x1920 became
# 101 MB), so ffmpeg is used when it is installed.
FFMPEG="$(command -v ffmpeg || echo /opt/homebrew/bin/ffmpeg)"
if [ -x "$FFMPEG" ]; then
  "$FFMPEG" -v error -y -i "$IN" -vf "scale=-2:'min(1920,ih)'" -c:v libx264 -preset medium -crf 26 \
    -maxrate 3M -bufsize 6M -pix_fmt yuv420p -c:a aac -b:a 128k -movflags +faststart "$OUT"
else
  /usr/bin/avconvert --source "$IN" --preset PresetMediumQuality --output "$OUT" --replace >/dev/null
fi
SIZE=$(stat -f %z "$OUT")
if [ "$SIZE" -gt 52428800 ]; then
  echo "video is $((SIZE / 1048576)) MB after compression (limit 50 MB): record a shorter take" >&2
  exit 1
fi
DUR=$(/usr/bin/afinfo "$OUT" 2>/dev/null | awk '/estimated duration/ {print int($3)}')
AUDIO=$(/usr/bin/afinfo "$OUT" 2>/dev/null | awk '/Data format/ {print $3}')
echo "video: $OUT, $((SIZE / 1048576)) MB, ${DUR:-?} s, audio channels: ${AUDIO:-none}"
[ -n "$AUDIO" ] || { echo "no audio track found (was the silent switch on?): not uploading" >&2; exit 1; }

FL="$(readlink -f "$(command -v fastlane || echo /opt/homebrew/bin/fastlane)")"
D="$(dirname "$FL")/../libexec"
LC_ALL=en_US.UTF-8 GEM_HOME="$D" GEM_PATH="$D" /opt/homebrew/opt/ruby/bin/ruby -e 'gem "fastlane"; require "spaceship"; load ARGV[0]' \
  "$ROOT/tools/store/review_upload.rb" "$OUT" "$ROOT/docs/store/app-store.md" "$SUBMIT"
