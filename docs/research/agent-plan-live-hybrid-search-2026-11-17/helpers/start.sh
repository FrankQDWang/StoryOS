#!/bin/sh
# usage: start.sh server|worker <destination> <logdir>
set -eu
role=$1; dest=$2; logdir=$3
root=$(cd "$(dirname "$0")/../.." && pwd)
eval "$("$root/scripts/dev-postgres.sh" env)"
mkdir -p "$logdir"
cd "$root"
case $role in
  server)
    exec env -i PATH="$PATH" HOME="$HOME" \
      STORYOS_DATABASE_URL="$STORYOS_TEST_DATABASE_URL" \
      STORYOS_WORKER=0 \
      STORYOS_MODEL_DESTINATION="$dest" \
      STORYOS_EMBEDDING_DESTINATION="$dest" \
      STORYOS_BOOTSTRAP_SESSIONS='{"session-a":"018f0000-0000-7001-8000-000000000001"}' \
      STORYOS_CHALLENGE_SECRET='live-run-local-challenge-secret-0123456789' \
      STORYOS_LOG=info \
      target/release-package/storyos-server --bind 127.0.0.1:8750 --web-root target/release-package/web \
      >"$logdir/server.log" 2>&1 ;;
  worker)
    exec env -i PATH="$PATH" HOME="$HOME" \
      STORYOS_DATABASE_URL="$STORYOS_TEST_DATABASE_URL" \
      STORYOS_MODEL_DESTINATION="$dest" \
      STORYOS_EMBEDDING_DESTINATION="$dest" \
      STORYOS_LOG=info \
      target/release-package/storyos-worker \
      >"$logdir/worker.log" 2>&1 ;;
esac
