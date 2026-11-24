#!/bin/sh
# A systemctl for the container check of the personal instance (scripts/verify-personal-instance.sh). A container
# has no systemd, so this script starts the ExecStart line of a unit itself: as root it exports the Environment
# lines and the EnvironmentFile of the unit, and then it runs the command as the User of the unit. The check
# verifies the units themselves with systemd-analyze verify.
set -eu
state=/run/fake-systemd
mkdir -p "$state"

unit_file() {
  for directory in /etc/systemd/system /usr/lib/systemd/system /lib/systemd/system; do
    if [ -f "$directory/$1.service" ]; then
      printf '%s\n' "$directory/$1.service"
      return
    fi
  done
  echo "fake systemctl: no unit $1" >&2
  exit 5
}
value() { sed -n "s/^$2=//p" "$1"; }
running() { [ -f "$state/$1.pid" ] && kill -0 "$(cat "$state/$1.pid")" 2>/dev/null; }

# Writes the script $state/$1.$2.sh, which runs the line $2 of the unit $1 as the User of the unit, in the
# environment of the unit. Each step of the script is an exec, so the script keeps its process ID.
unit_script() {
  file=$(unit_file "$1")
  user=$(value "$file" User)
  user=${user:-root}
  {
    value "$file" Environment | sed 's/^/export /'
    value "$file" EnvironmentFile | sed 's/^-//; s/.*/set -a; . & ; set +a/'
    printf 'export HOME=%s USER=%s\n' "$(getent passwd "$user" | cut -d : -f 6)" "$user"
    printf 'exec setpriv --reuid=%s --regid=%s --init-groups %s\n' "$user" "$user" "$(value "$file" "$2")"
  } > "$state/$1.$2.sh"
}

start() {
  running "$1" && return
  unit_script "$1" ExecStart
  sh "$state/$1.ExecStart.sh" < /dev/null >> "$state/$1.log" 2>&1 &
  printf '%s\n' "$!" > "$state/$1.pid"
}

stop() {
  if running "$1"; then
    kill "$(cat "$state/$1.pid")"
    while running "$1"; do sleep 0.1; done
  fi
  rm -f "$state/$1.pid"
}

command=$1
shift
[ "${1:-}" != --quiet ] || shift
[ "${1:-}" != --now ] || { now=1; shift; }
for unit in "$@"; do
  name=${unit%.service}
  case $command:$name in
    daemon-reload:* | *:*.timer) ;;
    enable:*) [ -z "${now:-}" ] || start "$name" ;;
    start:*) start "$name" ;;
    stop:*) stop "$name" ;;
    restart:*) stop "$name"; start "$name" ;;
    try-restart:*) if running "$name"; then stop "$name"; start "$name"; fi ;;
    reload-or-restart:*)
      if running "$name"; then
        unit_script "$name" ExecReload
        sh "$state/$name.ExecReload.sh"
      else
        start "$name"
      fi
      ;;
    is-active:*) running "$name" || exit 3 ;;
    *) echo "fake systemctl: $command is not supported" >&2; exit 1 ;;
  esac
done
