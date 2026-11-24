#!/bin/sh
# The check of the scripts of the personal instance (issue 1855): make verify-personal-instance.
#
# It runs install.sh, secrets.sh, activate.sh, upgrade.sh, and backup.sh of deploy/personal-instance in an Ubuntu
# 24.04 container against a PostgreSQL container, with the Linux archive of the current commit in target/. It
# needs the network for the packages of Caddy and of the PostgreSQL client. With --inside, the script is the part
# that runs in the Ubuntu container. Usage: verify-personal-instance.sh [<archive>]
set -eu
if [ "${1:-}" != --inside ]; then
  repository_root=$(cd "$(dirname "$0")/.." && pwd)
  . "$repository_root/scripts/lib/controlled-postgres.sh"
  # An argument names another archive, for a run during development.
  commit=$(git -C "$repository_root" rev-parse --short=12 HEAD)
  archive=${1:-$repository_root/target/storyos-linux-x86_64-$commit.tar.gz}
  [ -f "$archive" ] || { echo "$archive is absent. Run make release-package-linux" >&2; exit 1; }
  check=storyos-personal-check-$$
  work=$(mktemp -d "${TMPDIR:-/tmp}/storyos-personal-check.XXXXXX")
  trap 'docker rm -fv "$check" "$check-db" >/dev/null 2>&1 || true; rm -rf "$work"' EXIT
  trap 'exit 143' TERM
  trap 'exit 130' INT
  mkdir "$work/deploy"
  cp "$repository_root"/deploy/personal-instance/* "$archive" "$archive.sha256" "$work/deploy/"
  cp "$repository_root/scripts/verify-personal-instance.sh" "$repository_root/scripts/lib/fake-systemctl.sh" "$work/"
  # Each secret is a value of this run. The admin password has the characters that a URL must encode.
  admin_password="a $(openssl rand -hex 8)%@:/?#"
  (umask 077 && printf '%s\n' "$admin_password" "p$(openssl rand -hex 12)" "m$(openssl rand -hex 16)" \
    "w$(openssl rand -hex 16)" "m$(openssl rand -hex 16)" "p$(openssl rand -hex 12)" > "$work/secrets")
  start_postgres "$check-db"
  docker exec -i "$check-db" psql -X -q -v ON_ERROR_STOP=1 -U postgres <<EOF
ALTER ROLE postgres PASSWORD '$admin_password';
EOF
  # --init reaps each stopped service, so that the fake systemctl sees that it ended.
  docker run --rm --init --name "$check" --platform linux/amd64 --network "container:$check-db" \
    --volume "$work:/check:ro" ${STORYOS_CHECK_RUN:+--label "storyos.check-run=$STORYOS_CHECK_RUN"} \
    ubuntu:24.04@sha256:562111b88caa79c71391c0c4d3c2114a7d5c35915c20561fd893cb1e07929e6b \
    sh /check/verify-personal-instance.sh --inside
  exit
fi

deploy=/check/deploy
archive=$(ls "$deploy"/storyos-linux-x86_64-*.tar.gz)
host=storyos.localhost
step() { printf '\n== %s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }
# Runs one script, keeps its output in /tmp/output, and fails when the exit status is not $1.
run() {
  expected=$1
  shift
  status=0
  "$@" > /tmp/last 2>&1 || status=$?
  cat /tmp/last
  cat /tmp/last >> /tmp/output
  [ "$status" = "$expected" ] || fail "$* exited with $status, not $expected"
}
files_state() {
  find /etc/storyos /etc/systemd/system /etc/caddy /opt/storyos/deploy /usr/local/share/ca-certificates -type f \
    -exec stat -c '%n %U %a %Y' {} + | sort
}

step "Prepare the container"
export DEBIAN_FRONTEND=noninteractive
apt-get update -q > /dev/null
apt-get install -y -q --no-install-recommends procps systemd > /dev/null
install -m 755 /check/fake-systemctl.sh /usr/local/bin/systemctl
(set +e; while :; do ps -eo args >> /tmp/ps; sleep 0.1; done) &
watcher=$!

step "install.sh with no instance.env stops and writes the example"
run 1 sh "$deploy/install.sh" "$archive"
grep -q 'Set each value in /etc/storyos/instance.env' /tmp/last || fail "no message for instance.env"
printf '%s\n' "STORYOS_HOST=$host" STORYOS_PROXY_USER=author STORYOS_DATABASE_HOST=127.0.0.1 \
  STORYOS_DATABASE_PORT=5432 STORYOS_DATABASE_NAME=postgres STORYOS_DATABASE_ADMIN_USER=postgres \
  STORYOS_DATABASE_RUNTIME_USER=storyos_runtime STORYOS_DATABASE_TLS=none > /etc/storyos/instance.env

step "install.sh with no secrets installs the packages and stops"
run 1 sh "$deploy/install.sh" "$archive"
grep -q 'these secrets are absent: database-admin-password proxy-password' /tmp/last || fail "no message for secrets"
pg_dump --version | grep -q ' 17\.' || fail "the PostgreSQL 17 client is absent"

step "secrets.sh reads the secrets of the author from a pipe"
head -n 4 /check/secrets | run 0 sh "$deploy/secrets.sh"
run 0 sh "$deploy/secrets.sh"
grep -q basic_auth /etc/storyos/proxy-auth.caddy || fail "the proxy file has no hash"

step "install.sh with a CA certificate, and a second run that changes nothing"
openssl req -x509 -newkey rsa:2048 -nodes -keyout /tmp/ca.key -out /tmp/ca.crt -subj /CN=storyos-test-ca -days 1 \
  2> /dev/null
run 0 sh "$deploy/install.sh" "$archive" /tmp/ca.crt
[ -e /etc/ssl/certs/storyos-database.pem ] || fail "the CA certificate is not in the trust store"
before=$(files_state)
run 0 sh "$deploy/install.sh" "$archive" /tmp/ca.crt
[ "$(files_state)" = "$before" ] || fail "the second run of install.sh changed a file"

step "activate.sh and a second run"
run 0 sh "$deploy/activate.sh"
run 0 sh "$deploy/activate.sh"
users=$(PGPASSWORD=$(cat /etc/storyos/secrets/database-admin-password) psql -X -At -h 127.0.0.1 -U postgres \
  -c 'SELECT count(*) FROM storyos.users')
[ "$users" = 1 ] || fail "activate.sh did not make one User row"

step "secrets.sh for one secret, and install.sh restarts the Worker"
worker=$(cat /run/fake-systemd/storyos-worker.pid)
sed -n 5p /check/secrets | run 0 sh "$deploy/secrets.sh" model-key
run 0 sh "$deploy/install.sh" "$archive"
[ "$(cat /run/fake-systemd/storyos-worker.pid)" != "$worker" ] && ! kill -0 "$worker" 2>/dev/null \
  || fail "install.sh did not restart the Worker"
sed -n 5p /check/secrets > /tmp/model-key
grep -qFf /tmp/model-key /etc/storyos/worker.env || fail "worker.env has not the new model key"

step "Each file of /etc/storyos has mode 600"
[ -z "$(find /etc/storyos -type f ! -perm 600)" ] || fail "a file of /etc/storyos does not have mode 600"
[ "$(stat -c %U /etc/storyos/worker.env /etc/storyos/proxy-auth.caddy | tr '\n' ' ')" = "storyos caddy " ] \
  || fail "worker.env must belong to storyos and proxy-auth.caddy to caddy"

step "The Server answers through Caddy only with the password"
printf 'machine %s login author password %s\n' "$host" "$(sed -n 2p /check/secrets)" > /tmp/netrc
proxy() { curl -sk --resolve "$host:443:127.0.0.1" --resolve "$host:80:127.0.0.1" -o /dev/null "$@"; }
[ "$(proxy -w '%{http_code}' "https://$host/api/v1/protocol")" = 401 ] || fail "no 401 with no password"
[ "$(proxy -w '%{http_code}' --netrc-file /tmp/netrc "https://$host/api/v1/protocol")" = 200 ] \
  || fail "no 200 with the password"
proxy -D /tmp/headers --netrc-file /tmp/netrc "https://$host/"
grep -qi '^strict-transport-security: max-age=31536000' /tmp/headers || fail "no HSTS"
grep -qi '^set-cookie: storyos_session=.*Secure' /tmp/headers || fail "the document GET issued no Secure cookie"
[ "$(proxy -w '%{http_code}' "http://$host/")" = 308 ] || fail "port 80 does not redirect"
sed -n 6p /check/secrets | run 0 sh "$deploy/secrets.sh" proxy-password
run 0 sh "$deploy/install.sh" "$archive" /tmp/ca.crt
[ "$(proxy -w '%{http_code}' --netrc-file /tmp/netrc "https://$host/api/v1/protocol")" = 401 ] \
  || fail "the old proxy password still works"
printf 'machine %s login author password %s\n' "$host" "$(sed -n 6p /check/secrets)" > /tmp/netrc
[ "$(proxy -w '%{http_code}' --netrc-file /tmp/netrc "https://$host/api/v1/protocol")" = 200 ] \
  || fail "the new proxy password does not work"

step "backup.sh removes the dumps older than 14 days, and a failed dump keeps the older dumps"
install -m 600 /dev/null /var/backups/storyos/storyos-old.dump
install -m 600 /dev/null /var/backups/storyos/storyos-recent.dump
touch -d '15 days ago' /var/backups/storyos/storyos-old.dump
touch -d '13 days ago' /var/backups/storyos/storyos-recent.dump
run 0 sh "$deploy/backup.sh"
[ ! -e /var/backups/storyos/storyos-old.dump ] && [ -e /var/backups/storyos/storyos-recent.dump ] \
  || fail "backup.sh did not remove only the old dump"
[ -z "$(find /var/backups/storyos -type f ! -perm 600)" ] || fail "a dump does not have mode 600"
cp -a /etc/storyos /tmp/broken-etc
sed -i 's/^STORYOS_DATABASE_PORT=.*/STORYOS_DATABASE_PORT=1/' /tmp/broken-etc/instance.env
dumps=$(ls /var/backups/storyos)
run 1 env STORYOS_ETC=/tmp/broken-etc sh "$deploy/backup.sh"
[ "$(ls /var/backups/storyos)" = "$dumps" ] || fail "a failed dump changed the dumps"

