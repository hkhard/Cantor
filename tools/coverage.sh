#!/usr/bin/env bash
# NFR-11: line coverage of each core crate must be >= 85% (cargo-llvm-cov).
# Usage: tools/coverage.sh [min-percent]
set -euo pipefail
export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
cd "$(dirname "$0")/.."
MIN="${1:-85}"
command -v cargo-llvm-cov >/dev/null || cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview >/dev/null 2>&1 || true
cargo llvm-cov --workspace --summary-only 2>/dev/null | grep -E "^[a-z]+/src/" |
  awk -v min="$MIN" '{c=$1; sub(/\/.*/,"",c); l[c]+=$8; m[c]+=$9}
    END {bad=0; for (c in l) {p=100*(l[c]-m[c])/l[c]; printf "%-12s %5.1f%%\n", c, p; if (p < min) bad=1}
         if (bad) {print "below " min "%"; exit 1}}'
