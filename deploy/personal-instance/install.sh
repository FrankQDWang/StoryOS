#!/bin/sh
# Installs the personal instance on the VPS (ADR 0104): the packages of Caddy and of the PostgreSQL 17 client, the
# service user, the directories, the release package, the units, the settings of the services, and the Caddyfile.
#
# Usage: install.sh <archive> [<certificate of the database CA>]
#
# <archive> is the archive of make release-package-linux, with its .sha256 file beside it. The certificate goes
# into the trust store of the operating system. A second run changes nothing that is correct. The first package
# becomes /opt/storyos/current; upgrade.sh changes that link later.
here=$(cd "$(dirname "$0")" && pwd)
. "$here/lib.sh"
require_root
[ "$#" -ge 1 ] && [ "$#" -le 2 ] || fail "usage: install.sh <archive> [<certificate of the database CA>]"
install -d -m 711 "$etc"
install -d -m 700 "$etc/secrets"
if [ ! -f "$etc/instance.env" ]; then
  write_file "$etc/instance.env" root 600 "$(cat "$here/instance.env.example")"
  fail "Set each value in $etc/instance.env, then run install.sh again"
fi
chmod 600 "$etc/instance.env"
load_instance

export DEBIAN_FRONTEND=noninteractive
apt_install() {
  if [ -z "${apt_updated:-}" ]; then
    apt-get update -q
    apt_updated=1
  fi
  apt-get install -y -q --no-install-recommends "$@"
}
for command in curl gpg openssl update-ca-certificates; do
  command -v "$command" >/dev/null || apt_install ca-certificates curl gpg openssl
done
if ! command -v caddy >/dev/null; then
  # The package source of Caddy: https://caddyserver.com/docs/install#debian-ubuntu-raspbian
  curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/gpg.key \
    | gpg --dearmor --yes -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -1sLf https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt > /etc/apt/sources.list.d/caddy-stable.list
  chmod 644 /usr/share/keyrings/caddy-stable-archive-keyring.gpg /etc/apt/sources.list.d/caddy-stable.list
  apt_updated=
  apt_install caddy
fi
if ! pg_dump --version 2>/dev/null | grep -q ' 17\.'; then
  # The package source of PostgreSQL: https://www.postgresql.org/download/linux/ubuntu/
  apt_install postgresql-common
  /usr/share/postgresql-common/pgdg/apt.postgresql.org.sh -y
  apt_updated=
  apt_install postgresql-client-17
fi

if ! id "$service_user" >/dev/null 2>&1; then
  useradd --system --user-group --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin "$service_user"
fi
install -d -m 755 "$opt" "$opt/releases" "$opt/deploy"
install -d -m 700 "$backup_dir"
# A run that stops after a change and before its reloads leaves this marker, so that the next run does them.
pending=/run/storyos-install-pending
restart=0
[ ! -f "$pending" ] || restart=1
: > "$pending"

if [ "$#" = 2 ]; then
  openssl x509 -in "$2" -noout 2>/dev/null || fail "$2 is not a certificate in the PEM form"
  write_file /usr/local/share/ca-certificates/storyos-database.crt root 644 "$(cat "$2")"
  restart=$((restart + changed))
fi

release=$(install_release "$1")
if [ ! -e "$opt/current" ]; then
  ln -s "$release" "$opt/current"
fi
# The timer of the backup runs this copy, which belongs to root.
for file in lib.sh backup.sh; do
  write_file "$opt/deploy/$file" root 755 "$(cat "$here/$file")"
done

units=0
for unit in storyos-server.service storyos-worker.service storyos-backup.service storyos-backup.timer; do
  write_file "/etc/systemd/system/$unit" root 644 "$(sed "s/@HOST@/$STORYOS_HOST/g" "$here/$unit")"
  units=$((units + changed))
done
if [ "$units" -gt 0 ]; then
  systemctl daemon-reload
  restart=$((restart + 1))
fi
systemctl enable storyos-server.service storyos-worker.service >/dev/null 2>&1
systemctl enable --now storyos-backup.timer >/dev/null 2>&1

absent=
for name in $asked_secrets $made_secrets; do
  [ -s "$(secret_file "$name")" ] || absent="$absent $name"
done
[ -s "$etc/user-id" ] || absent="$absent user-id"
[ -z "$absent" ] || fail "these secrets are absent:$absent. Run secrets.sh, then run install.sh again"

runtime_password=$(read_secret runtime-password)
runtime_url=$(database_url "$STORYOS_DATABASE_RUNTIME_USER" "$runtime_password")
session_token=$(read_secret session-token)
user_id=$(cat "$etc/user-id")
challenge_secret=$(read_secret challenge-secret)
write_file "$etc/server.env" root 600 "$(printf "%s='%s'\n" \
  STORYOS_DATABASE_URL "$runtime_url" \
  STORYOS_BOOTSTRAP_SESSIONS "{\"$session_token\":\"$user_id\"}" \
  STORYOS_CHALLENGE_SECRET "$challenge_secret")"
restart=$((restart + changed))
model_key=$(read_secret model-key)
web_search_key=$(read_secret web-search-key)
# Only the Worker reads this file (ADR 0050), so it belongs to the service user.
write_file "$etc/worker.env" "$service_user" 600 "$(printf "%s='%s'\n" \
  STORYOS_DATABASE_URL "$runtime_url" \
  STORYOS_AGENT_PLAN_API_KEY "$model_key" \
  STORYOS_EXA_API_KEY "$web_search_key")"
restart=$((restart + changed))

if [ -f /etc/caddy/Caddyfile ] && [ ! -f /etc/caddy/Caddyfile.before-storyos ] \
  && ! grep -q 'proxy-auth.caddy' /etc/caddy/Caddyfile; then
  cp -p /etc/caddy/Caddyfile /etc/caddy/Caddyfile.before-storyos
  echo "Kept the earlier Caddyfile as /etc/caddy/Caddyfile.before-storyos"
fi
# The digest of the proxy file makes the Caddyfile change, and Caddy reload, when the proxy password changes.
proxy_digest=$(sha256sum < "$(secret_file proxy-password)" | cut -c 1-64)
caddyfile=$(printf '# The SHA-256 of proxy-auth.caddy: %s\n' "$proxy_digest"
  sed "s/@HOST@/$STORYOS_HOST/g" "$here/Caddyfile.template")
candidate=$(mktemp /etc/caddy/Caddyfile.XXXXXX)
printf '%s\n' "$caddyfile" > "$candidate"
caddy validate --adapter caddyfile --config "$candidate" >/dev/null 2>&1 || {
  rm -f "$candidate"
  fail "Caddy refused the Caddyfile of the template"
}
rm -f "$candidate"
write_file /etc/caddy/Caddyfile root 644 "$caddyfile"
restart=$((restart + changed))

if [ "$restart" -gt 0 ]; then
  update-ca-certificates >/dev/null
  systemctl daemon-reload
  systemctl reload-or-restart caddy
  systemctl try-restart storyos-server.service storyos-worker.service
fi
systemctl is-active --quiet caddy || systemctl start caddy
rm -f "$pending"
echo "The instance is installed. For a new database, run activate.sh"
