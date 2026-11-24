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
  # The stream check of the inner part sends the golden Project Activity event of the public release as one frame.
  python3 - "$repository_root/generated/golden-wire/storyos-public-release-1/activity-stream.json" \
    > "$work/activity-event" <<'PYTHON'
import json
import sys

frame = json.load(open(sys.argv[1], encoding="utf-8"))
data = json.dumps(frame["data"], separators=(",", ":"), ensure_ascii=False)
sys.stdout.write(f"id: {frame['id']}\nevent: {frame['event']}\ndata: {data}\n\n")
PYTHON
  # Each secret is a value of this run. The admin password has the characters that a URL must encode.
  admin_password="a $(openssl rand -hex 8)%@:/?#"
  (umask 077 && printf '%s\n' "$admin_password" "p$(openssl rand -hex 12)" "m$(openssl rand -hex 16)" \
    "w$(openssl rand -hex 16)" "m$(openssl rand -hex 16)" "p$(openssl rand -hex 12)" > "$work/secrets")
  start_postgres "$check-db"
  docker exec -i "$check-db" psql -X -q -v ON_ERROR_STOP=1 -U postgres <<EOF
ALTER ROLE postgres PASSWORD '$admin_password';
EOF
  # --init reaps each stopped service, so that the fake systemctl sees that it ended. The inner part has a time
  # limit. At the limit, timeout sends TERM, then KILL after 10 s, and the container ends with the inner part.
  # timeout prints each signal that it sends.
  limit=1200
  status=0
  docker run --rm --init --name "$check" --platform linux/amd64 --network "container:$check-db" \
    --volume "$work:/check:ro" ${STORYOS_CHECK_RUN:+--label "storyos.check-run=$STORYOS_CHECK_RUN"} \
    ubuntu:24.04@sha256:562111b88caa79c71391c0c4d3c2114a7d5c35915c20561fd893cb1e07929e6b \
    timeout --verbose --kill-after=10 "$limit" sh /check/verify-personal-instance.sh --inside || status=$?
  case $status in
    124) echo "FAIL: the inner part of the check did not end in $limit s" >&2 ;;
    137) echo "FAIL: the inner part of the check ended with SIGKILL" >&2 ;;
  esac
  exit "$status"
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
[ "$(stat -c %U /etc/storyos/worker.env /etc/storyos/proxy-auth.caddy | tr '\n' ' ')" = "storyos-worker caddy " ] \
  || fail "worker.env must belong to storyos-worker and proxy-auth.caddy to caddy"

step "The Server and the Worker run as two users, and the user of the Server cannot read worker.env"
# The process of a service starts as root and changes its user, so the check waits at most 5 s for the user $2.
expect_user() {
  attempt=0
  until [ "$(ps -o user= -p "$(cat "/run/fake-systemd/$1.pid")")" = "$2" ]; do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 50 ] || fail "$1 does not run as $2"
    sleep 0.1
  done
}
expect_user storyos-server storyos
expect_user storyos-worker storyos-worker
if setpriv --reuid=storyos --regid=storyos --init-groups cat /etc/storyos/worker.env >/dev/null 2>&1; then
  fail "the user of the Server can read worker.env"
fi

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

# Reads the path $1 through Caddy with the password and the curl arguments after $1. The headers go to /tmp/headers
# and the body goes to /tmp/body, with no decompression.
read_through_proxy() {
  path=$1
  shift
  curl -fsSk --resolve "$host:443:127.0.0.1" --netrc-file /tmp/netrc -D /tmp/headers -o /tmp/body "$@" \
    "https://$host$path"
}
header_value() { tr -d '\r' < /tmp/headers | sed -n "s/^$1: *//Ip"; }
# install.sh restarts the Server with no wait when the Caddyfile changes, so a read waits until the Server answers.
await_server() {
  attempt=0
  until curl -fsS --max-time 5 -o /dev/null http://127.0.0.1:3000/api/v1/protocol 2>/dev/null; do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 60 ] || fail "the Server does not answer on 127.0.0.1:3000"
    sleep 1
  done
}
script=$(grep -o '/assets/[^"]*\.js' /opt/storyos/current/web/index.html | head -n 1)
[ -n "$script" ] || fail "index.html names no Web script"

