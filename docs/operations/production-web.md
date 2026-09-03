# Operate the Production Web Package

## Build and verify

Use a clean checkout with the repository's pinned Rust, Node, and pnpm toolchain. Run:

```sh
make release-package
make verify
```

The package is `target/release-package/`. It contains `storyos-server`, `storyos-worker`, and `web/`, including the generated `manifest.json`. Transfer that directory as one unit. The default Server process runs the Worker loop. Operators may start `storyos-worker` alone. The manifest records the source commit and tree, client-contract and security-policy identities, and resource metadata and digests. The binary contains the matching manifest digest. Build outputs are not tracked source files.

`make verify` builds one matching package, retains the source and exact-dist checks, and runs the real production Chrome journey through the existing Browser Mode provider. It records the installed Google Chrome Stable version. The journey checks direct production pages and API calls, Trusted Types, blocked embedding, Project creation and opening, manual save, reload recovery, old-writer fencing, and a new writer's save. Its PostgreSQL oracle checks exact receipts and final authority. It does not claim a Takeover UI or later Stage 2 acceptance.

## Prepare the controlled local runtime

Use the existing [PostgreSQL storage and bootstrap contract](../foundation/postgresql-project-storage-isolation-and-migration-contract.md). The database must have the complete accepted bootstrap and a deployment-provided runtime credential. Do not connect the application as the database owner or superuser. This Web host change adds no database migration.

Supply these existing environment values through the trusted local deployment setup, without printing or committing their contents:

| Variable | Existing role |
| --- | --- |
| `STORYOS_DATABASE_URL` | Runtime-role PostgreSQL connection. |
| `STORYOS_BOOTSTRAP_SESSIONS` | JSON object that maps opaque session handles to existing User UUIDs. |
| `STORYOS_CHALLENGE_SECRET` | Secret for the existing command-challenge boundary; at least 32 bytes. |

The trusted bootstrap must also install the matching `storyos_session` browser cookie with HttpOnly and SameSite protection for the exact local origin. The Server does not create a login flow or issue that cookie. Serving a page does not grant Project access. Existing Host, Origin/Referer, session, generation, lifetime, Project Scope, and nonce checks still apply. The current executable binds local sessions for eight hours. Use the printed origin exactly; `localhost` and `127.0.0.1` are not interchangeable bindings.

## Start

Set `package_dir` to the absolute path of the complete package. For a local build:

```sh
package_dir="$PWD/target/release-package"
"$package_dir/storyos-server" --check-web-root "$package_dir/web"
"$package_dir/storyos-server" --web-root "$package_dir/web" --bind 127.0.0.1:3000
```

The offline check exits without opening a listener or accessing PostgreSQL. Normal startup requires `--web-root`; the default bind address is `127.0.0.1:3000`. It validates all Web resources before binding the listener and printing `STORYOS_SERVER_URL`. Missing, extra, changed, mixed, illegal, duplicate, or symbolic-link resources fail startup. Readiness confirms resource and configuration acceptance, not a completed database or author journey.

Open the printed origin in Google Chrome after the trusted bootstrap has supplied the session cookie. Existing Project links use `/projects/<ProjectId>`. Runtime needs no Node, pnpm, or Vite. This contract covers the controlled local HTTP deployment. It does not specify a public network, TLS proxy, cloud, or CDN deployment.

## Upgrade and roll back

1. Build and verify the new complete package. Keep the previously verified package outside the build output directory; a later build replaces `target/release-package`.
2. Let current author work reach its existing settled or explicit recovery state. Stop the old Server process. Do not delete browser Journal data or infer non-commit from a missing response.
3. Install the new package into a separate directory. Run its own binary with `--check-web-root` against its own `web/`. Do not copy individual assets over a running release.
4. Start that binary with its matching `--web-root` and the controlled deployment settings. Follow existing Client Session Binding and pending-command reconciliation rules when the process restarts. Refresh the application to load the new no-store HTML.
5. Confirm Project opening, saved content, and normal author input through the new origin. If the package fails acceptance, stop it and start the complete previously verified package with its matching resources.

Rollback is a whole-package operation. It does not undo database writes or remove local recovery evidence. Use a previous package only if it accepts the current public and persisted contracts; a change that needs a migration requires its own approved plan. Never pair an old binary with new Web resources or change the manifest to bypass startup refusal. A running process serves its validated snapshot, so changes to its resource directory cannot activate an update.

See [the paired-host decision](../adr/0016-deliver-and-verify-the-paired-production-web-host.md) and [Web Editor Session recovery semantics](../foundation/web-editor-session-synchronization-and-recovery-semantics.md) for the governing boundaries.
