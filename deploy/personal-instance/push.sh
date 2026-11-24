#!/bin/sh
# Copies the archive of make release-package-linux, its checksum, and the scripts of the personal instance to the
# VPS with scp (ADR 0104), and prints the next command. It runs on the Mac of the author, in the repository.
#
# Usage: push.sh <archive> <SSH destination> <SSH port> [<certificate of the database CA>]
#
# The files go to the directory storyos-deploy in the home directory of the SSH user on the VPS.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
if [ "$#" != 3 ] && [ "$#" != 4 ]; then
  echo "usage: push.sh <archive> <SSH destination> <SSH port> [<certificate of the database CA>]" >&2
  exit 1
fi
archive=$1
destination=$2
port=$3
certificate=
if [ "$#" = 4 ]; then
  certificate=storyos-deploy/database-ca.crt
fi
if [ ! -f "$archive" ] || [ ! -f "$archive.sha256" ]; then
  echo "push.sh: $archive and $archive.sha256 must exist. Run make release-package-linux" >&2
  exit 1
fi
staging=$(mktemp -d "${TMPDIR:-/tmp}/storyos-push.XXXXXX")
trap 'rm -rf "$staging"' EXIT
mkdir "$staging/storyos-deploy"
cp "$archive" "$archive.sha256" "$here"/*.sh "$here"/*.service "$here"/*.timer "$here/Caddyfile.template" \
  "$here/instance.env.example" "$root/crates/storyos-adapter-postgres/migrations/0000_roles.sql" \
  "$staging/storyos-deploy/"
if [ -n "$certificate" ]; then
  cp "$4" "$staging/$certificate"
fi
scp -P "$port" -r "$staging/storyos-deploy" "$destination:"
cat <<EOF
Copied the files to storyos-deploy on the VPS. The next commands:
  ssh -p $port $destination
  sudo sh storyos-deploy/install.sh storyos-deploy/${archive##*/} $certificate
EOF
