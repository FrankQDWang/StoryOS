#!/bin/sh
# One Active, fixture-loaded PostgreSQL for local test iteration.
#
# `up` prepares the same Server-facing database that `verify-project-scope.sh`
# uses and prints the export lines that the ignored cargo tests and the
# `node-postgresql` Vitest project read. Run `eval "$(scripts/dev-postgres.sh up)"`.
# `reload` empties the domain tables and loads the fixture again. `env` prints the
# export lines for a running container. `down` removes the container.
#
# The packaged `storyos-storage` activates the database, so `make release-package`
# must have run on a clean worktree first.
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
. "$repository_root/scripts/lib/controlled-postgres.sh"

container=storyos-dev-postgres
storage_bin="$repository_root/target/release-package/storyos-storage"

print_env() {
  printf "export STORYOS_TEST_DATABASE_URL='%s'\n" "$(postgres_runtime_url "$container")"
  printf "export STORYOS_TEST_ADMIN_DATABASE_URL='%s'\n" "$(postgres_admin_url "$container")"
  printf "export STORYOS_TEST_POSTGRES_CONTAINER='%s'\n" "$container"
}

case "${1:-}" in
  up)
    if [ ! -x "$storage_bin" ]; then
      echo "storyos-storage is missing; run make release-package first" >&2
      exit 1
    fi
    docker rm -f "$container" >/dev/null 2>&1 || true
    start_postgres "$container"
    STORYOS_STORAGE_ADMIN_URL="$(postgres_admin_url "$container")" "$storage_bin" >/dev/null
    set_runtime_password "$container"
    load_controlled_fixture "$container"
    print_env
    ;;
  reload)
    reload_controlled_fixture "$container"
    ;;
  env)
    print_env
    ;;
  down)
    docker rm -f "$container" >/dev/null 2>&1 || true
    ;;
  *)
    echo "usage: scripts/dev-postgres.sh up | reload | env | down" >&2
    exit 1
    ;;
esac
