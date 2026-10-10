#!/bin/sh
# Sign a copy of the packaged storyos-worker with the local development identity, then run it
# with the given arguments. The release package stays unchanged. A fixed identity and identifier keep the Keychain access of the Worker valid
# after each rebuild (docs/operations/local-real-model-route.md).
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
worker_bin="$repository_root/target/release-package/storyos-worker"
signed_dir="$repository_root/target/local-real-route"
signed_bin="$signed_dir/storyos-worker"
identity="StoryOS Local Development"
identifier="dev.storyos.worker"

if [ ! -x "$worker_bin" ]; then
  echo "The release package has no storyos-worker. Run make release-package first." >&2
  exit 1
fi
if ! security find-identity -v -p codesigning | grep -q "\"$identity\""; then
  echo "The code signing identity \"$identity\" is missing." >&2
  echo "Create it one time as docs/operations/local-real-model-route.md tells." >&2
  exit 1
fi
mkdir -p "$signed_dir"
cp "$worker_bin" "$signed_bin"
codesign --force --sign "$identity" --identifier "$identifier" "$signed_bin"
codesign --verify --strict "$signed_bin"
exec "$signed_bin" "$@"
