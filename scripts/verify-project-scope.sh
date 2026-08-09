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

docker exec "$container" psql -v ON_ERROR_STOP=1 -U postgres -c \
  "CREATE ROLE storyos_owner NOLOGIN; CREATE ROLE storyos_runtime LOGIN PASSWORD 'runtime' NOSUPERUSER NOBYPASSRLS;" >/dev/null
docker exec -i "$container" psql -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/migrations/0001_controlled_project.sql" >/dev/null
docker exec -i "$container" psql -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null

published=$(docker port "$container" 5432/tcp)
port=${published##*:}
export STORYOS_TEST_DATABASE_URL="postgres://storyos_runtime:runtime@127.0.0.1:$port/postgres"
export STORYOS_TEST_ADMIN_DATABASE_URL="postgres://postgres:admin@127.0.0.1:$port/postgres"
echo "Running PostgreSQL Application and RLS tests"
cargo test -p storyos-adapter-postgres --test project_scope -- --ignored --nocapture
