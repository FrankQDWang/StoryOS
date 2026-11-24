#!/bin/sh
# Writes the secrets of the personal instance (ADR 0104 sections 2 and 3), each one only to its file of mode 600.
#
# Usage: secrets.sh [<name>]
#
# With no name, the script writes each secret that is absent. With a name, it writes that secret again. It asks
# for the secrets of the author with no echo: database-admin-password, proxy-password, model-key, and
# web-search-key. It makes challenge-secret, session-token, and runtime-password with openssl rand. It keeps the
# proxy password only as the hash of caddy hash-password. It prints no secret. Run install.sh after this script.
here=$(cd "$(dirname "$0")" && pwd)
. "$here/lib.sh"
require_root
load_instance
command -v caddy >/dev/null || fail "Caddy is absent. Run install.sh first"
[ "$#" -le 1 ] || fail "usage: secrets.sh [<name>]"
install -d -m 711 "$etc"
install -d -m 700 "$etc/secrets"
trap 'if [ -t 0 ]; then stty echo; fi' EXIT

ask() {
  printf 'Give the %s (no echo): ' "$1" >&2
  if [ -t 0 ]; then stty -echo; fi
  IFS= read -r secret || secret=
  if [ -t 0 ]; then
    stty echo
    printf '\n' >&2
  fi
  [ -n "$secret" ] || fail "the $1 is empty"
}

write_secret() {
  owner=root
  case $1 in
    proxy-password)
      ask "$1"
      # Caddy reads this file when it loads the Caddyfile.
      owner=caddy
      hash=$(printf '%s\n' "$secret" | caddy hash-password)
      secret=$(printf 'basic_auth {\n\t%s %s\n}' "$STORYOS_PROXY_USER" "$hash")
      ;;
    database-admin-password) ask "$1" ;;
    model-key | web-search-key)
      ask "$1"
      # worker.env has each value in single quotes.
      case $secret in
        *"'"* | *'\'*) fail "the $1 has a quote or a backslash, which worker.env does not permit" ;;
      esac
      ;;
    session-token) secret=$(openssl rand -hex 16) ;;
    challenge-secret | runtime-password) secret=$(openssl rand -hex 32) ;;
    *) fail "unknown secret $1. The names are: $asked_secrets $made_secrets" ;;
  esac
  write_file "$(secret_file "$1")" "$owner" 600 "$secret"
  echo "Wrote the secret $1"
}

if [ "$#" = 1 ]; then
  write_secret "$1"
else
  for name in $asked_secrets $made_secrets; do
    [ -s "$(secret_file "$name")" ] || write_secret "$name"
  done
fi
# The ID of the one User. It is not a secret, and it never changes after the first activation.
if [ ! -s "$etc/user-id" ]; then
  write_file "$etc/user-id" root 600 "$(cat /proc/sys/kernel/random/uuid)"
fi
echo "Run install.sh again to write the settings of the services"
