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
container="storyos-linux-package-$$"
trap 'docker rm -fv "$container" >/dev/null 2>&1 || true' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

in_ubuntu() {
  docker run --rm --platform linux/amd64 --volume "$package:/package:ro" "$@"
}

echo "Checking the Web root of the Server"
in_ubuntu "$ubuntu_image" /package/storyos-server --check-web-root /package/web
echo "Checking the Worker"
in_ubuntu "$ubuntu_image" /package/storyos-worker --check
echo "Running storyos-storage against a new database"
start_postgres "$container"
activation=$(in_ubuntu --network "container:$container" \
  --env STORYOS_STORAGE_ADMIN_URL=postgres://postgres:admin@127.0.0.1:5432/postgres \
  "$ubuntu_image" /package/storyos-storage)
printf '%s\n' "$activation"
if [ "$activation" != STORYOS_STORAGE_ACTIVATION=Active ]; then
  echo "storyos-storage did not activate the database" >&2
  exit 1
fi
echo "The Linux package passed its checks"
