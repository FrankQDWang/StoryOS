# The shared functions of the scripts of the personal instance (ADR 0104). Each script sources this file after it
# sets `here` to its own directory. The scripts run as root on the VPS, other than push.sh.
#
# No secret goes into a process argument: a secret goes from a file through a shell variable to printf, which is
# a built-in command of sh, or into the environment of one process.
set -eu

etc=${STORYOS_ETC:-/etc/storyos}
backup_dir=${STORYOS_BACKUP_DIR:-/var/backups/storyos}
opt=/opt/storyos
# The service users of the Server and of the Worker. Only the user of the Worker reads worker.env (ADR 0104 section 3).
server_user=storyos
worker_user=storyos-worker
# The secrets that the author gives, and the secrets that secrets.sh makes.
asked_secrets="database-admin-password proxy-password model-key web-search-key"
made_secrets="challenge-secret session-token runtime-password"

fail() {
  printf '%s: %s\n' "${0##*/}" "$*" >&2
  exit 1
}

require_root() {
  [ "$(id -u)" = 0 ] || fail "run this script as root"
}

# Reads the parameters of the instance. They are not secret, and the file has mode 600 as each file of $etc.
load_instance() {
  [ -f "$etc/instance.env" ] || fail "$etc/instance.env is absent. Run install.sh"
  . "$etc/instance.env"
  for name in STORYOS_HOST STORYOS_PROXY_USER STORYOS_DATABASE_HOST STORYOS_DATABASE_PORT STORYOS_DATABASE_NAME \
    STORYOS_DATABASE_ADMIN_USER STORYOS_DATABASE_RUNTIME_USER STORYOS_DATABASE_TLS; do
    eval "value=\${$name:-}"
    [ -n "$value" ] || fail "$etc/instance.env has no value for $name"
  done
  case $STORYOS_HOST in
    *[!a-z0-9.-]*) fail "STORYOS_HOST must be a DNS name in lower case" ;;
  esac
  case $STORYOS_PROXY_USER in
    *[!A-Za-z0-9._-]*) fail "STORYOS_PROXY_USER must have only letters, digits, '.', '_', and '-'" ;;
  esac
  case $STORYOS_DATABASE_TLS:$STORYOS_DATABASE_HOST in
    require:* | none:127.0.0.1) ;;
    none:*) fail "STORYOS_DATABASE_TLS=none is only for a database on 127.0.0.1" ;;
    *) fail "STORYOS_DATABASE_TLS must be require or none" ;;
  esac
}

# The file of the secret $1. The proxy password is only its hash, in the form that Caddy imports.
secret_file() {
  if [ "$1" = proxy-password ]; then
    printf '%s/proxy-auth.caddy\n' "$etc"
  else
    printf '%s/secrets/%s\n' "$etc" "$1"
  fi
}

read_secret() {
  file=$(secret_file "$1")
  [ -s "$file" ] || fail "the secret $1 is absent. Run secrets.sh $1"
  cat "$file"
}

# Writes the text $4 to the file $1 with the owner $2 and the mode $3. The file changes only when the text, the
# owner, or the mode is different. Sets `changed` to 1 or 0. The temporary file has mode 600 from mktemp.
write_file() {
  new=$(mktemp "$1.XXXXXX")
  printf '%s\n' "$4" > "$new"
  if [ -f "$1" ] && cmp -s "$new" "$1" && [ "$(stat -c '%U %a' "$1")" = "$2 $3" ]; then
    rm -f "$new"
    changed=0
  else
    chown "$2:" "$new"
    chmod "$3" "$new"
    mv -f "$new" "$1"
    changed=1
  fi
}

percent_encode() {
  printf '%s' "$1" | od -An -v -tx1 | tr -d ' \n' | sed 's/../%&/g'
}

