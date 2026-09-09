# Operate the Production Web Package

## Build and verify

Use a clean checkout with the repository's pinned Rust, Node, and pnpm toolchain. Run:

```sh
make release-package
make verify
```

The package is `target/release-package/`. It contains `storyos-server`, `storyos-worker`, `storyos-storage`, and `web/`, including the generated `manifest.json`. Transfer that directory as one unit. The default Server process runs the Worker loop. Operators may start `storyos-worker` alone. The manifest records the source commit and tree, client-contract and security-policy identities, and resource metadata and digests. The binary contains the matching manifest digest. Build outputs are not tracked source files.

`make verify` is the local HTTP oracle. It builds one matching package, retains the source and exact-dist checks, and runs the real production Chrome journey through the existing Browser Mode provider on loopback HTTP. It records the installed Google Chrome Stable version. The journey checks direct production pages and API calls, Trusted Types, blocked embedding, Project creation and opening, manual save, reload recovery, old-writer fencing, and a new writer's save. Its PostgreSQL oracle checks exact receipts and final authority. It does not claim a Takeover UI or later Stage 2 acceptance. It does not open a public Host, obtain a certificate, or run the public-origin operator checklist.

## Prepare the runtime

Use the existing [PostgreSQL storage and bootstrap contract](../foundation/postgresql-project-storage-isolation-and-migration-contract.md). Run packaged `storyos-storage` against a distinct admin connection on an empty database until Release 1 Storage Activation is `Active`. Then set the `storyos_runtime` password as an external secret. Do not connect the application as the database owner or superuser. Tracked SQL installs no runtime password.

Supply these environment values through the trusted deployment setup, without printing or committing their contents:

| Variable | Existing role |
| --- | --- |
| `STORYOS_STORAGE_ADMIN_URL` | Distinct admin connection for `storyos-storage`. Do not reuse `STORYOS_DATABASE_URL`. |
| `STORYOS_DATABASE_URL` | Runtime-role PostgreSQL connection after Activation. |
| `STORYOS_BOOTSTRAP_SESSIONS` | JSON object that maps opaque session handles to existing User UUIDs. |
| `STORYOS_CHALLENGE_SECRET` | Secret for the existing command-challenge boundary; at least 32 bytes. |
| `STORYOS_PUBLIC_ORIGIN` | Optional Foundation Validation Public Origin. Leave this unset for the local HTTP profile. Set one `https` Origin for the public HTTPS profile. |

The packaged Server issues the matching `storyos_session` cookie on the printed-origin HTML GET. The cookie uses `HttpOnly`, `SameSite=Strict`, and `Path=/`. It is host-only and omits `Domain`. Serving a page does not grant Project access. There is no login product and no operator cookie injection. Existing Host, Origin/Referer, session, generation, lifetime, Project Scope, and nonce checks still apply. The current executable binds local sessions for eight hours. Use the printed origin exactly; `localhost` and `127.0.0.1` are not interchangeable bindings.

## Server profiles

One packaged `storyos-server` binary has two profiles.

### Local HTTP profile

Leave `STORYOS_PUBLIC_ORIGIN` unset. The Server derives the allowed Host and Origin from the bind address, prints an `http` Server URL, and issues `storyos_session` without `Secure`. This is the Mac development profile and the `make verify` oracle. A DNS name is not required.

### Public HTTPS profile

Set `STORYOS_PUBLIC_ORIGIN` to one absolute `https` WHATWG Origin. That Origin uses one operator-owned DNS name as its Host. A public IP is not a Host. The Origin must have no user information, fragment, path, or query. The Server derives the allowed Host from that Origin and omits default port 443. It prints that Origin as `STORYOS_SERVER_URL` and issues `storyos_session` with `Secure`. A parser error refuses startup before Storage Activation and admits no protected request.

The Server listen address must be loopback or a unix socket. A non-loopback TCP listen, including `0.0.0.0`, refuses startup. Forwarded headers do not define the allowed Host, the allowed Origin, cookie `Secure`, or the printed URL. The TLS reverse proxy is the only public listener. The current packaged start command uses TCP loopback.

CSP, Trusted Types, `frame-ancestors`, and exact-dist proof stay Server-owned. The proxy terminates TLS and presents Host. It does not serve the Protected Web Client. Vercel and any second production Web origin stay rejected.

## Start

Set `package_dir` to the absolute path of the complete package. For a local build:

```sh
package_dir="$PWD/target/release-package"
"$package_dir/storyos-storage"
"$package_dir/storyos-server" --check-web-root "$package_dir/web"
```

