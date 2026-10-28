---
status: accepted
---

# Generate the Route Catalog From the Operation Registry

The repository cleanup specification of phase 2 ([#1298](https://github.com/FrankQDWang/StoryOS/issues/1298)) owns this ADR. The author approved its ticket graph on 2026-10-28. Ticket [#1299](https://github.com/FrankQDWang/StoryOS/issues/1299) adds the generated Route Catalog beside the hand-kept catalog. Ticket [#1300](https://github.com/FrankQDWang/StoryOS/issues/1300) moves the readers and removes the hand-kept catalog and its Python verifier. This ADR partly supersedes [ADR 0042](0042-derive-release-1-contract-artifacts-from-one-operation-registry.md).

## Context

The maintainer kept the Release 1 Route Catalog by hand as a JSON file. It had 6,314 lines. The Operation Registry of ADR 0042 already held the route, the kind, the schemas, the status codes, the fixtures, and the preconditions of each served operation. Thus a route change edited the same facts in two places.

The hand-kept catalog also stated facts that were not true. It listed 63 operations, but the Server serves 48. It listed 54 Events, but the code can persist and replay only the 22 Event schemas of `ProjectActivityKind`. For some served operations, its status codes and preconditions differed from the registry. Tests recorded these differences as known drift.

ADR 0042 took the registry order from the position of each operation in the hand-kept catalog.

## Decision

- **Source.** The Operation Registry is the only source of the Route Catalog. Each `RegisteredOperation` holds the catalog metadata of its operation: shape, semantic owner, admission and settlement rules, error profile, Project Activity mapping, notes, action coverage, cause profiles, and query proof. Rust enums hold the closed value sets. The generated JSON keeps the field names and the value strings of the hand-kept catalog.
- **Output.** `make generate-contracts` writes the catalog to `generated/route-catalog/versioned-protocol-release-1-route-catalog.json`. The drift check of `make check-fast` compares it with the registry, as it compares the other generated artifacts.
- **Facts from the registry.** The catalog takes the route, the kind, the schema IDs, the status codes, the fixtures, and the preconditions from the registry. When the hand-kept catalog differed from the registry, the registry value is the served contract.
- **Served operations only.** The catalog lists only the operations in the registry. Section 7.1.2 of the protocol document keeps the planned operations of later Stages. A Stage that serves one of them adds it to the registry, and the catalog gets it from there.
- **Events with a producer only.** The catalog Events are the Event schemas of each `ProjectActivityKind`, the kinds that the code can persist and replay. The Project Activity mapping of an operation names `ProjectActivityKind` values, so it cannot name an Event without a producer.
- **Catalog sections.** The owners, the error profiles, the error source, the release identity, the capability coverage, the non-public operations and Events, and the coverage contract are Rust constants.
- **Registry order.** The registry order is the declared order of `RELEASE1_OPERATIONS` and of the operations in each entry. It no longer comes from the hand-kept catalog. The current order stays, so no existing generated artifact changes its order. The generated catalog follows the registry order. This replaces the first point of the ADR 0042 decision. The test that compared the registry order with the hand-kept catalog went away with that catalog.
- **Cause profiles in the types.** `OperationCatalog::conditional_by_cause` is the only way to make a command conditional by cause. It takes one author cause and one current-producer cause. Each cause type holds its own binding and its own recovery rule, so an entry cannot mix them.
- **Rules that the types do not hold.** Rust tests check these rules on the generated catalog. They take the place of the rules of the old Python verifier.
  - A unique operation ID, and a unique method and path.
  - A unique schema ID over the operations, the cause profiles, the Events, and the release identity, and a unique fixture ID over the operations and the Events.
  - An Accepted command that names its own registered GET settlement query.
  - Cause profiles only on a conditional command, a synchronous settlement for each editor command, and Project Activity only on a command.
  - Capability coverage of the required set with served operations only.
  - Error profiles that name only the stable errors of the protocol document. The served status codes of each operation include the status of each code of its error profile. Thus `getProject` uses the `query_current` profile. `getChapter` serves `409 snapshot_expired`, so it uses the `query_current_chapter` profile, which adds `snapshot_expired` (ticket #1301).
  - An owner source that resolves to a heading of a tracked document.
  - Known owners, complete schema sources, and non-empty preconditions for each operation.
  - An Activity mapping for each Event, and an Event for each Activity reference.
  - The exact author and current-producer cause rules of `withdrawProposal` and `replanProposal`.
  - The exact operations of the export capabilities, and the rename and reorder actions of the Volume and Chapter updates.
  - The one Author Edit ingress with its independent outcome query and its query proof.
- **Only one catalog.** Ticket #1300 removed the hand-kept catalog, its Python verifier and the `make check-fast` step of that verifier, and the pinned catalog digest of the generator. The contract graph no longer holds a catalog digest, so the generator pins only the contract graph digest. Each reader of the catalog reads the generated file: the persistence catalog verifier, the Author Edit batch policy verifier, and the reference-model prototype. The Stage 1 crosswalk reads the catalog of its baseline commit from Git history, so it keeps the historical path.
- **Storage identity digest.** The persistence catalog holds no copy of the route catalog digest. The build of `storyos-adapter-postgres` embeds the generated catalog, and storage activation computes the `route_catalog_sha256` field of the activation identity from it. The identity fields and the `identity_mismatch` refusal do not change.
- **Persistence route coverage.** The persistence catalog maps only served operations to its table families. A family with no served operation is `internal_only` until a Stage serves an operation of that family.

## Considered options

- Keep the hand-kept catalog as a reviewed input and check it against the registry. Rejected: each route change still edits two files, and the catalog keeps the planned operations as if they were served.
- Keep the planned operations in the generated catalog with a planned marker. Rejected: the protocol document already holds the plan, and a client must not expect an operation that the Server does not serve.
- Keep the hand-kept values where they differed from the registry. Rejected: the registry produces the served OpenAPI document and the client, so it is the served contract.
- Let the generator write the route catalog digest into the persistence catalog. Rejected: the persistence catalog is a hand-kept file, so the digest would stay a copy that a person or a tool must keep current. The storage build reads the generated catalog that it embeds, so no copy exists.

## Consequences

- A new operation adds its catalog metadata to its own registry entry. The catalog gets the operation in the same change.
- A new Project Activity kind becomes a catalog Event when it joins `ProjectActivityKind`. The compiler requires its semantic owner.
- The generated catalog omits the `generated` block of each operation, because it only restated the registry.
- A change of the catalog changes the generated file and the route catalog digest of the storage activation identity. A database activated with the earlier catalog refuses the new package with `identity_mismatch`, as it did after each catalog change before.
