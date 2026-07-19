---
status: accepted
---

# Specify the Manuscript Revision and Proposal State Machine

StoryOS gives durable entities opaque typed UUIDv7 identities while keeping authority order, author-action order, and Proposal stream order in independent project-local sequences. Authoritative and Proposal objects use immutable linear Revision histories, versioned canonical digests, exact expected Heads, and one atomic Core Transition for records, Heads, Receipts, resolutions, events, and follow-up intent. Proposals retain four orthogonal state axes, block-relative versioned Anchors, fail-closed target conflicts, exact author-intent-bound Acceptance, typed newest-first compensation, and recovery from Core facts rather than editor or network state. This resolves [Specify the Manuscript Revision and Proposal State Machine](https://github.com/FrankQDWang/StoryOS/issues/46); the full normative contract is [Manuscript Revision and Proposal State Machine](../foundation/manuscript-revision-proposal-state-machine.md).

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
- Physical SQLite layout and durability policy remain deferred to [Specify the Self-Contained Project Storage and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56), but that storage must implement the atomic logical boundary defined here.
