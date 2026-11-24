#!/bin/sh
# Checks one Linux x86_64 release package in an Ubuntu 24.04 container with no Rust (ADR 0104 section 6):
# the Web root of the Server, the settings of the Worker, and storyos-storage against a new PostgreSQL container.
# Usage: scripts/check-linux-package.sh <package directory>
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
. "$repository_root/scripts/lib/controlled-postgres.sh"

package=$(CDPATH= cd -- "$1" && pwd -P)
# The distribution of the VPS of ADR 0022, which has glibc 2.39.
ubuntu_image=ubuntu:24.04@sha256:562111b88caa79c71391c0c4d3c2114a7d5c35915c20561fd893cb1e07929e6b
# Seconds for each check. The checks of a correct package end in a few seconds.
check_limit=${STORYOS_LINUX_PACKAGE_CHECK_SECONDS:-120}
container="storyos-linux-package-$$"
trap 'docker rm -fv "$container" "$container-server" "$container-worker" "$container-storage" >/dev/null 2>&1 || true' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Runs one command of the package in the container "$container-$1". After check_limit seconds, a watchdog removes the
# container, so that the command fails. The Docker client sends a signal to the container, so it cannot stop the run.
in_ubuntu() {
  name="$container-$1"
  shift
  (
    sleep "$check_limit" &
    sleeper=$!
    trap 'kill "$sleeper"; exit 0' TERM
    wait "$sleeper"
    docker rm -fv "$name"
  ) >/dev/null 2>&1 &
  watchdog=$!
  status=0
  docker run --rm --name "$name" --platform linux/amd64 --volume "$package:/package:ro" "$@" || status=$?
  kill "$watchdog" 2>/dev/null || true
  if [ "$status" -ne 0 ]; then
    echo "The check $name failed with the status $status" >&2
  fi
  return "$status"
}

echo "Checking the Web root of the Server"
in_ubuntu server "$ubuntu_image" /package/storyos-server --check-web-root /package/web
echo "Checking the Worker"
in_ubuntu worker "$ubuntu_image" /package/storyos-worker --check
echo "Running storyos-storage against a new database"
start_postgres "$container"
activation=$(in_ubuntu storage --network "container:$container" \
  --env STORYOS_STORAGE_ADMIN_URL=postgres://postgres:admin@127.0.0.1:5432/postgres \
  "$ubuntu_image" /package/storyos-storage)
printf '%s\n' "$activation"
if [ "$activation" != STORYOS_STORAGE_ACTIVATION=Active ]; then
  echo "storyos-storage did not activate the database" >&2
  exit 1
fi
echo "The Linux package passed its checks"
