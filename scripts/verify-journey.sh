#!/bin/sh
# Run one exact-dist journey file with the project-scope procedure. Use make verify-journey.
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
if [ "$#" -ne 4 ]; then
  echo "Usage: verify-journey.sh <exact-dist test path> <runs> <load processes> <record directory>" >&2
  exit 2
fi
if [ -z "${STORYOS_VERIFICATION_RUN:-}" ]; then
  exec python3 scripts/verification.py step journey -- sh "$repository_root/scripts/verify-journey.sh" "$@"
fi
test_file=$1
runs=$2
load=$3
record=$4

timed_stage() {
  PYTHONDONTWRITEBYTECODE=1 python3 "$repository_root/scripts/verification.py" step "$@"
}

. "$repository_root/scripts/lib/exact-dist.sh"

container="storyos-journey-$$"
load_pids=""
stop_load() {
  if [ -n "$load_pids" ]; then
    kill $load_pids >/dev/null 2>&1 || true
    for pid in $load_pids; do
      wait "$pid" >/dev/null 2>&1 || true
    done
    remaining=0
    for pid in $load_pids; do
      if kill -0 "$pid" >/dev/null 2>&1; then
        remaining=$((remaining + 1))
      fi
    done
    printf 'Stopped load processes:%s; remaining: %s\n' "$load_pids" "$remaining"
    load_pids=""
  fi
}
cleanup() {
  stop_load
  if [ -n "$exact_dist_server_log" ]; then
    cp "$exact_dist_server_log" "$record/server.log" || true
  fi
  stop_exact_dist_server
  docker rm -fv "$container" >/dev/null 2>&1 || true
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
cleanup

make release-package
require_release_package
prepare_server_database "$container"
if exact_dist_ai_disabled_journey "$test_file"; then
  start_exact_dist_server ""
else
  start_exact_dist_server
fi
count=0
while [ "$count" -lt "$load" ]; do
  yes >/dev/null &
  load_pids="$load_pids $!"
  count=$((count + 1))
done
if [ -n "$load_pids" ]; then
  printf 'Started load processes:%s\n' "$load_pids"
fi
run=1
failed=0
while [ "$run" -le "$runs" ]; do
  echo "Run $run of $runs: restoring the controlled fixture"
  reset_exact_dist_fixture "$container"
  started=$(date +%s)
  status=passed
  timed_stage exact-dist -- pnpm --dir apps/web exec vitest run --project browser-exact-dist "$test_file" \
    || status=failed
  ended=$(date +%s)
  load_average=$(python3 -c 'import os; print(f"{os.getloadavg()[0]:.2f}")')
  printf '%s\t%s\t%s\t%s\n' "$run" "$status" "$((ended - started))" "$load_average" >>"$record/runs.tsv"
  if [ "$status" != "passed" ]; then
    failed=$((failed + 1))
  fi
  run=$((run + 1))
done
cleanup
[ "$failed" -eq 0 ]