step "An instance with the Caddyfile of today sends the Web script with no compression"
# The Caddyfile of today is the template with no encode block.
mkdir /tmp/today
find "$deploy" -maxdepth 1 -type f ! -name '*.tar.gz*' -exec cp {} /tmp/today/ \;
sed -i '/^\t# Compress the document/,/^\t}$/d' /tmp/today/Caddyfile.template
! grep -q encode /tmp/today/Caddyfile.template || fail "the Caddyfile of today has an encode block"
run 0 sh /tmp/today/install.sh "$archive"
await_server
read_through_proxy "$script" -H 'Accept-Encoding: gzip'
[ -z "$(header_value content-encoding)" ] || fail "the Caddyfile of today compressed the Web script"

step "install.sh, the step of the upgrade, gives the instance the compression of the template"
run 0 sh "$deploy/install.sh" "$archive"
await_server
# Decompresses /tmp/body with gzip and compares the result with the file $1. A damaged body makes gzip fail.
decompressed_body_is() { gzip -dc /tmp/body > /tmp/plain && cmp -s /tmp/plain "$1"; }
read_through_proxy "$script" -H 'Accept-Encoding: gzip'
[ "$(header_value content-encoding)" = gzip ] || fail "the Web script is not compressed"
decompressed_body_is "/opt/storyos/current/web$script" || fail "the decompressed Web script is not the script"
proxied_cache=$(header_value cache-control)
curl -fsS -D /tmp/headers -o /dev/null "http://127.0.0.1:3000$script"
[ "$proxied_cache" = "$(header_value cache-control)" ] || fail "Caddy changed the cache header of the Web script"
read_through_proxy "$script"
[ -z "$(header_value content-encoding)" ] || fail "a request with no Accept-Encoding got a compressed Web script"
cmp -s /tmp/body "/opt/storyos/current/web$script" || fail "the plain Web script is not the script"
read_through_proxy / -H 'Accept-Encoding: gzip'
[ "$(header_value content-encoding)" = gzip ] || fail "the HTML document is not compressed"
decompressed_body_is /opt/storyos/current/web/index.html || fail "the decompressed HTML document is not index.html"
read_through_proxy /api/v1/protocol -H 'Accept-Encoding: gzip'
[ "$(header_value content-encoding)" = gzip ] || fail "the JSON response is not compressed"
curl -fsS -o /tmp/direct.json http://127.0.0.1:3000/api/v1/protocol
decompressed_body_is /tmp/direct.json || fail "the decompressed JSON response is not the response"

step "An event of the Project Activity Stream goes through Caddy before the stream ends"
# A stub in place of the Server answers only the route of the stream of the golden event. It sends that complete
# event, which is larger than the minimum of the encode block, and then it keeps the stream open for 30 s.
cp /check/activity-event /tmp/event
[ "$(wc -c < /tmp/event)" -gt 256 ] || fail "the golden event is not larger than the minimum of the encode block"
project=$(grep -o '"project_id":"[^"]*"' /tmp/event | head -n 1 | cut -d '"' -f 4)
[ -n "$project" ] || fail "the golden event has no project_id"
activity="/api/v1/projects/$project/activity?snapshot_id=019a0000-0000-7000-8000-000000000002&protocol_release=storyos.public.release.1"
listening() { grep -q ':0BB8 00000000:0000 0A' /proc/net/tcp; }
systemctl stop storyos-server.service
attempt=0
while listening; do
  attempt=$((attempt + 1))
  [ "$attempt" -lt 50 ] || fail "the Server did not stop"
  sleep 0.1
done
STUB_PATH=$activity perl -MIO::Socket::INET -e '
  my $server = IO::Socket::INET->new(LocalAddr => "127.0.0.1:3000", Listen => 1, ReuseAddr => 1) or die "$!\n";
  my $client = $server->accept or die "$!\n";
  my $request = <$client>;
  while (my $line = <$client>) { last if $line =~ /^\r?\n$/ }
  $client->autoflush(1);
  if ($request !~ /^GET \Q$ENV{STUB_PATH}\E HTTP\/1\.1\r?\n$/) {
    print $client "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
    exit;
  }
  open(my $event, "<", "/tmp/event") or die "$!\n";
  print $client "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\r\n", <$event>;
  sleep 30;' &
