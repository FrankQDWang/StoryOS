#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
if [ -z "${STORYOS_VERIFICATION_RUN:-}" ]; then
  exec python3 scripts/verification.py step daily-database -- sh "$repository_root/scripts/verify-daily-database.sh" "$@"
fi
. "$repository_root/scripts/lib/controlled-postgres.sh"
container="storyos-daily-$$"
trap 'docker rm -f "$container" >/dev/null 2>&1 || true' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
start_postgres "$container"
STORYOS_STORAGE_ADMIN_URL=$(postgres_admin_url "$container") target/release-package/storyos-storage
set_runtime_password "$container"
load_controlled_fixture "$container"
export STORYOS_TEST_DATABASE_URL=$(postgres_runtime_url "$container")
export STORYOS_TEST_ADMIN_DATABASE_URL=$(postgres_admin_url "$container")
export STORYOS_TEST_POSTGRES_CONTAINER="$container"
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
