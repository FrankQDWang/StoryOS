# Source after setting repository_root.

# The PostgreSQL image with pgvector of the record of S7-C1 (ADR 0095), with its exact tag and
# digest. Each script that starts PostgreSQL reads this variable.
storyos_postgres_image=pgvector/pgvector:0.8.7-pg16-bookworm@sha256:484e838076e04939d27d3e176e91c86eea96236126023be8d3059761122e579f
# The C.UTF-8 locale keeps the text order of the earlier Alpine image (the record, "Text order").
storyos_postgres_initdb_args=--locale=C.UTF-8

# Earlier stages use the 10-per-minute Command Challenge budget of the fixture Project.
reset_command_challenge_rate_windows() {
  docker exec "$1" psql -X -v ON_ERROR_STOP=1 -U postgres \
    -c "UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0" >/dev/null
}

# Starts the container $1 from the image $2, or from storyos_postgres_image when $2 is empty. The arguments after $2
# go to the PostgreSQL server, for example "-c fsync=off". In a run of scripts/check-lanes.py, the container gets
# the label of the run, so that the runner removes only the containers of its own run.
start_postgres() {
  name=$1
  postgres_image=${2:-$storyos_postgres_image}
  shift
  if [ "$#" -gt 0 ]; then
    shift
  fi
  docker run --detach --name "$name" ${STORYOS_CHECK_RUN:+--label "storyos.check-run=$STORYOS_CHECK_RUN"} \
    --env POSTGRES_PASSWORD=admin \
    --env POSTGRES_INITDB_ARGS="$storyos_postgres_initdb_args" \
    --publish 127.0.0.1::5432 "$postgres_image" "$@" >/dev/null
  attempt=0
  until docker logs "$name" 2>&1 | grep -q "PostgreSQL init process complete" \
    && docker exec "$name" pg_isready -U postgres >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 40 ]; then
      echo "PostgreSQL did not become ready: $name" >&2
      exit 1
    fi
    sleep 0.25
  done
}

postgres_admin_url() {
  published=$(docker port "$1" 5432/tcp)
  printf 'postgres://postgres:admin@127.0.0.1:%s/postgres\n' "${published##*:}"
}

postgres_runtime_url() {
  published=$(docker port "$1" 5432/tcp)
  printf 'postgres://storyos_runtime:runtime@127.0.0.1:%s/postgres\n' "${published##*:}"
}

set_runtime_password() {
  docker exec "$1" psql -X -v ON_ERROR_STOP=1 -U postgres \
    -c "ALTER ROLE storyos_runtime PASSWORD 'runtime'" >/dev/null
}

load_controlled_fixture() {
  docker exec -i "$1" psql -X -v ON_ERROR_STOP=1 -U postgres \
    < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null
}

# Keeps the activation proof and migration ledger, so the database stays Active.
reload_controlled_fixture() {
  docker exec "$1" psql -X -v ON_ERROR_STOP=1 -U postgres \
    -c "SET client_min_messages = warning" -c \
    "DO \$\$ DECLARE tbl text; BEGIN
       FOR tbl IN SELECT tablename FROM pg_tables
         WHERE schemaname = 'storyos'
           AND tablename NOT IN (
             'storage_activation_proofs',
             'schema_migrations',
             'migration_phases',
             'migration_phase_checksums'
           )
       LOOP
         EXECUTE format('TRUNCATE TABLE storyos.%I CASCADE', tbl);
       END LOOP;
     END \$\$;" >/dev/null
  load_controlled_fixture "$1"
}