stub=$!
attempt=0
until listening; do
  attempt=$((attempt + 1))
  [ "$attempt" -lt 50 ] || fail "the stub does not listen"
  sleep 0.1
done
status=0
read_through_proxy "$activity" -N --max-time 5 -H 'Accept-Encoding: gzip' -H 'Accept: text/event-stream' || status=$?
kill "$stub" 2>/dev/null || true
wait "$stub" 2>/dev/null || true
[ "$status" = 28 ] || fail "the stream ended or failed before the time limit with status $status"
[ -z "$(header_value content-encoding)" ] || fail "Caddy compressed the Project Activity Stream"
cmp -s /tmp/body /tmp/event || fail "the complete event did not arrive in 5 s"
systemctl start storyos-server.service
await_server

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

# Makes the archive /tmp/<$1>/storyos-linux-x86_64-<$1>.tar.gz of the current package, with a storyos-storage that
# is the shell script $2.
storage_package() {
  package=storyos-linux-x86_64-$1
  mkdir "/tmp/$1"
  cp -a "$(readlink -f /opt/storyos/current)" "/tmp/$1/$package"
  printf '#!/bin/sh\n%s\n' "$2" > "/tmp/$1/$package/storyos-storage"
  (cd "/tmp/$1" && tar -czf "$package.tar.gz" "$package" && sha256sum "$package.tar.gz" > "$package.tar.gz.sha256")
}

step "upgrade.sh with a package that storyos-storage refuses on each run keeps the old package"
storage_package refused 'echo STORYOS_STORAGE_ACTIVATION=Refused; exit 1'
run 1 sh "$deploy/upgrade.sh" /tmp/refused/storyos-linux-x86_64-refused.tar.gz
[ "$(grep -c '^STORYOS_STORAGE_ACTIVATION=Refused$' /tmp/last)" = 2 ] \
  || fail "upgrade.sh did not run storyos-storage two times"
[ "$(readlink -f /opt/storyos/current)" = "$current" ] || fail "the refused upgrade changed the link"
curl -fsS -o /dev/null http://127.0.0.1:3000/api/v1/protocol || fail "the old package does not answer"

# A package of this build refuses each database of an earlier package (the ledger of #1852), so the upgrade line
# comes from a storyos-storage that prints it.
step "upgrade.sh accepts the upgrade line of storyos-storage"
storage_package upgraded \
  "printf '%s\n' STORYOS_STORAGE_ACTIVATION=Active 'STORYOS_STORAGE_UPGRADE=from 2 files to 3 files'"
run 0 sh "$deploy/upgrade.sh" /tmp/upgraded/storyos-linux-x86_64-upgraded.tar.gz
grep -q '^STORYOS_STORAGE_UPGRADE=from 2 files to 3 files$' /tmp/last || fail "upgrade.sh did not print the upgrade line"
[ "$(readlink -f /opt/storyos/current)" = /opt/storyos/releases/upgraded ] || fail "the upgrade did not change the link"
curl -fsS -o /dev/null http://127.0.0.1:3000/api/v1/protocol || fail "the upgraded package does not answer"

# A second run of a committed upgrade answers Active, so this fake fails only on its first run.
step "upgrade.sh continues when the second run of storyos-storage answers Active"
storage_package lost '[ -f /tmp/lost/ran ] || { : > /tmp/lost/ran; echo "the connection ended"; exit 1; }
echo STORYOS_STORAGE_ACTIVATION=Active'
run 0 sh "$deploy/upgrade.sh" /tmp/lost/storyos-linux-x86_64-lost.tar.gz
grep -q '^the connection ended$' /tmp/last || fail "upgrade.sh did not print the output of the first run"
[ "$(readlink -f /opt/storyos/current)" = /opt/storyos/releases/lost ] || fail "the upgrade did not change the link"
curl -fsS -o /dev/null http://127.0.0.1:3000/api/v1/protocol || fail "the new package does not answer"

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
