#!/bin/sh
# Local Active, fixture-loaded PostgreSQL: eval "$(scripts/dev-postgres.sh up)"
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
