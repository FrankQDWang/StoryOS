#!/bin/sh
# The Web source guards (ADR 0070): text searches of apps/web and the build files. make check-fast runs them.
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

# Accept only ripgrep status 0 (match) and 1 (no match); other statuses stop the script.
search_source() {
  rg_status=0
  rg_output=$(rg "$@") || rg_status=$?
  if [ "$rg_status" -gt 1 ]; then
    printf 'ripgrep failed with exit status %s: rg %s\n' "$rg_status" "$*" >&2
    exit 1
  fi
}

guard_source() {
  guard_message=$1
  shift
  search_source "$@"
  if [ "$rg_status" -eq 0 ]; then
    echo "$guard_message" >&2
    printf '%s\n' "$rg_output" >&2
    exit 1
  fi
}

verify_web_migration_guards() {
  if ! command -v rg >/dev/null 2>&1; then
    echo "ripgrep (rg) is required for the Web source guards" >&2
    exit 1
  fi

  legacy_web_files=$(find apps/web \
    \( -path apps/web/dist -o -path apps/web/node_modules \) -prune -o \
    -type f \( -name '*.js' -o -name '*.jsx' -o -name '*.mjs' -o -name '*.cjs' \) -print)
  if [ -n "$legacy_web_files" ]; then
    echo "Hand-written Web JavaScript remains:" >&2
    printf '%s\n' "$legacy_web_files" >&2
    exit 1
  fi

  guard_source "An active raw browser harness signature remains:" -n \
    'DevTools listening|webSocketDebuggerUrl|Runtime\.evaluate|remote-debugging-(port|pipe)|new WebSocket\(' \
    apps/web --glob '!dist/**' --glob '!node_modules/**'

  guard_source "A CDP primitive escaped the typed Browser Command boundary:" -n \
    'newCDPSession|CDPSession|session\.send\(' \
    apps/web --glob '!dist/**' --glob '!node_modules/**' \
      --glob '!**/test/support/browser-commands.ts'

  search_source -n 'session\.send\(' apps/web/test/support/browser-commands.ts
  if [ "$rg_status" -eq 0 ]; then
    printf '%s\n' "$rg_output" \
      | guard_source "The IME Browser Command uses an unsupported CDP method:" \
        -v 'Input\.imeSetComposition'
  fi

  guard_source "An active legacy browser harness entry remains:" -n \
    'production-page-browser\.integration\.test\.mjs|s1-jrn-001-browser\.integration\.test\.mjs|author-edit-batch-browser-process\.test\.mjs|author-edit-batch-prerelease-browser-harness\.mjs' \
    Makefile package.json apps/web/package.json scripts .github

  guard_source "A browser skip or fallback remains:" -n \
    '\.(skip|skipIf|runIf|todo)\b|\bskip\s*:|Chrome or Chromium is unavailable|CHROME_BIN|chromium-browser|/usr/bin/chromium' \
    apps/web/test/browser-source apps/web/test/browser-exact-dist \
      apps/web/test/support/browser-command-client.ts \
      apps/web/test/support/browser-command-contract.ts \
      apps/web/test/support/production-host-command.ts \
      apps/web/test/support/browser-commands.ts apps/web/vitest.config.ts \
      --glob '*.ts' --glob '*.tsx' \
      --glob '*.js' --glob '*.jsx' --glob '*.mjs' --glob '*.cjs'

  guard_source "A single-User exact-dist journey still injects a test storyos_session cookie:" -n \
    'addCookies\(|updateClientSessionCookie\(\{ action: "set"' \
    apps/web/test/browser-exact-dist \
    apps/web/test/support/production-host-command.ts

  guard_source "A prohibited TypeScript escape remains:" -n \
    '\bany\b|@ts-(ignore|nocheck)|declare module|\bas unknown as\b|\bas [A-Za-z0-9_.$<>\[\] |]+ as\b' \
    apps/web --glob '*.ts' --glob '*.tsx' --glob '!dist/**' --glob '!node_modules/**'
}

verify_web_migration_guards