# The URL of the user $1 with the password $2, for the binaries of the package.
database_url() {
  address=$STORYOS_DATABASE_HOST
  case $address in *:*) address="[$address]" ;; esac
  url="postgres://$(percent_encode "$1"):$(percent_encode "$2")@$address:$STORYOS_DATABASE_PORT"
  url="$url/$STORYOS_DATABASE_NAME"
  if [ "$STORYOS_DATABASE_TLS" = require ]; then
    url="$url?sslmode=require"
  fi
  printf '%s\n' "$url"
}

# Sets the environment of psql, pg_dump, and pg_restore for the admin login. These clients verify the certificate
# and the host name with the trust store of the operating system.
use_admin_connection() {
  PGPASSWORD=$(read_secret database-admin-password)
  PGHOST=$STORYOS_DATABASE_HOST PGPORT=$STORYOS_DATABASE_PORT PGDATABASE=$STORYOS_DATABASE_NAME
  PGUSER=$STORYOS_DATABASE_ADMIN_USER PGSSLMODE=disable PGCONNECT_TIMEOUT=30
  if [ "$STORYOS_DATABASE_TLS" = require ]; then
    PGSSLMODE=verify-full
    PGSSLROOTCERT=system
    export PGSSLROOTCERT
  fi
  export PGPASSWORD PGHOST PGPORT PGDATABASE PGUSER PGSSLMODE PGCONNECT_TIMEOUT
}

admin_psql() {
  psql -X -q -At -v ON_ERROR_STOP=1 --no-password "$@"
}

# Verifies the archive $1 with its .sha256 file and extracts it to $opt/releases/<commit>. Prints that directory.
install_release() {
  archive=$1
  [ -f "$archive" ] && [ -f "$archive.sha256" ] || fail "$archive and $archive.sha256 must exist"
  [ "$(cut -d ' ' -f 3 "$archive.sha256")" = "${archive##*/}" ] || fail "$archive.sha256 names another file"
  (cd "$(dirname "$archive")" && sha256sum --check --quiet --strict "${archive##*/}.sha256") >&2 \
    || fail "the SHA-256 of $archive is not correct"
  top=$(tar -tzf "$archive" | head -n 1)
  top=${top%%/*}
  case $top in
    storyos-linux-x86_64-?*) ;;
    *) fail "$archive is not an archive of make release-package-linux" ;;
  esac
  release=$opt/releases/${top#storyos-linux-x86_64-}
  if [ ! -d "$release" ]; then
    install -d -m 755 "$opt/releases"
    staging=$(mktemp -d "$opt/releases/.staging.XXXXXX")
    tar -xzf "$archive" -C "$staging" --no-same-owner
    mv "$staging/$top" "$release"
    rmdir "$staging"
  fi
  printf '%s\n' "$release"
}

# Runs storyos-storage of the package directory $1 for at most 10 minutes. The admin URL goes only into the
# environment of that process.
run_storage() {
  password=$(read_secret database-admin-password)
  url=$(database_url "$STORYOS_DATABASE_ADMIN_USER" "$password")
  status=0
  result=$(STORYOS_STORAGE_ADMIN_URL=$url timeout 600 "$1/storyos-storage") || status=$?
  printf '%s\n' "$result"
  # After an upgrade, a second line STORYOS_STORAGE_UPGRADE=... follows the first line.
  [ "$status" = 0 ] || { echo "storyos-storage exited with $status"; return 1; }
  [ "$(printf '%s\n' "$result" | head -n 1)" = STORYOS_STORAGE_ACTIVATION=Active ]
}

wait_for_server() {
  attempt=0
  until curl -fsS --max-time 5 -o /dev/null http://127.0.0.1:3000/api/v1/protocol 2>/dev/null; do
    attempt=$((attempt + 1))
    [ "$attempt" -lt 60 ] || fail "the Server does not answer on 127.0.0.1:3000. Read journalctl -u storyos-server"
    sleep 1
  done
  echo "The Server answers on 127.0.0.1:3000"
}
