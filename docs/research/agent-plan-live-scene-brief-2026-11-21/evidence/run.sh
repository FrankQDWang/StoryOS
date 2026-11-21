#!/bin/sh
# Starts the packaged Server (STORYOS_WORKER=0) and one continuous packaged Worker, then the run script.
# Usage: run.sh <destination: host_fake | volcengine_agent_plan> <port> <output directory>
# The database comes from STORYOS_TEST_DATABASE_URL and STORYOS_TEST_POSTGRES_CONTAINER (scripts/dev-postgres.sh).
# The run sets no embedding destination and no Exa key. The Skill directory is the one of the release package.
set -eu
destination=$1
port=$2
out=$3
root=$STORYOS_LIVE_ROOT
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$out"
cd "$root"
env -i PATH="$PATH" HOME="$HOME" \
  STORYOS_DATABASE_URL="$STORYOS_TEST_DATABASE_URL" \
  STORYOS_BOOTSTRAP_SESSIONS='{"session-a":"018f0000-0000-7001-8000-000000000001"}' \
  STORYOS_CHALLENGE_SECRET="test-only-challenge-secret-that-is-at-least-thirty-two-bytes" \
  STORYOS_WORKER=0 \
  STORYOS_SKILLS_DIR="$root/target/release-package/skill-packages" \
  STORYOS_MODEL_DESTINATION="$destination" \
  target/release-package/storyos-server --bind "127.0.0.1:$port" --web-root target/release-package/web \
  >"$out/server.log" 2>&1 &
server_pid=$!
env -i PATH="$PATH" HOME="$HOME" \
  STORYOS_DATABASE_URL="$STORYOS_TEST_DATABASE_URL" \
  STORYOS_MODEL_DESTINATION="$destination" \
  target/release-package/storyos-worker >"$out/worker.log" 2>&1 &
worker_pid=$!
trap 'kill "$server_pid" "$worker_pid" 2>/dev/null || true' EXIT
attempt=0
until grep -q '^STORYOS_SERVER_URL=http://' "$out/server.log"; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 100 ] || ! kill -0 "$server_pid" 2>/dev/null; then
    echo "The Server did not start" >&2
    exit 1
  fi
  sleep 0.2
done
sleep 2
kill -0 "$worker_pid" || { echo "The Worker did not start" >&2; exit 1; }
echo "server $server_pid worker $worker_pid"
node "$here/scene-brief-live.ts" "http://127.0.0.1:$port" "$worker_pid" "$out"
echo "script exit 0"
wait "$worker_pid" 2>/dev/null || true
