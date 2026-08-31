#!/bin/sh
set -eu

# Isolated physical Recovery Copy drill. Archive and base-backup volumes are not
# primary PGDATA. Ordinary StoryOS reads stay disabled: the restored copy remains
# in recovery_hold without Recovery Visibility Proof.

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

suffix=$$
network="storyos-recovery-net-$suffix"
primary="storyos-recovery-primary-$suffix"
hold="storyos-recovery-hold-$suffix"
archive_volume="storyos-recovery-archive-$suffix"
backup_volume="storyos-recovery-backup-$suffix"
hold_volume="storyos-recovery-hold-data-$suffix"
wal_marker="WAL after base backup"
project_id="018f0000-0000-7001-8000-000000000002"

if [ -n "${STORYOS_RECOVERY_COPY_DIR:-}" ] && [ -f "${STORYOS_RECOVERY_COPY_DIR}/PG_VERSION" ]; then
  echo "Recovery Copy destination must not be PostgreSQL PGDATA" >&2
  exit 1
fi

cleanup() {
  docker rm -f "$primary" "$hold" >/dev/null 2>&1 || true
  docker volume rm "$archive_volume" "$backup_volume" "$hold_volume" >/dev/null 2>&1 || true
  docker network rm "$network" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM
cleanup

docker network create "$network" >/dev/null
docker volume create "$archive_volume" >/dev/null
docker volume create "$backup_volume" >/dev/null
docker volume create "$hold_volume" >/dev/null
docker run --rm --user root --volume "$archive_volume:/archive" postgres:16-alpine \
  chown postgres:postgres /archive

docker run --detach --name "$primary" \
  --network "$network" \
  --env POSTGRES_PASSWORD=admin \
  --volume "$archive_volume:/var/lib/postgresql/wal_archive" \
  postgres:16-alpine \
  -c wal_level=replica \
  -c archive_mode=on \
  -c "archive_command=test ! -f /var/lib/postgresql/wal_archive/%f && cp %p /var/lib/postgresql/wal_archive/%f" \
  -c max_wal_senders=4 \
  -c wal_keep_size=64MB >/dev/null

wait_postgres() {
  container=$1
  require_init=$2
  attempt=0
  until {
      if [ "$require_init" = "init" ]; then
        docker logs "$container" 2>&1 | grep -q "PostgreSQL init process complete"
      else
        docker logs "$container" 2>&1 | grep -Eq "ready to accept (read-only )?connections"
      fi
    } && docker exec "$container" pg_isready -U postgres >/dev/null 2>&1; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 80 ]; then
      docker logs "$container" >&2
      echo "PostgreSQL did not become ready: $container" >&2
      exit 1
    fi
    sleep 0.25
  done
}

wait_postgres "$primary" init

docker cp "$repository_root/crates/storyos-adapter-postgres/migrations/." \
  "$primary:/tmp/storyos-release1-bootstrap" >/dev/null
psql_files=""
for file in "$repository_root/crates/storyos-adapter-postgres/migrations/"*.sql; do
  psql_files="$psql_files -f /tmp/storyos-release1-bootstrap/$(basename "$file")"
done
# shellcheck disable=SC2086
docker exec "$primary" psql -X -v ON_ERROR_STOP=1 --single-transaction -U postgres \
  $psql_files >/dev/null
docker exec -i "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres \
  < "$repository_root/crates/storyos-adapter-postgres/tests/fixture.sql" >/dev/null

