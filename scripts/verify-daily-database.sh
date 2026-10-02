#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
if [ -z "${STORYOS_VERIFICATION_RUN:-}" ]; then
  exec python3 scripts/verification.py step daily-database -- sh "$repository_root/scripts/verify-daily-database.sh" "$@"
fi
if [ "${1:-}" != "--prepared" ]; then
  exec "$repository_root/scripts/dev-postgres.sh" run sh "$0" --prepared "$@"
fi
shift
for group in "$@"; do
  case "$group" in
    database)
      for target in project_scope project_command_challenge; do
        case "$target" in project_scope) stage=postgres-scope ;; project_command_challenge) stage=postgres-challenge ;; esac
        python3 scripts/verification.py step "$stage" -- cargo test --locked --workspace --all-features \
          --test "$target" -- --ignored --nocapture
      done
      python3 scripts/verification.py step postgres-library -- cargo test --locked --workspace --all-features \
        --lib -- --ignored --nocapture ;;
    node-postgresql|node-process-cut) ;;
    *) echo "Unsupported daily database group: $group" >&2; exit 1 ;;
  esac
done
python3 scripts/verification_shared.py run --groups "$@"
