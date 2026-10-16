#!/bin/sh
# Run git fetch, git pull --ff-only, or git push, and retry a transient network failure for a
# bounded time. A rejected push or another permanent failure stops at once with the git error.
# Usage: scripts/git-retry.sh fetch|pull --ff-only|push [git arguments]
set -eu
case "${1:-}" in
	fetch | push) ;;
	pull)
		case " $* " in
			*" --ff-only "*) ;;
			*) printf '%s\n' 'git-retry.sh: use pull only with --ff-only' >&2; exit 2 ;;
		esac
		;;
	*) printf '%s\n' 'git-retry.sh: use fetch, pull --ff-only, or push' >&2; exit 2 ;;
esac
exec python3 "$(dirname "$0")/verification_github.py" git "$@"