role_count=$(docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT count(*) FROM pg_roles WHERE rolname IN ('storyos_backup', 'storyos_restore')")
if [ "$role_count" != "2" ]; then
  echo "Maintenance backup and restore roles are missing" >&2
  exit 1
fi
backup_replication=$(docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT rolreplication::text || '/' || rolsuper::text || '/' || rolbypassrls::text
     FROM pg_roles WHERE rolname = 'storyos_backup'")
if [ "$backup_replication" != "true/false/false" ]; then
  echo "storyos_backup must be REPLICATION without superuser or BYPASSRLS: $backup_replication" >&2
  exit 1
fi

docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres \
  -c "ALTER ROLE storyos_runtime PASSWORD 'runtime'" \
  -c "ALTER ROLE storyos_backup PASSWORD 'backup'" \
  -c "ALTER ROLE storyos_restore PASSWORD 'restore'" >/dev/null
docker exec "$primary" sh -c \
  "printf '%s\n' 'host replication storyos_backup 0.0.0.0/0 scram-sha-256' \
    'host replication storyos_backup ::/0 scram-sha-256' \
    >> /var/lib/postgresql/data/pg_hba.conf"
docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres -c "SELECT pg_reload_conf()" >/dev/null

wal_level=$(docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc "SHOW wal_level")
archive_mode=$(docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc "SHOW archive_mode")
if [ "$wal_level" != "replica" ] || [ "$archive_mode" != "on" ]; then
  echo "Physical WAL archive is not enabled: wal_level=$wal_level archive_mode=$archive_mode" >&2
  exit 1
fi

docker run --rm --user root --volume "$backup_volume:/backup" postgres:16-alpine \
  chown postgres:postgres /backup
docker run --rm --network "$network" --user postgres \
  --volume "$backup_volume:/backup" \
  --env PGPASSWORD=backup \
  --entrypoint pg_basebackup \
  postgres:16-alpine \
  -h "$primary" -U storyos_backup -D /backup -Fp -Xs --no-password >/dev/null

docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres \
  -c "UPDATE storyos.projects SET title = '$wal_marker' WHERE project_id = '$project_id'" >/dev/null
target_lsn=$(docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT pg_current_wal_lsn()")
archived_wal=$(docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT pg_walfile_name('$target_lsn'::pg_lsn)")
docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres -c "SELECT pg_switch_wal()" >/dev/null
attempt=0
until docker exec "$primary" test -f "/var/lib/postgresql/wal_archive/$archived_wal"; do
  attempt=$((attempt + 1))
  if [ "$attempt" -ge 80 ]; then
    docker exec "$primary" psql -X -U postgres -c "SELECT * FROM pg_stat_archiver" >&2 || true
    docker exec "$primary" ls -la /var/lib/postgresql/wal_archive >&2 || true
    echo "WAL archive did not receive $archived_wal" >&2
    exit 1
  fi
  sleep 0.25
done

docker run --rm --volume "$backup_volume:/backup" postgres:16-alpine sh -c '
  set -eu
  test -f /backup/backup_label
  grep -q "START WAL LOCATION" /backup/backup_label
  if find /backup \( -name "*.sql" -o -name toc.dat \) | grep -q .; then
    echo "Logical dump artifacts are not a Recovery Copy" >&2
    exit 1
  fi
'

primary_data_volume=$(docker inspect -f \
  '{{range .Mounts}}{{if eq .Destination "/var/lib/postgresql/data"}}{{.Name}}{{end}}{{end}}' \
  "$primary")
primary_archive_mount=$(docker inspect -f \
  '{{range .Mounts}}{{if eq .Destination "/var/lib/postgresql/wal_archive"}}{{.Name}}{{end}}{{end}}' \
  "$primary")
if [ -z "$primary_data_volume" ] || [ "$primary_data_volume" = "$primary_archive_mount" ] \
  || [ "$primary_data_volume" = "$backup_volume" ] || [ "$primary_archive_mount" != "$archive_volume" ]; then
  echo "Recovery Copy chain shares the primary data volume" >&2
  exit 1
fi

chain_manifest=$(mktemp "${TMPDIR:-/tmp}/storyos-recovery-chain.XXXXXX")
saved_wal=$(mktemp "${TMPDIR:-/tmp}/storyos-recovery-wal.XXXXXX")
docker run --rm --volume "$archive_volume:/archive" --volume "$backup_volume:/backup" \
  postgres:16-alpine sh -c 'sha256sum /backup/backup_label /archive/*' >"$chain_manifest"

assert_recovery_chain() {
  docker run --rm \
    --volume "$archive_volume:/archive" \
    --volume "$backup_volume:/backup" \
    --volume "$chain_manifest:/manifest:ro" \
    postgres:16-alpine sha256sum -c /manifest >/dev/null
}

assert_recovery_chain

chown_archive() {
  docker exec -u root "$primary" sh -c \
    'chown -R postgres:postgres /var/lib/postgresql/wal_archive && chmod -R a+r /var/lib/postgresql/wal_archive'
}

docker exec "$primary" test -f "/var/lib/postgresql/wal_archive/$archived_wal"
docker cp "$primary:/var/lib/postgresql/wal_archive/$archived_wal" "$saved_wal"
docker exec "$primary" rm "/var/lib/postgresql/wal_archive/$archived_wal"
if assert_recovery_chain 2>/dev/null; then
  echo "A missing Recovery Copy chain member was accepted" >&2
  exit 1
fi
docker cp "$saved_wal" "$primary:/var/lib/postgresql/wal_archive/$archived_wal"
chown_archive
assert_recovery_chain

docker exec "$primary" sh -c "dd if=/dev/zero of=/var/lib/postgresql/wal_archive/$archived_wal bs=1 count=8 conv=notrunc >/dev/null 2>&1"
if assert_recovery_chain 2>/dev/null; then
  echo "A corrupt Recovery Copy chain member was accepted" >&2
  exit 1
fi
docker cp "$saved_wal" "$primary:/var/lib/postgresql/wal_archive/$archived_wal"
chown_archive
assert_recovery_chain
rm -f "$saved_wal"

docker run --rm --user root \
  --volume "$backup_volume:/backup" \
  --volume "$hold_volume:/hold" \
  postgres:16-alpine sh -c 'cp -a /backup/. /hold/ && chown -R postgres:postgres /hold'
restore_conf=$(mktemp "${TMPDIR:-/tmp}/storyos-restore-conf.XXXXXX")
cat >"$restore_conf" <<EOF
restore_command = 'cp /var/lib/postgresql/wal_archive/%f %p'
recovery_target_lsn = '$target_lsn'
recovery_target_inclusive = on
recovery_target_action = 'promote'
default_transaction_read_only = on
EOF
docker run --rm --user root \
  --volume "$restore_conf:/tmp/restore.conf:ro" \
  --volume "$hold_volume:/hold" \
  postgres:16-alpine sh -c '
    cat /tmp/restore.conf >> /hold/postgresql.auto.conf
    touch /hold/recovery.signal
    rm -f /hold/standby.signal
    chown postgres:postgres /hold/postgresql.auto.conf /hold/recovery.signal
  '
rm -f "$restore_conf"

docker run --detach --name "$hold" \
  --network "$network" \
  --volume "$hold_volume:/var/lib/postgresql/data" \
  --volume "$archive_volume:/var/lib/postgresql/wal_archive:ro" \
  postgres:16-alpine >/dev/null
wait_postgres "$hold" restore

docker exec -e PGOPTIONS="-c default_transaction_read_only=off" "$hold" \
  psql -X -v ON_ERROR_STOP=1 -U postgres -c \
  "ALTER ROLE storyos_runtime NOLOGIN;
   GRANT USAGE ON SCHEMA storyos TO storyos_restore;
   GRANT SELECT ON storyos.projects TO storyos_restore;" >/dev/null

read_only=$(docker exec "$hold" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SHOW default_transaction_read_only")
if [ "$read_only" != "on" ]; then
  echo "Isolated restore did not keep ordinary execution disabled" >&2
  exit 1
fi

if docker exec "$hold" env PGPASSWORD=runtime \
  psql -X -v ON_ERROR_STOP=1 -U storyos_runtime -d postgres -c "SELECT 1" >/dev/null 2>&1; then
  echo "storyos_runtime still executes on a recovery-hold restore" >&2
  exit 1
fi

restored_title=$(docker exec "$hold" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT title FROM storyos.projects WHERE project_id = '$project_id'")
if [ "$restored_title" != "$wal_marker" ]; then
  echo "Isolated restore did not replay archived WAL: $restored_title" >&2
  exit 1
fi

if ! docker exec "$hold" env PGPASSWORD=restore \
  psql -X -v ON_ERROR_STOP=1 -U storyos_restore -d postgres -c "SELECT 1" >/dev/null; then
  echo "storyos_restore cannot connect to the isolated restore" >&2
  exit 1
fi

if docker exec "$hold" env PGPASSWORD=restore \
  psql -X -v ON_ERROR_STOP=1 -U storyos_restore -d postgres \
  -c "INSERT INTO storyos.projects(owner_user_id, project_id, title) VALUES (
        '018f0000-0000-7001-8000-000000000001',
        '018f0000-0000-7001-8000-00000000ffff',
        'must not write'
      )" >/dev/null 2>&1; then
  echo "Isolated restore accepted an ordinary write" >&2
  exit 1
fi

role_identities=$(docker exec "$primary" psql -X -v ON_ERROR_STOP=1 -U postgres -Atc \
  "SELECT string_agg(rolname || ':login=' || rolcanlogin::text || ',super=' || rolsuper::text ||
                     ',bypass=' || rolbypassrls::text || ',repl=' || rolreplication::text, '; '
                     ORDER BY rolname)
     FROM pg_roles
     WHERE rolname IN ('storyos_backup', 'storyos_restore', 'storyos_runtime')")
evidence=$(mktemp "${TMPDIR:-/tmp}/storyos-recovery-hold.XXXXXX")
python3 - "$evidence" "$wal_level" "$archive_mode" "$primary_data_volume" "$archive_volume" \
  "$backup_volume" "$archived_wal" "$role_identities" "$chain_manifest" <<'PY'
from pathlib import Path
import hashlib
import json
import re
import sys

(
    dest,
    wal_level,
    archive_mode,
    data_volume,
    archive_volume,
    backup_volume,
    archived_wal,
    role_identities,
    manifest_path,
) = sys.argv[1:]
members = []
for line in Path(manifest_path).read_text(encoding="utf-8").splitlines():
    digest, path = line.split(None, 1)
    members.append({"path": path, "sha256": digest})
payload = {
    "schema_id": "storyos.recovery-copy.hold-evidence.v1",
    "method": "pg_basebackup",
    "state": "recovery_hold",
    "visibility_proof": "absent",
    "ordinary_read_execution": "disabled",
    "wal_level": wal_level,
    "archive_mode": archive_mode,
    "required_wal_member": archived_wal,
    "role_identities": role_identities,
    "failure_domain": {
        "primary_data_volume": data_volume,
        "archive_volume": archive_volume,
        "backup_volume": backup_volume,
    },
    "chain": members,
}
text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
if re.search(r"password|secret|pgpassword", text, re.I):
    raise SystemExit("Recovery evidence contains credential material")
Path(dest).write_text(text, encoding="utf-8")
print("sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest())
PY

echo "Isolated Recovery Copy restore remains in hold"
rm -f "$chain_manifest" "$evidence"
