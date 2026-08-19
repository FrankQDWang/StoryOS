#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
container="storyos-issue105-$$"
export CARGO_NET_OFFLINE=true
cleanup() { docker rm -f "$container" >/dev/null 2>&1 || true; }
trap cleanup EXIT INT TERM
cleanup

docker run --detach --name "$container" \
  --env POSTGRES_PASSWORD=admin \
  --publish 127.0.0.1::5432 postgres:16-alpine >/dev/null

attempt=0
until docker logs "$container" 2>&1 | grep -q "PostgreSQL init process complete" \
  && docker exec "$container" pg_isready -U postgres >/dev/null 2>&1; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 40 ]; then
    echo "PostgreSQL did not become ready" >&2
    exit 1
  fi
  sleep 0.25
done

docker cp "$repository_root/crates/storyos-adapter-postgres/migrations/." \
  "$container:/tmp/storyos-release1-bootstrap" >/dev/null
if docker exec "$container" psql -X -v ON_ERROR_STOP=1 --single-transaction -U postgres \
  -f /tmp/storyos-release1-bootstrap/0000_roles.sql \
  -f /tmp/storyos-release1-bootstrap/0001_controlled_project.sql \
  -f /tmp/storyos-release1-bootstrap/0002_project_command_challenges.sql \
  -c "SELECT 1 / 0" \
  -f /tmp/storyos-release1-bootstrap/0004_editor_sessions.sql \
  -f /tmp/storyos-release1-bootstrap/0005_author_edits.sql \
  -f /tmp/storyos-release1-bootstrap/0006_snapshot_replay.sql \
  -f /tmp/storyos-release1-bootstrap/0007_takeover_admission_activity.sql >/dev/null 2>&1; then
  echo "The faulted Release 1 bootstrap unexpectedly committed" >&2
  exit 1
fi
rollback_state=$(docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT (SELECT count(*) FROM pg_roles
            WHERE rolname IN ('storyos_owner', 'storyos_runtime'))::text
          || '/' || COALESCE(to_regnamespace('storyos')::text, 'absent')")
if [ "$rollback_state" != "0/absent" ]; then
  echo "The faulted Release 1 bootstrap exposed partial state: $rollback_state" >&2
  exit 1
fi

docker exec "$container" psql -X -v ON_ERROR_STOP=1 --single-transaction -U postgres \
  -f /tmp/storyos-release1-bootstrap/0000_roles.sql \
  -f /tmp/storyos-release1-bootstrap/0001_controlled_project.sql \
  -f /tmp/storyos-release1-bootstrap/0002_project_command_challenges.sql \
  -f /tmp/storyos-release1-bootstrap/0004_editor_sessions.sql \
  -f /tmp/storyos-release1-bootstrap/0005_author_edits.sql \
  -f /tmp/storyos-release1-bootstrap/0006_snapshot_replay.sql \
  -f /tmp/storyos-release1-bootstrap/0007_takeover_admission_activity.sql >/dev/null

runtime_secret_state=$(docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT CASE WHEN rolpassword IS NULL THEN 'absent' ELSE 'present' END
     FROM pg_authid WHERE rolname = 'storyos_runtime'")
if [ "$runtime_secret_state" != "absent" ]; then
  echo "The tracked Release 1 bootstrap installed a runtime password" >&2
  exit 1
fi
docker exec "$container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  -c "ALTER ROLE storyos_runtime PASSWORD 'runtime'" >/dev/null

docker exec -i "$container" psql -X -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null

published=$(docker port "$container" 5432/tcp)
port=${published##*:}
export STORYOS_TEST_DATABASE_URL="postgres://storyos_runtime:runtime@127.0.0.1:$port/postgres"
export STORYOS_TEST_ADMIN_DATABASE_URL="postgres://postgres:admin@127.0.0.1:$port/postgres"
export STORYOS_TEST_POSTGRES_CONTAINER="$container"
echo "Running PostgreSQL Application and RLS tests"
cargo test -p storyos-adapter-postgres --test project_scope -- --ignored --nocapture
cargo test -p storyos-adapter-postgres --test project_command_challenge -- --ignored --nocapture
cargo test -p storyos-adapter-postgres --lib -- --ignored --nocapture
echo "Building the StoryOS Server"
cargo build --quiet -p storyos-server
echo "Running HTTP Project Scope tests"
node --test apps/web/test/project-http.integration.test.mjs
echo "Running HTTP ApplyAuthorEdit process-cut tests"
node --test apps/web/test/apply-author-edit-process-cut.integration.test.mjs
echo "Running HTTP Snapshot and Activity Stream tests"
node --test apps/web/test/snapshot-replay-http.integration.test.mjs
