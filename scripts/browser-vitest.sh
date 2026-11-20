#!/bin/sh
# Run one browser Vitest run of the scripts: `scripts/browser-vitest.sh <arguments of vitest run>` (issue 1734).
# Each run waits for the start gap of Google Chrome. A run that connected to no browser session ran no test file,
# so it runs one more time (ADR 0015). A failed test does not run again.
set -u
root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd) || exit 1
if [ -d "$root/target/check" ]; then
  work=$(mktemp -d "$root/target/check/browser-vitest.XXXXXX") || exit 1
else
  work=$(mktemp -d "${TMPDIR:-/tmp}/storyos-browser-vitest.XXXXXX") || exit 1
fi
trap 'rm -rf "$work"' EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

wait_for_start_gap() {
  python3 "$root/scripts/check-lanes.py" --browser-start
}

run_vitest() {
  rm -f "$work/code"
  # POSIX sh has no pipefail, so the exit code of Vitest goes through a file.
  if ! { pnpm --dir "$root/apps/web" exec vitest run "$@" 2>&1; echo "$?" > "$work/code"; } | tee "$work/output"
  then
    echo "browser-vitest: the output of the Vitest run was not kept" >&2
    exit 1
  fi
  code=$(cat "$work/code" 2>/dev/null) || code=
  case $code in
    '' | *[!0-9]*) echo "browser-vitest: the exit code of the Vitest run is not known" >&2; exit 1 ;;
  esac
  return "$code"
}

connected_to_no_browser() {
  # Only the error line and the last "Tests" line (the summary of Vitest) count, also with colors.
  sed "s/$(printf '\033')\[[0-9;]*m//g" "$work/output" > "$work/plain" || return 1
  grep -Eq '^Error: Failed to connect to the browser session ' "$work/plain" \
    && grep -E '^[[:space:]]*Tests[[:space:]]' "$work/plain" | tail -n 1 \
      | grep -Eq '^[[:space:]]*Tests[[:space:]]+no tests[[:space:]]*$'
}

wait_for_start_gap || exit
run_vitest "$@" && exit 0
code=$?
connected_to_no_browser || exit "$code"
wait_for_start_gap || exit
# The drill state is safe: the first run executed no test file, and apps/web/vitest.config.ts has no global setup.
if [ -n "$(tail -c 1 "$work/output")" ]; then echo; fi
echo "browser-vitest: the run connected to no browser session and ran no test, so it runs one more time"
run_vitest "$@"
