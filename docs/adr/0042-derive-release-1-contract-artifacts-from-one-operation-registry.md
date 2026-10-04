---
status: accepted
---

# Derive Release 1 Contract Artifacts From One Operation Registry

On 2026-10-04 the author accepted this decision. Each Release 1 artifacts module in `storyos-contracts` declares one `OperationArtifacts` value: its operations with their contract-graph kind and preconditions, and the functions that produce its JSON Schemas, OpenAPI method blocks, TypeScript fragments, and golden-wire fixtures. One registry, `RELEASE1_OPERATIONS`, lists these values. The generator iterates the registry to produce the OpenAPI document, the JSON Schema catalog, the implemented-operation list, the contract graph, the TypeScript client and declarations, and the fixture corpus. The Rust generator stays the only owner of the checked-in artifacts.

## Context

At `main` `6c69595c`, `release1_artifacts.rs` and `release1_fixture_corpus.rs` referred to the per-operation artifacts modules more than 500 times through hand-written lists. A new operation needed one line in six or seven lists. Each list had a different order, so each generated artifact had a different operation order, and no source owned that order.

## Decision

- The registry order is the order of the reviewed route catalog, `docs/foundation/versioned-protocol-release-1-route-catalog.json`. Modules sort by the catalog position of their first operation. Inside a module, operations follow the catalog order. A test enforces this order.
- Every generated artifact follows the registry order. The OpenAPI generator puts the method blocks that share one path under one path key, at the position of the first block. The TypeScript declaration file puts the shared domain types that no single operation owns first, and joins the type fragments with one blank line.
- An operation that the reviewed contract graph does not contain declares this explicitly in its registry entry.
- The change from the earlier orders changed the order of entries in the aggregate artifacts and the derived artifact digests once. It did not change any schema or fixture file, the set of entries in any artifact, `CONTRACT_REVISION`, the generated-client revision, or the Web Client contract revision. [ADR 0015](0015-adopt-typescript-and-vitest-browser-mode-for-the-protected-web-client.md) permits this deterministic digest drift. After that change, a refactor of the generator keeps `generated/` byte-identical.

## Considered options

- Frozen order tables that keep the earlier orders, with new operations at the end, were rejected. They keep accidental history as permanent data for every later reader.
- An order rank for each artifact in each registry entry was rejected. Each new operation would have to choose many ranks.
- A trait with one type for each module was rejected. A plain struct with `fn` values is a constant and needs no new trait.

## Consequences

- A new operation changes its own artifacts module and adds one registry line at its route-catalog position.
- A change of the route catalog order changes the generated order and the derived digests.
