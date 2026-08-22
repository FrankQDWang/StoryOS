---
status: accepted
---

# Adopt TypeScript and Vitest Browser Mode for the Protected Web Client

StoryOS will migrate the complete hand-written Protected Web Client in `apps/web` to TypeScript and will replace its hand-written Chrome harnesses with Vitest Browser Mode. This decision supersedes the current-language and current-test-runner choices in ADR 0014. ADR 0014 remains unchanged as the historical record of the first pnpm, Vite, Node, and React toolchain slice.

The migration is a behavior-preserving platform change. It must not change the Protected Web Client runtime contract, author journey, protocol, persistence meaning, or database meaning. It must finish before another product stage starts.

## Migration boundary

All hand-written files below `apps/web` that end in `.js`, `.jsx`, `.mjs`, or `.cjs` must be zero at final acceptance. This includes production source, tests, support code, and configuration. `apps/web/dist`, `apps/web/node_modules`, repository-external research or prototypes, and Rust-generated runtime `.mjs` artifacts are not hand-written Web Client files and are outside this count.

A research artifact can remain in `docs/research` as historical evidence. It must not remain an active `make`, CI, or test entry if it starts Chrome or implements a second browser harness. Its covered behavior must move to the applicable Vitest Browser Mode project before the old entry is removed from active verification.

The migration does not promote `prototypes/**` or `.reference/**`. It does not start Stage 2.

## TypeScript contract

The production typecheck uses exactly [TypeScript 7.0.2](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/) through the `tsc` command. TypeScript 7.0 has no stable programmatic compiler API, so StoryOS must not make its production typecheck depend on that API. Layer 1 must prove that each selected tool works with TypeScript 7.0.2. A tool-only TypeScript 6 compatibility package is permitted only when a selected development tool requires it. It must not replace or weaken the production TypeScript 7.0.2 typecheck.

The Web Client TypeScript configuration must set:

- `strict: true`;
- `noUncheckedIndexedAccess: true`;
- `exactOptionalPropertyTypes: true`;
- `skipLibCheck: false`; and
- `allowJs: false`.

Hand-written Web Client code must not use explicit `any`, a hand-written ambient module shim, `@ts-ignore`, `@ts-nocheck`, or a double assertion such as `as unknown as T`. Generator-owned `.d.mts` files and normal package declaration files are declarations, not ambient escape shims. Node types must not leak into production browser modules.

Strict typecheck must run before the production build and before all Web tests. A build or test result cannot compensate for a failed typecheck.

## Runtime and persistence invariants

The migration must preserve all existing runtime named exports and all observable HTTP and DTO meaning. It must preserve `web_client_contract_revision` as `storyos.web-client.release-1.v3`. If the migration requires a wire, DTO, route, Problem, or generated-client semantic change, implementation must stop and reopen [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58).

External and persisted values enter Web Client code as `unknown`. The existing validation seams must narrow those values before use. These seams include protocol-profile, Project, Chapter, Editor Session, Local Edit Journal snapshot and payload-chain, Activity Event and Snapshot, outcome, and settlement validation. A generated declaration that describes the expected DTO does not replace runtime validation.

IndexedDB remains version 3. Its stores remain `metadata`, `partitions`, `payload_chains`, `intents`, `submission_groups`, `transport_capsules`, `transport_attempts`, `protocol_observations`, `outcome_query_attempts`, and `outcome_query_observations`. Database names, session keys, strict durability, upgrade rules, and all recovery meaning remain unchanged. PostgreSQL schemas, queries, and transaction meaning remain unchanged.

## Generated Release 1 artifacts

The Rust generator remains the only owner of the checked-in Release 1 runtime and declaration artifacts. It must generate these pairs:

- `client.mjs` and `client.d.mts`; and
- `release-profile.mjs` and `release-profile.d.mts`.

The new release-profile declaration must have deterministic drift coverage. It may contribute to the TypeScript artifact digest. This can change the derived TypeScript artifact digest and fixture-corpus digest. Such deterministic identity drift is permitted. It must not change generated runtime exports, HTTP or DTO meaning, the generated-client revision, or `web_client_contract_revision`. The runtime release profile must not include itself in its own digest.

