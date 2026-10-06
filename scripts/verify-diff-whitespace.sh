#!/bin/sh
# Check whitespace from the merge base with origin/main to the worktree, untracked files included.
set -eu
base=${STORYOS_PR_BASE_SHA:-$(git merge-base origin/main HEAD)}
index=$(mktemp)
trap 'rm -f "$index"' EXIT
cp "$(git rev-parse --git-path index)" "$index"
GIT_INDEX_FILE=$index git add --intent-to-add --all
GIT_INDEX_FILE=$index git diff --check "$base" --
