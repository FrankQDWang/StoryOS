#!/bin/sh
# Upgrades the personal instance to a new release package (ADR 0104 section 4). It installs the archive, stops
# the Worker and the Server, makes a dump with backup.sh, runs storyos-storage of the new package, changes the link
# /opt/storyos/current, and starts the two services. When a step before the change of the link fails, the script
# starts the old package again and exits with an error.
#
# Usage: upgrade.sh <archive>
here=$(cd "$(dirname "$0")" && pwd)
. "$here/lib.sh"
require_root
load_instance
[ "$#" = 1 ] || fail "usage: upgrade.sh <archive>"
release=$(install_release "$1")
old=$(readlink -f "$opt/current")
"$release/storyos-server" --check-web-root "$release/web" >/dev/null || fail "the Web root of $release is not correct"

keep_old() {
  systemctl start storyos-server.service storyos-worker.service
  wait_for_server
  fail "$1. The package $old is in use"
}
systemctl stop storyos-worker.service storyos-server.service
"$here/backup.sh" || keep_old "the backup failed"
run_storage "$release" || keep_old "storyos-storage of $release refused the database"
ln -sfn "$release" "$opt/current.new"
mv -T "$opt/current.new" "$opt/current"
systemctl start storyos-server.service storyos-worker.service
wait_for_server
echo "The package $release is in use. The earlier package stays in $old"
