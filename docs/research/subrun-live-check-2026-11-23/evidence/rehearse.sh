#!/bin/sh
# Installs the request guard on the database of STORYOS_TEST_POSTGRES_CONTAINER, then starts the run.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
docker exec -i "$STORYOS_TEST_POSTGRES_CONTAINER" psql -X -v ON_ERROR_STOP=1 -q -U postgres < "$here/request-guard.sql"
"$here/run.sh" "$1" "$2" "$3"
