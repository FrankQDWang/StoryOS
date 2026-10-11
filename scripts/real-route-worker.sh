#!/bin/sh
# Run the packaged storyos-worker for the local real model route with the given arguments. The
# script loads the STORYOS_ variables of the git-ignored .env of the main checkout into the
# Worker process only (ADR 0050). It never prints a value.
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
worker_bin="$repository_root/target/release-package/storyos-worker"
common_dir=$(git -C "$repository_root" rev-parse --path-format=absolute --git-common-dir)
env_file="$(dirname -- "$common_dir")/.env"

if [ ! -x "$worker_bin" ]; then
  echo "The release package has no storyos-worker. Run make release-package first." >&2
  exit 1
fi
if [ ! -f "$env_file" ]; then
  echo "The main checkout has no .env file. Copy .env.example and add the key." >&2
  exit 1
fi
if [ "$(stat -f %Lp "$env_file")" != "600" ]; then
  echo "The .env file must have mode 600." >&2
  exit 1
fi
while IFS= read -r line || [ -n "$line" ]; do
  case "$line" in
    STORYOS_*=*) ;;
    *) continue ;;
  esac
  name=${line%%=*}
  case "$name" in
    *[!A-Z0-9_]*) continue ;;
  esac
  value=${line#*=}
  value=${value#\"}
  value=${value%\"}
  export "$name=$value"
done < "$env_file"
exec "$worker_bin" "$@"
