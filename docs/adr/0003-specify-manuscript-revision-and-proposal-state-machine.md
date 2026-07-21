---
status: accepted
---

# Specify the Manuscript Revision and Proposal State Machine

StoryOS gives durable entities opaque typed UUIDv7 identities while keeping authority order, author-action order, and Proposal stream order in independent Project Scope-local sequences. Authoritative and Proposal objects use immutable linear Revision histories, versioned canonical digests, exact expected Heads, and one atomic Core Transition for records, Heads, Receipts, resolutions, events, and follow-up intent. Proposals retain four orthogonal state axes, block-relative versioned Anchors, fail-closed target conflicts, exact author-intent-bound Acceptance, typed newest-first compensation, and recovery from Core facts rather than editor or network state. This resolves [Specify the Manuscript Revision and Proposal State Machine](https://github.com/FrankQDWang/StoryOS/issues/46); the full normative contract is [Manuscript Revision and Proposal State Machine](../foundation/manuscript-revision-proposal-state-machine.md). [ADR 0004](0004-adopt-postgresql-service-and-project-isolation-boundary.md) requires every identity, Revision, Head, command, Receipt, sequence, and projection in this state machine to bind the same exact `ProjectScope { owner_user_id, project_id }`.

## Considered options

- Ordering by UUIDv7 or wall time was rejected because identity locality is not a causal or authoritative project clock.
- One mutable Proposal status was rejected because generation, validation, author resolution, closure, and retention change independently.
- Silent anchor mapping or revalidation after target drift was rejected because it would apply an inspected Proposal to a different base without explicit replanning.
- Client-selected authoritative versus Proposal write routes were rejected because the editor cannot grant itself authority.
- Range-level inverse patches over later target Heads were rejected because non-overlap inference is not the same as an exact safe compensation Head.
- Serialized DOM, ProseMirror history, or best-effort stream replay was rejected because editor and network processes are reconstructible projections rather than durable truth.

## Consequences

- Core and editor adapters need versioned Rust/TypeScript contracts plus shared digest and coordinate golden vectors.
- Author input may require an explicit replan or recovery Draft in conservative conflict and crash windows.
- Unified undo needs an independent Author Action Sequence, a derived Author Undo Frontier, explicit Compensation entries, and typed handlers; there is no generic durable redo.
- PostgreSQL schema, transaction isolation, durability, payload policy, and migration are implemented by the accepted [PostgreSQL Project Storage, Isolation, and Migration Contract](../foundation/postgresql-project-storage-isolation-and-migration-contract.md), which preserves the atomic logical boundary and Project Isolation defined here. This consequence supersedes the earlier SQLite assumption without changing the state machine.