## Test platform and project split

StoryOS pins these stable versions from the [Vitest Browser Mode](https://vitest.dev/guide/browser/) and Playwright toolchain:

- Vitest 4.1.11;
- `@vitest/browser-playwright` 4.1.11; and
- Playwright 1.62.1.

Vitest 5 is a prerelease at this decision point and is not selected.

All Web tests must run in exactly five Vitest projects:

1. `node-contract` owns production-build, protocol boot, Project-open, and pure protocol HTTP checks that do not need PostgreSQL or a browser.
2. `node-postgresql` owns PostgreSQL-backed HTTP, Activity, Snapshot, Takeover, and project-scope checks.
3. `node-process-cut` owns process-termination and acknowledgement-cut checks.
4. `browser-source` owns browser behavior against source modules, including acknowledgement loss, Activity reorder and resync, outcome reconciliation, Editor Session, Journal collection, manual input, reload recovery, and stale-writer behavior.
5. `browser-exact-dist` owns the production-page and complete Stage 1 exact-dist journeys.

Pure HTTP, PostgreSQL, and process-cut tests remain Node projects. Browser-observable behavior must not stay in a Node test with a hand-written browser process. Shared PostgreSQL fixtures, process-cut fixtures, and exact-dist fixtures must stay in deterministic serial orchestration even when Vitest project configuration also disables file parallelism.

The production build runs once. No test may rebuild or clear `dist` while another project serves or reads it.

## Google Chrome Stable gate

Each Browser project uses the [Vitest Playwright provider](https://vitest.dev/config/browser/playwright.html) with a Chromium browser type and the [branded Google Chrome channel](https://playwright.dev/docs/browsers#google-chrome--microsoft-edge):

- `instances: [{ browser: "chromium" }]`;
- `launchOptions: { channel: "chrome" }`; and
- `persistentContext: false`.

The Browser project also sets top-level `test.isolate: true`, `fileParallelism: false`, and `retry: 0`. The browser configuration sets `headless: true` for automated verification. The deprecated `browser.isolate` option must not be used.

Isolation is per test file. Tests in one file share a browser page. A persistent-state scenario must therefore use one scenario per file and must use deterministic database, cookie, session-storage, and IndexedDB setup and cleanup.

The machine must have Google Chrome Stable. Missing Chrome, a failed `channel: "chrome"` launch, or evidence of a different browser must fail the project. There is no Chromium fallback and no missing-browser skip. Verification must record the actual Google Chrome version because a Playwright package pin does not pin the installed system Chrome binary.

## Browser Mode and exact-dist transport

Normal browser tests use Browser Mode assertions and its application iframe. They must not use a private HTTP result collector or evaluate test code through a browser debugging protocol.

The exact-dist tests use one small read-only Vite [`configureServer` middleware](https://vite.dev/guide/api-plugin.html#configureserver) in the Vitest Vite server. The middleware sends the already-built `dist` bytes without an HTML transform. It serves the production index for the accepted application routes and serves the root `/assets/...` paths used by the current `base: "/"` build. The child application iframe and test orchestrator therefore have the same scheme, host, and port.

A reload reloads only the child application iframe. It must destroy that application realm without reloading the Vitest orchestrator. The browser context stays alive so the scenario can prove cookie and IndexedDB recovery.

The exact-dist transport must preserve the current Host and Origin rewrite when it proxies API calls to `storyos-server`. The Vite middleware proves exact production-bundle behavior. It does not replace the real `storyos-server` path for CSP, security headers, Host and Origin admission, Client Session Binding, Application, Core, or PostgreSQL evidence. The complete Stage 1 journey must still use that real path.

A separate `vite preview` process is not part of the test transport because it creates another origin and another process lifecycle.

## Privileged browser operations

Delete every hand-written Chrome process launcher, DevTools URL parser, debugging WebSocket client, `Runtime.evaluate` call, arbitrary CDP harness, and missing-browser skip from the active Web test platform.

StoryOS may expose only four private, typed Browser Command families:

- IME composition;
- trusted keyboard or text input;
- clipboard permission; and
- the test Client Session cookie.

Each command has a fixed StoryOS-owned name, a closed typed request, and a closed typed result. A command may use the provider context and the minimum fixed browser primitive needed for its one operation. It must not accept an arbitrary browser method, JavaScript source, URL, navigation target, file path, shell command, network destination, database query, or proxy instruction. It must not operate on the Vitest orchestrator page. Navigation, reload, and DOM observation belong to Browser Mode and the child application iframe.

Hand-written tests use typed wrapper functions for these fixed commands. They must not add an ambient Vitest command shim as a type escape.

## Four-layer implementation stack

The implementation remains one Issue and uses four dependency-ordered pull requests:

1. **Type, generator, and test-transport foundation:** add the TypeScript 7.0.2 strict configuration, generator-owned release-profile declaration, five-project Vitest structure, private typed command and exact-dist transport foundations, and compatibility proof for every compiler-API-dependent tool. Do not migrate production behavior in this layer.
2. **Production TypeScript and TSX:** migrate all production source and the Vite configuration while preserving every runtime and persistence invariant.
3. **Node and normal Browser Mode tests:** migrate `node-contract`, `node-postgresql`, `node-process-cut`, and `browser-source`. Remove source-browser raw harnesses and inactive research-harness execution.
4. **Exact-dist, Stage 1, Make, CI, and final guards:** migrate `browser-exact-dist`, preserve the real Server/Application/Core/PostgreSQL path, update Stage 1 provenance, order Make and CI gates, and enforce zero hand-written JavaScript, zero raw harness signatures, and zero browser-test skips.

Every layer must keep `make verify` green. Every layer needs independent Standards and Spec review, an exact synthetic-merge-tree local verification record, fresh required checks, zero unresolved review threads, and an ordinary bottom-up merge. A later layer cannot hide a lower-layer contract or migration defect.

## Final acceptance

The migration is complete only when:

- strict TypeScript 7.0.2 typecheck runs before build and tests and passes with all locked options and bans;
- all five named Vitest projects pass;
- verification records the actual Google Chrome Stable version;
- the exact-dist Stage 1 journey passes through the real Server, Application, Core, and PostgreSQL path;
- hand-written `apps/web/**/*.{js,jsx,mjs,cjs}` is zero outside excluded generated or output trees;
- active hand-written Chrome, DevTools URL, debugging WebSocket, `Runtime.evaluate`, and generic CDP harness signatures are zero;
- browser-test skip declarations and missing-browser skip branches are zero;
- the Rust-generated runtime and declaration pairs pass deterministic drift checks;
- runtime exports, HTTP and DTO meaning, IndexedDB version and schema, database meaning, and `web_client_contract_revision` remain unchanged; and
- complete verification passes on the final composed `main` tree.

## Considered options

- A partial migration with `allowJs` was rejected because it would make mixed-language debt a permanent boundary.
- TypeScript 6 or a silent TypeScript 7 fallback was rejected because the selected production type contract is TypeScript 7.0.2.
- Vitest 5 was rejected because it is a prerelease at this decision point.
- Playwright's bundled Chromium or a Chrome-missing skip was rejected because acceptance requires the real Google Chrome Stable channel.
- Keeping Node browser harnesses beside Browser Mode was rejected because it would preserve two privileged browser platforms.
- A broad CDP, file, shell, navigation, or network Browser Command was rejected because it would move the old unsafe harness behind a new name.
- A separate `vite preview` server was rejected because it does not give the orchestrator and exact-dist application one origin.
- Changing protocol, DTO, IndexedDB, PostgreSQL, or runtime meaning during the migration was rejected because those changes have different owners and acceptance evidence.

## Consequences

The repository takes one four-layer migration before the next product stage. The TypeScript declaration set and its derived digests can change, but public runtime meaning cannot. Verification now depends on an installed Google Chrome Stable and must record its live version. TypeScript-compiler-API tools need an explicit compatibility proof. Browser tests gain one typed and narrow privileged boundary, while ordinary behavior moves to Browser Mode and same-origin child iframes.
