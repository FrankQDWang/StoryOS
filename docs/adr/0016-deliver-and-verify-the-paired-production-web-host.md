---
status: accepted
---

# Deliver and Verify the Paired Production Web Host

[Own the Production Protected Web Client Host](https://github.com/FrankQDWang/StoryOS/issues/243) delivers the Server binary and its fixed Web resources as one release package. This implements the asset boundary in ADR 0013. It adds the narrow production verification exception below to ADR 0015. Historical ADRs remain unchanged.

## Release boundary

`make release-package` builds from a clean Git source identity. `storyos-contracts` owns the resource manifest format and generator. The generated manifest records the source commit and tree, client-contract and security-policy identities, and every resource path, byte length, MIME type, and SHA-256. The same build binds the manifest digest into the Server. This build identity is separate from `web_client_contract_revision` and changes no public protocol meaning.

The Server requires an explicit `--web-root`. Before it binds a listener or reports readiness, it rejects missing, extra, changed, mixed, duplicate, illegal, or symbolic-link resources. It then retains immutable bytes. Requests do not read the resource directory again. Normal Rust unit checks do not require a Web build; actual production process tests require the matching package. Runtime needs neither Node nor Vite.

The existing Server owns same-origin pages and resources. Its static fallback cannot replace API routes or their admission rules. HTML uses `no-store`; content-addressed resources use immutable caching. HTTP policy blocks inline scripts, dynamic code evaluation, embedding, and unused Workers. It enforces Trusted Types without a default policy. `Referrer-Policy: same-origin` preserves the existing same-origin Referer admission path and sends no cross-origin referrer.

## Narrow Browser Mode exception

The five Vitest projects, strict TypeScript rules, real Google Chrome Stable channel, serial fixtures, and existing source, exact-dist, and recovery tests remain required. The paired production build runs once before Web verification. No second browser harness or `vite preview` process is added.

In addition to the four Browser Command families in ADR 0015, one private production-host journey command may create and close its own top-level pages through the existing provider context. It belongs to `browser-exact-dist`. Its only accepted request selects the fixed open, edit, reload, and takeover scenario. Its result reports success only after the full scenario assertions pass.

This command may navigate its own pages to the trusted loopback test Server and Project IDs observed from the application. It may reload, observe DOM state, and apply trusted input. It observes the Server-issued `storyos_session` cookie on the printed-origin HTML GET. It does not install a test cookie. Fixed callbacks may observe the existing session key and Journal records through read-only transactions, test browser policy enforcement, and send the existing generated Editor Session queries and Takeover challenge/command through exact same-origin paths. They cannot create or modify Journal records. A missing Journal fails the observation without creating a database.

The command accepts no caller-supplied code, callback, URL, browser method, SQL, file path, network destination, or proxy instruction. It does not operate on the orchestrator page, expose a generic evaluation or CDP capability, or disable CSP. All owned pages close in cleanup. The production application, resources, and API come directly from the Server; the existing exact-dist middleware continues to own its separate evidence.

Takeover uses the existing generated API, not a new product UI or durable client command workflow. Acceptance proves old-page fencing with retained local text, the winner's reload into a distinct generation-bound Journal partition, preservation of old records, and a successful manual save by the winner. A separate exact PostgreSQL oracle checks this Project while preserving the original Stage 1 authority assertions.

## Considered options and consequences

- Embedding the production page was rejected because HTTP `frame-ancestors 'none'` is a required protection. A meta policy cannot supply that protection. See [CSP frame-ancestors](https://www.w3.org/TR/CSP3/#directive-frame-ancestors).
- A separate Web server or CDN was rejected for this slice because paired same-origin delivery satisfies the current boundary without another runtime or release authority.
- A broad browser automation command was rejected because it would hide a second test platform behind Browser Mode.
- [Production operation](../operations/production-web.md) must upgrade or roll back the complete verified package. This decision adds no login flow, database migration, deployment service, or author write path.
