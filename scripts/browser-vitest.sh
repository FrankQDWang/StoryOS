#!/bin/sh
# Run one browser Vitest run of the scripts: `scripts/browser-vitest.sh <arguments of vitest run>` (issue 1734).
# Each run waits for the start gap of Google Chrome. A run that connected to no browser session ran no test file,
# so it runs one more time (ADR 0015). A failed test does not run again.
set -u
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
if [ -d "$root/target/check" ]; then
  work=$(mktemp -d "$root/target/check/browser-vitest.XXXXXX")
else
  work=$(mktemp -d "${TMPDIR:-/tmp}/storyos-browser-vitest.XXXXXX")
fi
trap 'rm -rf "$work"' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

run_once() {
  python3 "$root/scripts/check-lanes.py" --browser-start || return
  # POSIX sh has no pipefail, so the exit code of Vitest goes through a file.
  { pnpm --dir "$root/apps/web" exec vitest run "$@" 2>&1; echo "$?" > "$work/code"; } | tee "$work/output"
  return "$(cat "$work/code")"
}

run_once "$@" && exit 0
code=$?
if grep -qs 'Failed to connect to the browser session' "$work/output" && grep -qs 'Tests.*no tests' "$work/output"; then
  # The drill state is safe: the first run executed no test file, and apps/web/vitest.config.ts has no global setup.
  echo "browser-vitest: the run connected to no browser session and ran no test, so it runs one more time"
  run_once "$@"
  exit
fi
exit "$code"
