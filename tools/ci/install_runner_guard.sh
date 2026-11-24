#!/bin/bash
# Install tools/ci/runner_guard.sh into a self-hosted runner:
#   tools/ci/install_runner_guard.sh ~/dev/cantor-runner hkhard/Cantor
# Copies the guard to ~/.config/runner-guard/guard.sh (outside any checkout),
# points the runner's .env at it and restarts the runner service.
set -euo pipefail
dir="$1"
repo="$2"
src="$(cd "$(dirname "$0")" && pwd)/runner_guard.sh"
dest="$HOME/.config/runner-guard/guard.sh"
mkdir -p "$(dirname "$dest")"
cp "$src" "$dest"
chmod 755 "$dest"
env_file="$dir/.env"
touch "$env_file"
{
  grep -v -E '^(ACTIONS_RUNNER_HOOK_JOB_STARTED|RUNNER_GUARD_REPO|RUNNER_GUARD_ACTORS)=' "$env_file" || true
  echo "ACTIONS_RUNNER_HOOK_JOB_STARTED=$dest"
  echo "RUNNER_GUARD_REPO=$repo"
  echo "RUNNER_GUARD_ACTORS=hkhard"
} > "$env_file.new"
mv "$env_file.new" "$env_file"
(cd "$dir" && ./svc.sh stop >/dev/null && ./svc.sh start >/dev/null)
echo "guard installed for $repo in $dir"
