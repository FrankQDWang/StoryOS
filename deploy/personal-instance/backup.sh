#!/bin/sh
# Makes one dump of the schema storyos in the custom format with the admin connection (ADR 0104 section 7), and
# removes each dump that is older than 14 days. A failed dump leaves the older dumps and exits with an error.
# The timer storyos-backup.timer runs this script each day.
#
# Usage: backup.sh
here=$(cd "$(dirname "$0")" && pwd)
. "$here/lib.sh"
require_root
load_instance
[ "$#" = 0 ] || fail "usage: backup.sh"
use_admin_connection
install -d -m 700 "$backup_dir"
# The nanoseconds keep two dumps of one second apart.
dump=$backup_dir/storyos-$(date -u +%Y%m%dT%H%M%S.%NZ).dump
partial=$(mktemp "$backup_dir/.partial.XXXXXX")
trap 'rm -f "$partial"' EXIT
pg_dump --no-password --format=custom --schema=storyos --file="$partial" || fail "pg_dump failed"
mv "$partial" "$dump"
trap - EXIT
find "$backup_dir" -maxdepth 1 -type f -name 'storyos-*.dump' -mmin +20160 -exec rm -f {} +
echo "Backup: $dump"