step "upgrade.sh and a second run"
run 0 sh "$deploy/upgrade.sh" "$archive"
run 0 sh "$deploy/upgrade.sh" "$archive"
current=$(readlink -f /opt/storyos/current)

step "upgrade.sh with a package that storyos-storage refuses keeps the old package"
mkdir /tmp/refused
cp -a "$current" /tmp/refused/storyos-linux-x86_64-refused
printf '#!/bin/sh\necho STORYOS_STORAGE_ACTIVATION=Refused\nexit 1\n' \
  > /tmp/refused/storyos-linux-x86_64-refused/storyos-storage
(cd /tmp/refused && tar -czf storyos-linux-x86_64-refused.tar.gz storyos-linux-x86_64-refused \
  && sha256sum storyos-linux-x86_64-refused.tar.gz > storyos-linux-x86_64-refused.tar.gz.sha256)
run 1 sh "$deploy/upgrade.sh" /tmp/refused/storyos-linux-x86_64-refused.tar.gz
[ "$(readlink -f /opt/storyos/current)" = "$current" ] || fail "the refused upgrade changed the link"
curl -fsS -o /dev/null http://127.0.0.1:3000/api/v1/protocol || fail "the old package does not answer"

step "The units pass systemd-analyze verify"
systemd-analyze verify /etc/systemd/system/storyos-server.service /etc/systemd/system/storyos-worker.service \
  /etc/systemd/system/storyos-backup.service /etc/systemd/system/storyos-backup.timer

step "No secret is in the output of a script or in a process argument"
kill "$watcher" || fail "the watcher of ps stopped before the end"
grep -q 'storyos-storage' /tmp/ps || fail "the watcher of ps saw no run of storyos-storage"
for name in database-admin-password model-key web-search-key challenge-secret session-token runtime-password; do
  value=$(cat "/etc/storyos/secrets/$name")
  ! grep -qF -- "$value" /tmp/output /tmp/ps || fail "the secret $name is in the output or in ps"
done
for line in 1 2 3 4 5 6; do
  ! grep -qF -- "$(sed -n "${line}p" /check/secrets)" /tmp/output /tmp/ps || fail "test secret $line is visible"
done
# The URLs have the admin password in the percent encoding of lib.sh.
encoded=$(sed -n 1p /check/secrets | tr -d '\n' | od -An -v -tx1 | tr -d ' \n' | sed 's/../%&/g')
! grep -qiF -- "$encoded" /tmp/output /tmp/ps || fail "the encoded admin password is visible"
echo "The scripts of the personal instance passed their checks"
