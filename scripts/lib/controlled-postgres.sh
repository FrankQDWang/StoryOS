# Source after setting repository_root; shared by verify-project-scope.sh and dev-postgres.sh.

start_postgres() {
  name=$1
  docker run --detach --name "$name" \
    --env POSTGRES_PASSWORD=admin \
    --publish 127.0.0.1::5432 postgres:16-alpine >/dev/null
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
