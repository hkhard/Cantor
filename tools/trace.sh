#!/usr/bin/env bash
# Requirement traceability (NFR-11): list each PRD requirement ID and the tests named after it.
# Usage: tools/trace.sh [--strict]   (--strict fails if any P0 requirement has no test)
set -euo pipefail
cd "$(dirname "$0")/.."

PRD=$(ls ./*PRD*.md | head -n1)
strict=${1:-}
missing=0

# Rows look like: | LIB-01 | P0 | ...
while IFS='|' read -r _ id pri _; do
  id=$(echo "$id" | xargs); pri=$(echo "$pri" | xargs)
  prefix=$(echo "$id" | tr 'A-Z-' 'a-z_')
  # Rust tests (fn <id>_...) and UI acceptance steps (step('<id> ...') in
  # tools/ui_harness/harness.js, run by the CI "ui" job).
  count=$({ grep -rhoE "fn ${prefix}_[a-z0-9_]+" crates 2>/dev/null || true
            grep -hoE "step\('${prefix} [^']+'" tools/ui_harness/*.js 2>/dev/null || true; } | sort -u | wc -l | xargs)
  printf '%-8s %-3s %s tests\n' "$id" "$pri" "$count"
  if [[ "$pri" == "P0" && "$count" == "0" ]]; then missing=$((missing + 1)); fi
done < <(grep -E '^\| [A-Z]+-[0-9]+ \| P[0-2] \|' "$PRD")

echo "P0 requirements without tests: $missing"
if [[ "$strict" == "--strict" && "$missing" -gt 0 ]]; then exit 1; fi
