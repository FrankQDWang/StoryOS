#!/bin/sh
# Check whitespace from the merge base with origin/main to the worktree, untracked files included.
# In the pull-request check, the base is the first parent of the synthetic merge, as in verify-pr.
set -eu
if [ -n "${STORYOS_PR_BASE_SHA:-}" ]; then
	base=$(git rev-parse HEAD^1)
else
	base=$(git merge-base origin/main HEAD)
fi
index=$(mktemp)
trap 'rm -f "$index"' EXIT
cp "$(git rev-parse --git-path index)" "$index"
GIT_INDEX_FILE=$index git add --intent-to-add --all
GIT_INDEX_FILE=$index git diff --check "$base" --