`storyos-storage` reads `STORYOS_STORAGE_ADMIN_URL` and does not use `STORYOS_DATABASE_URL`. It prints `STORYOS_STORAGE_ACTIVATION=Active` when the empty database reaches Active or when a later same-identity run succeeds. Set the runtime password after that proof exists and before Server start. The offline Server `--check-web-root` and Worker `--check` exit without opening a listener or accessing PostgreSQL. Normal Server startup requires `--web-root`; the default bind address is `127.0.0.1:3000`. It validates all Web resources and requires a matching Active Release 1 Storage Activation proof on `STORYOS_DATABASE_URL` before it binds the listener or prints `STORYOS_SERVER_URL`. A missing, not-Active, or identity-mismatched proof refuses bind and does not start the in-process Worker. A standalone Worker refuses claim in those cases. Server and Worker do not read `STORYOS_STORAGE_ADMIN_URL` and do not run DDL. Missing, extra, changed, mixed, illegal, duplicate, or symbolic-link resources fail startup.

Local HTTP profile:

```sh
"$package_dir/storyos-server" --web-root "$package_dir/web" --bind 127.0.0.1:3000
```

Public HTTPS profile. Replace `https://example.com` with the configured Foundation Validation Public Origin. Keep the Server on loopback:

```sh
STORYOS_PUBLIC_ORIGIN=https://example.com \
"$package_dir/storyos-server" --web-root "$package_dir/web" --bind 127.0.0.1:3000
```

Open the printed origin in Google Chrome. The packaged Server issues the session cookie on that HTML GET. Existing Project links use `/projects/<ProjectId>`. Runtime needs no Node, pnpm, or Vite.

## Public HTTPS edge

Place an operator-owned TLS reverse proxy, such as Caddy, in front of the Server on the paired Linux VPS. Caddy is operator-owned. It is not in the release package. Do not add Caddy, a certificate, or a DNS API token to the package.

The proxy must:

1. Terminate TLS on port 443.
2. Present the public Host to the Server. Set the Host header to the Host the Server derives from `STORYOS_PUBLIC_ORIGIN`. For `https://example.com` that Host is `example.com`, with port 443 omitted. Do not send the Server listen address as Host.
3. Proxy to the Server listen address.
4. Redirect port 80 to `https`.
5. Send HSTS.

Extra DNS names, including `www`, must redirect at the proxy to the configured Host. The Server admits only that Host. HTTP-01 or DNS-01 certificate issuance is operator practice. StoryOS does not take a DNS API token.

Cloudflare gray-cloud DNS is allowed. It is a DNS provider only. A proxied CDN, including Cloudflare orange cloud, is a rejected second Web host. It terminates TLS in front of the operator proxy and can change served bytes.

Example Caddyfile. Replace `example.com` with the configured Host. Replace `127.0.0.1:3000` when the Server listen address is different:

```
example.com {
	header Strict-Transport-Security "max-age=31536000; includeSubDomains"
	reverse_proxy 127.0.0.1:3000 {
		header_up Host example.com
	}
}

www.example.com {
	redir https://example.com{uri} permanent
}
```

Caddy automatic HTTPS listens on port 80 and port 443 for those names, redirects port 80 to `https`, and obtains the certificate. This file is operator-owned. It is not a package member.

## Public-origin operator checklist

Use this checklist after the public HTTPS profile is running behind the proxy. It is operator evidence against the printed origin. It is not a `make verify` journey and not a second Browser Mode harness. Do not add it to `make verify`. Do not record secrets, connection strings, or DNS API tokens in logs, screenshots, or archives.

1. Open the printed origin from `STORYOS_SERVER_URL` in Google Chrome.
2. Observe that the HTML GET sets `storyos_session` with `Secure`.
3. Complete one author command on that origin, such as create a Project or save in an open Project.

## Upgrade and roll back

1. Build and verify the new complete package. Keep the previously verified package outside the build output directory; a later build replaces `target/release-package`.
2. Let current author work reach its existing settled or explicit recovery state. Stop the old Server process. Do not delete browser Journal data or infer non-commit from a missing response.
3. Install the new package into a separate directory. Run its own binary with `--check-web-root` against its own `web/`. Do not copy individual assets over a running release.
4. Start that binary with its matching `--web-root` and the same Server profile settings. Follow existing Client Session Binding and pending-command reconciliation rules when the process restarts. Refresh the application to load the new no-store HTML.
5. Confirm Project opening, saved content, and normal author input through the new origin. If the package fails acceptance, stop it and start the complete previously verified package with its matching resources.

Rollback is a whole-package operation. It does not undo database writes or remove local recovery evidence. Use a previous package only if it accepts the current public and persisted contracts; a change that needs a migration requires its own approved plan. Never pair an old binary with new Web resources or change the manifest to bypass startup refusal. A running process serves its validated snapshot, so changes to its resource directory cannot activate an update.

See [the paired-host decision](../adr/0016-deliver-and-verify-the-paired-production-web-host.md), [the hosted-infrastructure decision](../adr/0022-prefer-widely-validated-hosted-infrastructure.md), [the public HTTPS transport decision](../adr/0023-own-foundation-validation-public-https-without-pass-cloud.md), [the Server-held Origin decision](../adr/0024-hold-the-allowed-origin-on-the-server.md), and [Web Editor Session recovery semantics](../foundation/web-editor-session-synchronization-and-recovery-semantics.md) for the governing boundaries.
