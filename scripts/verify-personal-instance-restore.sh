#!/bin/sh
# The restore drill of the personal instance (issue 1855, ADR 0104 section 7), a lane of make check-full.
#
# It activates a new database with storyos-storage of target/release-package, writes one Project with one Chapter
# through the packaged Server, runs deploy/personal-instance/backup.sh, runs restore.sh into a second new
# database of the provider image (its admin login is not a superuser), starts the packaged Server on it with no run
# of storyos-storage, and reads the Chapter with the same text. The two scripts run in a container of the PostgreSQL image, which has psql and pg_dump of PostgreSQL 17.
set -eu
repository_root=$(cd "$(dirname "$0")/.." && pwd)
. "$repository_root/scripts/lib/controlled-postgres.sh"
package=$repository_root/target/release-package
[ -x "$package/storyos-server" ] || { echo "make release-package must run first" >&2; exit 1; }
drill=storyos-personal-restore-$$
work=$(mktemp -d "${TMPDIR:-/tmp}/storyos-personal-restore.XXXXXX")
server_pid=
cleanup() {
  [ -z "$server_pid" ] || kill "$server_pid" 2>/dev/null || true
  docker rm -fv "$drill-a" "$drill-b" >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 143' TERM
trap 'exit 130' INT

user_id=018f0000-0000-7001-8000-0000000f0101
runtime_password=$(openssl rand -hex 32)
text="The text of the restore drill — 恢复演练的正文。"
mkdir -p "$work/deploy" "$work/etc/secrets" "$work/backups"
cp "$repository_root"/deploy/personal-instance/* "$work/deploy/"
cp "$repository_root/crates/storyos-adapter-postgres/migrations/0000_roles.sql" "$work/deploy/"
printf '%s\n' STORYOS_HOST=storyos.localhost STORYOS_PROXY_USER=author STORYOS_DATABASE_HOST=127.0.0.1 \
  STORYOS_DATABASE_PORT=5432 STORYOS_DATABASE_NAME=postgres STORYOS_DATABASE_ADMIN_USER=postgres \
  STORYOS_DATABASE_RUNTIME_USER=storyos_runtime STORYOS_DATABASE_TLS=none > "$work/etc/instance.env"
printf admin > "$work/etc/secrets/database-admin-password"
printf '%s' "$runtime_password" > "$work/etc/secrets/runtime-password"

# Runs one script of deploy/personal-instance in the network of the database container $1.
run_script() {
  container=$1
  shift
  docker run --rm --network "container:$container" \
    ${STORYOS_CHECK_RUN:+--label "storyos.check-run=$STORYOS_CHECK_RUN"} \
    --volume "$work/deploy:/deploy:ro" --volume "$work/etc:/etc/storyos-test:ro" --volume "$work/backups:/backups" \
    --env STORYOS_ETC=/etc/storyos-test --env STORYOS_BACKUP_DIR=/backups "$storyos_postgres_image" "$@"
}

start_server() {
  log=$work/server-$1.log
  : > "$log"
  published=$(docker port "$1" 5432/tcp)
  STORYOS_DATABASE_URL="postgres://storyos_runtime:$runtime_password@127.0.0.1:${published##*:}/postgres" \
  STORYOS_BOOTSTRAP_SESSIONS="{\"drill-session\":\"$user_id\"}" \
  STORYOS_CHALLENGE_SECRET=$(openssl rand -hex 32) STORYOS_WORKER=0 \
    "$package/storyos-server" --bind 127.0.0.1:0 --web-root "$package/web" > "$log" 2>&1 &
  server_pid=$!
  attempt=0
  until grep -q '^STORYOS_SERVER_URL=http://' "$log"; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 200 ] || ! kill -0 "$server_pid" 2>/dev/null; then
      cat "$log" >&2
      echo "The packaged Server did not start on $1" >&2
      exit 1
    fi
    sleep 0.05
  done
  server_url=$(sed -n 's/^STORYOS_SERVER_URL=//p' "$log" | head -n 1)
}

stop_server() {
  kill "$server_pid"
  wait "$server_pid" || true
  server_pid=
}

start_postgres "$drill-a"
STORYOS_STORAGE_ADMIN_URL=$(postgres_admin_url "$drill-a") "$package/storyos-storage"
docker exec -i "$drill-a" psql -X -q -v ON_ERROR_STOP=1 -U postgres <<EOF
ALTER ROLE storyos_runtime PASSWORD '$runtime_password';
INSERT INTO storyos.users(user_id) VALUES ('$user_id');
EOF
start_server "$drill-a"
ids=$(node "$repository_root/scripts/personal-instance-chapter.mjs" write "$server_url" drill-session "$text")
stop_server
run_script "$drill-a" /deploy/backup.sh
dump=$(cd "$work/backups" && ls storyos-*.dump)
[ "$(printf '%s\n' "$dump" | wc -l | tr -d ' ')" = 1 ] || { echo "backup.sh did not write one dump" >&2; exit 1; }
echo "backup.sh wrote $dump"

start_postgres "$drill-b" "$storyos_provider_postgres_image"
run_script "$drill-b" /deploy/restore.sh "/backups/$dump"
start_server "$drill-b"
read_text=$(node "$repository_root/scripts/personal-instance-chapter.mjs" read "$server_url" drill-session $ids)
stop_server
if [ "$read_text" != "$text" ]; then
  echo "The restored Chapter has another text: $read_text" >&2
  exit 1
fi
echo "The restore drill of the personal instance read the same Chapter text"
