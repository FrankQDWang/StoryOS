---
status: accepted
---

# Generate the Route Catalog From the Operation Registry

The repository cleanup specification of phase 2 ([#1298](https://github.com/FrankQDWang/StoryOS/issues/1298)) owns this ADR. The author approved its ticket graph on 2026-10-28. Ticket [#1299](https://github.com/FrankQDWang/StoryOS/issues/1299) adds the generated Route Catalog beside the hand-kept catalog. Ticket [#1300](https://github.com/FrankQDWang/StoryOS/issues/1300) moves the readers and removes the hand-kept catalog and its Python verifier. This ADR partly supersedes [ADR 0042](0042-derive-release-1-contract-artifacts-from-one-operation-registry.md).

## Context

The maintainer kept the Release 1 Route Catalog, `docs/foundation/versioned-protocol-release-1-route-catalog.json`, by hand. It had 6,314 lines. The Operation Registry of ADR 0042 already held the route, the kind, the schemas, the status codes, the fixtures, and the preconditions of each served operation. Thus a route change edited the same facts in two places.

The hand-kept catalog also stated facts that were not true. It listed 63 operations, but the Server serves 48. It listed 54 Events, but the code can persist and replay only the 22 Event schemas of `ProjectActivityKind`. For some served operations, its status codes and preconditions differed from the registry. Tests recorded these differences as known drift.

ADR 0042 took the registry order from the position of each operation in the hand-kept catalog.

## Decision

- **Source.** The Operation Registry is the only source of the Route Catalog. Each `RegisteredOperation` holds the catalog metadata of its operation: shape, semantic owner, admission and settlement rules, error profile, Project Activity mapping, notes, action coverage, cause profiles, and query proof. Rust enums hold the closed value sets. The generated JSON keeps the field names and the value strings of the hand-kept catalog.
- **Output.** `make generate-contracts` writes the catalog to `generated/route-catalog/versioned-protocol-release-1-route-catalog.json`. The drift check of `make check-fast` compares it with the registry, as it compares the other generated artifacts.
- **Facts from the registry.** The catalog takes the route, the kind, the schema IDs, the status codes, the fixtures, and the preconditions from the registry. When the hand-kept catalog differed from the registry, the registry value is the served contract.
- **Served operations only.** The catalog lists only the operations in the registry. The protocol document keeps the planned operations of later Stages. A Stage that serves one of them adds it to the registry, and the catalog gets it from there.
- **Events with a producer only.** The catalog Events are the Event schemas of each `ProjectActivityKind`, the kinds that the code can persist and replay. The Project Activity mapping of an operation names `ProjectActivityKind` values, so it cannot name an Event without a producer.
- **Catalog sections.** The owners, the error profiles, the error source, the release identity, the capability coverage, the non-public operations and Events, and the coverage contract are Rust constants.
- **Registry order.** The registry order is the declared order of `RELEASE1_OPERATIONS` and of the operations in each entry. It no longer comes from the hand-kept catalog. The current order stays, so no existing generated artifact changes its order. The generated catalog follows the registry order. This replaces the first point of the ADR 0042 decision. Until #1300 removes the hand-kept catalog, a test keeps the registry order consistent with it.
- **Cause profiles in the types.** `OperationCatalog::conditional_by_cause` is the only way to make a command conditional by cause. It takes one author cause and one current-producer cause. Each cause type holds its own binding and its own recovery rule, so an entry cannot mix them.
- **Rules that the types do not hold.** Rust tests check these rules on the generated catalog: a unique operation ID; a unique method and path; a unique schema ID over the operations, the cause profiles, the Events, and the release identity; a unique fixture ID over the operations and the Events; an Accepted command that names its own registered GET settlement query; cause profiles only on a conditional command; a synchronous settlement for each editor command; Project Activity only on a command; capability coverage of the required set with served operations only; error profiles that name only the stable errors of the protocol document, with the served status codes of each operation; and an owner source that resolves to a heading of a tracked document. The error profiles of `getProject` and `getChapter` name more status codes than the registry serves. The test pins this known difference.

## Considered options

- Keep the hand-kept catalog as a reviewed input and check it against the registry. Rejected: each route change still edits two files, and the catalog keeps the planned operations as if they were served.
- Keep the planned operations in the generated catalog with a planned marker. Rejected: the protocol document already holds the plan, and a client must not expect an operation that the Server does not serve.
- Keep the hand-kept values where they differed from the registry. Rejected: the registry produces the served OpenAPI document and the client, so it is the served contract.

## Consequences

- A new operation adds its catalog metadata to its own registry entry. The catalog gets the operation in the same change.
- A new Project Activity kind becomes a catalog Event when it joins `ProjectActivityKind`. The compiler requires its semantic owner.
- The generated catalog omits the `generated` block of each operation, because it only restated the registry.
- A change of the catalog changes the generated file. Until #1300, the readers of the hand-kept catalog and its pinned digest stay as they are.
