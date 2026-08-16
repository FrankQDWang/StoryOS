---
status: accepted
---

# Specify the Manuscript Revision and Proposal State Machine

StoryOS gives durable entities opaque typed UUIDv7 identities while keeping authority order, author-action order, and Proposal stream order in independent Project Scope-local sequences. Authoritative and Proposal objects use immutable linear Revision histories, versioned canonical digests, exact expected Heads, and one atomic Core Transition for records, Heads, Receipts, resolutions, events, and follow-up intent. Every first domain attempt has an exhaustive typed Receipt result whose allocation matrix determines whether it creates an Authoritative Commit, Author Action, Draft Artifact, or Proposal condition.

`ApplyAuthorEdit` is the single ownership-classifying entry point: Core recomputes current Heads, Anchors, reservations, and ownership, then atomically settles the whole command as authoritative, Proposal-revised, refused-to-Draft, conflicted, or no-effect. One completed semantic editor intent is the conservative commit boundary. Before Admission issuance, bounded idle coalescing is permitted only while every pre-issuance input that the Admission contract will bind and every #46 semantic field remain exactly equal under current hard limits; the final body and digest receive one Admission and are never merged afterward. The client never selects an authoritative route.

Release 1 binds this behavior to the #70-owned, structured [Author Edit prerelease policy](../foundation/author-edit-batch-release-1-policy.json), revision `storyos.author-edit-batch.release-1.preview.v1`. Its selected window and ceilings are replaceable conservative prerelease inputs supported only by synthetic browser evidence, not permanent or real-user-validated defaults. `storyos.editor-contract.release-1.v2` maps exactly to this policy and already participates in the command digest, so the wire needs no new field. Web proves complete local coverage before challenge creation; Core receives only the existing first-record anchor and exact ordered unit list and does not claim to detect browser-local omission, duplication, reordering, gaps, or policy identity independently.

Core applies each admitted unit in list order to one transient working body and settles the complete list as one indivisible command. An admitted `Refused`, `Conflicted`, or `NoEffect` result retains one typed zero-authority Receipt. Only a pre-Admission refusal or infrastructure/transaction failure before commit is Receipt-free. Neither boundary can create a new authoritative version, checkpoint, projection convergence, or base roll-forward; no intermediate unit or valid prefix becomes authority.

Proposals retain four orthogonal state axes, block-relative versioned Anchors, fail-closed target conflicts, exact-head Acceptance and lifecycle decisions, and typed newest-first compensation. A successful author-authored `ProposalRevised` allocates exactly one Forward Author Action even though it allocates no Authoritative Commit; refused, conflicted, and no-effect edits allocate none. The #69 production-editor harness is authoritative for completed-intent segmentation, acknowledgement, durability, and recovery mechanics, but its merged nullable Proposal/refused/conflicted/no-effect evidence row does not override this #46 domain allocation. Refused Edit Draft and Recovery Draft are the editor flow's only Draft Artifacts; Proposal Conflict and Proposal Recovery Conflict are conditions on preserved Proposal surfaces. Successful Draft retry, retry replacement, and Refused Edit Draft expansion atomically close the exact source Draft as `superseded` and return its lifecycle event, while conflict or no effect leaves the source unchanged and exact retry reuses the original settlement. Admission recovery treats post-admission uncertainty as `outcome_unknown`, reconciles from durable Receipt/idempotency facts, invokes automatically only the same unexpired direct edit when every binding required by Admission sections 3 and 5 plus all #46 facts match, and requires a fresh author confirmation for explicit, expired, changed, or unrecoverable commands. Missing response never proves non-commit.

This resolves [Specify the Manuscript Revision and Proposal State Machine](https://github.com/FrankQDWang/StoryOS/issues/46); the full normative contract is [Manuscript Revision and Proposal State Machine](../foundation/manuscript-revision-proposal-state-machine.md). [ADR 0004](0004-adopt-postgresql-service-and-project-isolation-boundary.md) requires every identity, Revision, Head, command, Receipt, sequence, and projection in this state machine to bind the same exact `ProjectScope { owner_user_id, project_id }`.

## Considered options

- Ordering by UUIDv7 or wall time was rejected because identity locality is not a causal or authoritative project clock.
- One mutable Proposal status was rejected because generation, validation, author resolution, closure, and retention change independently.
- Silent anchor mapping or revalidation after target drift was rejected because it would apply an inspected Proposal to a different base without explicit replanning.
- Client-selected authoritative versus Proposal write routes were rejected because the editor cannot grant itself authority.
- Range-level inverse patches over later target Heads were rejected because non-overlap inference is not the same as an exact safe compensation Head.
- Serialized DOM, ProseMirror history, or best-effort stream replay was rejected because editor and network processes are reconstructible projections rather than durable truth.
- Normalized editor transaction and fixed-window batching were rejected because they cross composition and frozen command bindings; one completed semantic intent plus equality-constrained bounded idle coalescing preserves the chosen semantic boundary.
- Treating synthetic 240-intent and 250 ms observations as permanent or real-user-validated defaults was rejected. #70 owns a replaceable conservative prerelease choice, and every semantic hard boundary still flushes immediately.
- Applying each unit as a separate Core command inside one browser group was rejected because it would allocate partial Receipts and authority and would let a later unit fail after an earlier unit committed.
- Treating the #69 harness's merged nullable Author Action field as the #46 domain allocation was rejected because it would remove successful Proposal edits from the unified Author Undo Frontier.
- Treating a missing response or absent unvalidated Receipt as non-commit was rejected because it can duplicate an Author Action after a post-admission crash.
- Representing refusal or recovery only in UI state, or representing Proposal conflict as a Draft, was rejected because durable Core artifacts and Proposal conditions must remain independently inspectable.

## Consequences

- Core and editor adapters need versioned Rust/TypeScript contracts plus shared digest and coordinate golden vectors.
- Author input may require an explicit replan or recovery Draft in conservative conflict and crash windows.
- Unified undo needs an independent Author Action Sequence, a derived Author Undo Frontier, explicit Compensation entries, and typed handlers; there is no generic durable redo.
- Every author command, Proposal decision, and undo route needs an exact typed result/Receipt and allocation test; exact retry returns those same identities without another effect.
- Multi-unit Author Edit verification must cover ordered coordinate evaluation, whole-command no-effect and refusal, limit rejection before Admission, transaction rollback without a prefix effect, and exact retry of the complete ordered digest.
- Web session, wire protocol, deterministic verification, evidence classification, and Admission contracts consume these semantic fields but retain their own lifecycle and serialization ownership.
- PostgreSQL schema, transaction isolation, durability, payload policy, and migration are implemented by the accepted [PostgreSQL Project Storage, Isolation, and Migration Contract](../foundation/postgresql-project-storage-isolation-and-migration-contract.md), which preserves the atomic logical boundary and Project Isolation defined here.
