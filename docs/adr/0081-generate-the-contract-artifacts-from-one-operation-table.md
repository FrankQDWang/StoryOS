---
status: accepted
---

# Generate the Contract Artifacts From One Operation Table

The repository cleanup specification of phase 8 ([#1385](https://github.com/FrankQDWang/StoryOS/issues/1385)) owns this ADR. The author decided it on 2026-11-02. Ticket [#1386](https://github.com/FrankQDWang/StoryOS/issues/1386) adds the table and the new writer beside the old modules. Ticket [#1387](https://github.com/FrankQDWang/StoryOS/issues/1387) makes the table the only source and deletes the old modules. Ticket [#1388](https://github.com/FrankQDWang/StoryOS/issues/1388) removes the schema files that no reader opens. This ADR replaces the paragraphs of [ADR 0042](0042-derive-release-1-contract-artifacts-from-one-operation-registry.md) that it names below. The rule paragraphs of ADR 0042 and [ADR 0074](0074-generate-the-route-catalog-from-the-operation-registry.md) stay in force.

## Context

At `main` `a470a07f`, the Operation Registry listed 39 `OperationArtifacts` values for 47 public operations. Each value was one hand-written module of 81 to 674 lines, 8,615 lines in all. Each module declared the same five parts: the registered operations, a schema function, an OpenAPI function, a TypeScript declaration list, and a fixture. The 21 synchronous command modules were near copies of one template. Each wrote the same OpenAPI YAML by hand: the summary, the three header parameters, the request body reference, the response list, and the Retry-After header of the 429 response. The route, the method, the status codes, the fixture id, and the Route Catalog metadata that the YAML repeated already existed in the registry entry. Each schema function applied the same hand patches after schemars.

The generator wrote 94 JSON Schema files. The OpenAPI document referenced 78. The other 16, the request schemas of the GET and stream operations, had no reader in the Server, the Web Client, the tests, or the scripts. The `check` command compared only the files that the generator emitted, so a file that stayed under `generated/` after its writer went was not drift.

## Decision

### 1. One table of the public operations

- The Operation Registry is one table with one row for each public operation, in the declared order. The declared order is the order of every generated artifact (ADR 0074).
- A row holds the registered operation: the route, the kind, the schema ids, the status codes, the fixture id, the contract-graph preconditions, the Route Catalog metadata, and the client override. A row holds five hand fields that the registered operation does not hold: the summary text, the header parameter set, the schema sources, the TypeScript declaration list, and the fixture.
- The header parameter set is a closed value over the Origin, Idempotency-Key, and X-StoryOS-Anti-Forgery parameters.
- A schema source names the request type and the response type and carries a declarative patch list. A patch names a JSON pointer inside the schema and one edit of a closed set: a constant, a format, a pattern, a minimum length, or a maximum UTF-8 byte count. One function applies the list after schemars. No row repeats a patch that the shared schema helpers apply to every schema.
- A TypeScript declaration that several operations share appears in the row of the first operation that uses it.
- The fixture of a row is the one positive golden-wire fixture of the operation (ADR 0075), kept as a JSON literal in the row.
- A row holds no code. A fact that the registered operation already holds is not written in a hand field.

### 2. One writer

- One writer reads the table and emits every generated contract artifact: the OpenAPI document, the schema files, the TypeScript client and declarations, the Route Catalog, and the golden-wire fixtures.
- The writer synthesizes the OpenAPI path item of an operation from the method, the path, the summary, the header parameter set, the request schema reference, the response list, and the Retry-After rule of the 429 response. The query, command, challenge, and stream shapes are four forms of one function. The registry kind picks the form. A row does not name its form.
- The writer emits no request schema file for a GET or stream operation, because no reader opens it. A registry entry of such an operation keeps a request schema id only when a reader uses it.
- The `check` command lists the files under `generated/` and reports a file that the generator does not emit as stale, with the same exit as a changed file. The persistence catalog directory, which the PostgreSQL adapter writes, is excluded by name.
- The crate root re-exports only the names that a consumer outside the crate reads.

### 3. Byte identity and the one exception

- The rewrite from the 39 modules to the table keeps every generated file byte-identical, as the byte-identical refactor rule of ADR 0042 requires. Ticket #1386 proves it with one test that compares every file of the old writer with the same file of the new writer. Ticket #1387 proves it with the `check` command against the committed files. The pinned contract-graph digest does not change.
- Ticket #1388 removes the 16 request schema files of the GET and stream operations from the emitted set. The schema catalog digest of the release identity changes once, and only its two carriers change with it: the `json_schema_catalog_digest` value of the protocol-profile golden-wire fixture and the same value in the release-profile TypeScript file. No other generated file changes. This is the one exception of phase 8 to the byte-identical refactor rule, as the deterministic digest drift that [ADR 0015](0015-adopt-typescript-and-vitest-browser-mode-for-the-protected-web-client.md) permits. After that change, a refactor of the generator again keeps `generated/` byte-identical.

### 4. Delivery

- The work lands on one integration branch, `cleanup-phase-8`, through three tickets in one chain: expand (#1386), contract (#1387), schema narrowing (#1388). Each ticket pull request passes `make check`. Each head of the aggregate pull request passes `make check-full` and gets the Codex review.
- The 34 per-operation type modules, the Route Catalog writer, the TypeScript client writer, and the persistence catalog generator do not change.
- The three Python readers of the schema directory under `prototypes/reference-model/` and the one citation under `docs/research/` are research evidence and do not change.

## Superseded parts

- ADR 0042, the first paragraph: "Each Release 1 artifacts module in `storyos-contracts` declares one `OperationArtifacts` value: its operations with their contract-graph kind and preconditions, and the functions that produce its JSON Schemas, OpenAPI method blocks, TypeScript fragments, and golden-wire fixtures." The table row replaces the module, and the hand fields replace the functions.
- ADR 0042, Considered options, the third point: "A trait with one type for each module was rejected. A plain struct with `fn` values is a constant and needs no new trait." The row holds data, not `fn` values.
- ADR 0042, Consequences, the first point: "A new operation changes its own artifacts module and adds one registry line at its position in the declared registry order (ADR 0074)." A new operation adds one row at its position.

## Considered options

- An MCP tool manifest generated from the table was rejected for this phase. No ADR records StoryOS as an MCP server for an external host, and the repository plans no such surface. A generated artifact without a reader is the fault that the cleanup removes. A new artifact kind is one writer function over the table when a Stage and an ADR add that surface.
- Schema patches as `schemars` attributes on the Rust types were rejected for this phase. The attribute output of schemars is not proved byte-identical to the patched output of today, and the rewrite must be.
- Fixtures as authored JSON files outside the crate were rejected. About one hundred Web Client test files read the fixtures from `generated/`, and a move gives no reader a benefit.
- Schema files inlined into the OpenAPI components were rejected. It changes the OpenAPI bytes, which the rewrite must keep.

## Consequences

- A new public operation adds one row with five hand fields and no code. The registry kind picks its OpenAPI form.
- A dead generated file is drift. The `check` command reports it.
- The schema directory holds only files with a reader: the 78 that the OpenAPI document references.
- The schema catalog digest changes once, in ticket #1388.
