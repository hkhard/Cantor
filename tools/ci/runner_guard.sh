#!/bin/bash
# Self-hosted runner guard: a job-started hook (ACTIONS_RUNNER_HOOK_JOB_STARTED
# in the runner's .env). The runner runs it before any step of any job, from a
# copy outside the checked-out repo, so a pull request cannot change or skip
# it. A job runs only when ALL of these hold; otherwise it fails before step 1:
#   - the repository is the one this runner belongs to (RUNNER_GUARD_REPO),
#   - the actor and the triggering actor are allowed (RUNNER_GUARD_ACTORS),
#   - the event is push, workflow_dispatch, schedule or pull_request,
#   - a pull request comes from a branch of the same repository, never a fork.
# Install: tools/ci/install_runner_guard.sh <runner dir> <owner/repo>.
set -u
repo="${RUNNER_GUARD_REPO:-}"
actors="${RUNNER_GUARD_ACTORS:-hkhard}"
deny() {
  echo "runner guard: refused: $*" >&2
  exit 1
}
[ -n "$repo" ] || deny "RUNNER_GUARD_REPO is not set on this runner"
[ "${GITHUB_REPOSITORY:-}" = "$repo" ] || deny "repository '${GITHUB_REPOSITORY:-}' is not $repo"
allowed() {
  local a
  for a in $actors; do [ "$1" = "$a" ] && return 0; done
  return 1
}
allowed "${GITHUB_ACTOR:-}" || deny "actor '${GITHUB_ACTOR:-}' is not allowed"
allowed "${GITHUB_TRIGGERING_ACTOR:-${GITHUB_ACTOR:-}}" || deny "triggering actor '${GITHUB_TRIGGERING_ACTOR:-}' is not allowed"
case "${GITHUB_EVENT_NAME:-}" in
  push | workflow_dispatch | schedule) ;;
  pull_request)
    head="$(/usr/bin/python3 -c 'import json,os;e=json.load(open(os.environ["GITHUB_EVENT_PATH"]));print(e["pull_request"]["head"]["repo"]["full_name"])' 2>/dev/null)"
    [ "$head" = "$repo" ] || deny "pull request from '$head', not from $repo"
    ;;
  *) deny "event '${GITHUB_EVENT_NAME:-}' is not allowed on this runner" ;;
esac
echo "runner guard: ok (${GITHUB_EVENT_NAME} by ${GITHUB_ACTOR} on ${GITHUB_REPOSITORY})"
