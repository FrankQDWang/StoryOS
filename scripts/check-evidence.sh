#!/bin/sh
# Run one check tier and print its Check Evidence Line (ADR 0070).
# usage: scripts/check-evidence.sh <tier> command...
set -u
tier=$1
shift
dirty() {
  [ -n "$(git status --porcelain --untracked-files=all)" ]
}
if [ "$tier" != check-fast ] && dirty; then
  echo "make $tier needs a clean working tree. Commit your changes first, then run it again." >&2
  exit 1
fi
dirty_at_start=0
dirty && dirty_at_start=1
started=$(date +%s)
"$@"
code=$?
result=PASS
[ "$code" -eq 0 ] || result=FAIL
tree=$(git rev-parse HEAD^{tree})
if [ "$dirty_at_start" -eq 1 ] || dirty; then
  tree=DIRTY
  echo "The working tree has changes. This line is not evidence for a pull request."
fi
printf 'Check Evidence Line: tier=%s commit=%s tree=%s result=%s time=%s duration=%ss\n' \
  "$tier" "$(git rev-parse HEAD)" "$tree" "$result" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(($(date +%s) - started))"
exit "$code"
