# Glossary: Proposal, Acceptance, undo, and Receipt

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Proposal**:
A Core Artifact containing inspectable, core-validatable domain changes together with their targets, base versions, and preconditions. It is the only Artifact kind that can become eligible for Acceptance, but Proposal identity alone never grants eligibility.
_Avoid_: Suggestion, direct write, executable extension

**Proposal Operation**:
A stable, independently resolvable domain change within a Proposal; dynamically calculated diff hunks are never operations. Historical applied or rejected incarnations remain frozen, while reopening creates a new Proposal Revision and retains the operation ID only when target and semantic identity are unchanged.
_Avoid_: Diff hunk, visual change marker

**Proposal Generation**:
One durably identified Agent production attempt for a Proposal, owning a strictly ordered and idempotent sequence of candidate batches under one active-writer boundary. A paused or completed Generation never reopens; explicit continuation creates a new Generation identity from an exact Proposal Revision.
_Avoid_: Network stream, resumable socket, mutable generator session

**Proposal Pause Fence**:
The immutable boundary that ends one Proposal Generation at an exact Proposal Revision and digest, admitted-through batch sequence, projection checkpoint, and Editor Input Fence. Later batches remain Run evidence but are permanently ineligible for Proposal or editor replay.
_Avoid_: Network cancellation, UI pause flag, queued late delta

**Editor Input Fence**:
The immutable Operational Record and automatic safety cause identified by `EditorInputFenceId` that binds the first completed author-input signal to one exact Editor Session, writer generation, local intent range, and active Proposal Generation before semantic command admission. It closes the Agent write gate but grants no author-command authority and receives no Author Action Sequence.
_Avoid_: Author Command Admission, browser event as domain command, reusable write permission

**Proposal State Axes**:
The orthogonal generation (`generating | ready_partial | ready`), validation (`pending | valid | invalid | conflicted`), closure (`open | withdrawn | superseded`), and per-Operation resolution (`pending | applied | rejected`) facts of a Proposal. Completion, partial application, and Acceptance Eligibility are derived projections rather than additional states, while Retention State remains separate.
_Avoid_: Proposal status, accepted Proposal state, rejected Proposal state, stale state

**Proposal Transition**:
One exhaustive StoryOS Core command-and-event change to a Proposal State Axis under exact expected Revision and lifecycle preconditions. Unlisted transitions are invalid; content correction, replanning, reopening, and Undo Acceptance append new Proposal Revisions rather than rewriting historical state.
_Avoid_: Status assignment, implicit transition, mutable historical resolution

**Proposal Anchor**:
The durable, versioned, block-relative address by which an InlineEditProposal Operation binds a stable Manuscript Block, exact base Authoritative Revision, coordinate and boundary contract, range, and canonical base-slice digest. A multi-block Operation carries ordered Anchors, while document-wide positions, DOM state, and editor decorations remain reconstructible projections.
_Avoid_: Absolute editor position, DOM range, Decoration identity, visible-text match

**Proposal Boundary Ownership**:
Author input exactly at either edge of an InlineEditProposal Operation belongs to the adjacent Authoritative State as a Direct Author Action; only input strictly inside the Operation edits the Proposal. Extending a Proposal across an edge requires an explicit Proposal edit.
_Avoid_: Inclusive Proposal edge, implicit Proposal growth, cursor affinity as authority

**Proposal Block Exclusivity**:
A stable top-level manuscript block may contain at most one unresolved InlineEditProposal Operation across all Proposals. Another Proposal targeting that block waits until the existing Operation is resolved or withdrawn, while Direct Author Actions outside its exact range remain permitted.
_Avoid_: Same-block Proposal concurrency, interval sharing, implicit Proposal merge

**Proposal Structural Reshaping**:
An author edit inside a pending Proposal Operation that splits, joins, moves, retypes, or changes the block span of its candidate content. StoryOS preserves the edit in a new Proposal Revision but projects a conflict until explicit replanning either proves unchanged semantic identity and retains the Operation ID or replaces it.
_Avoid_: Automatic anchor repair, rejected author input, silent Operation split

**Refused Edit Draft**:
A non-authoritative Draft Core Artifact created by the refused `ApplyAuthorEdit` Core Transition when one author edit crosses Authoritative State and Proposal ownership. It preserves the complete attempted structured payload, exact selection snapshot, and edit intent for narrowed retry, Proposal expansion, copy, or discard; a retry replacement or expansion closes the exact source as `superseded`, while no effect or a non-source conflict leaves it open and a source-binding conflict leaves its observed closure unchanged.
_Avoid_: Toast-only rejection, partial application, failed Direct Author Action

**Recovery Draft**:
A non-authoritative Draft Core Artifact created by an Editor Recovery Creator from one complete author-edit intent and its exact journal, admission-settlement, takeover, or in-memory recovery evidence as applicable. It requires explicit retry or discard and never applies automatically; a successful or replacement retry closes the exact source as `superseded`, while no effect or a non-source conflict leaves it open and a source-binding conflict leaves its observed closure unchanged, without turning any of those facts into a Receipt, Proposal condition, or proof of prior Core invocation.
_Avoid_: Autosaved truth, automatic crash replay, Refused Edit Draft

**Composition Edit**:
A complete author input intent bounded by one IME composition lifecycle and classified as a single edit only after the input method finishes while Agent document writes remain fenced. A single-owner result commits atomically, while mixed ownership restores the last durable projection and creates a Refused Edit Draft.
_Avoid_: Per-event authoritative write, cancelled IME as correctness, interleaved Agent write

**Proposal Safe Mode**:
A per-editor-session fallback used when the environment cannot uphold lossless Proposal editing, ownership recovery, or unified undo. Authoritative manuscript editing remains available while direct candidate editing and other unproven Proposal interactions are disabled without weakening authority checks.
_Avoid_: Weakened authority mode, blocked manuscript editor, silent compatibility downgrade

**Proposal Recovery Conflict**:
The fail-closed condition on a preserved Proposal surface in which durable Proposal Heads, stream sequences, Pause Fences, Anchors, digests, or an editor checkpoint cannot prove one unambiguous review projection. Agent replay and Acceptance remain disabled until explicit reconciliation; no cache or network order may fill the uncertainty. A separately preserved complete author intent may become a Recovery Draft, but the condition and Draft remain orthogonal.
_Avoid_: Best-effort replay, hidden repair, ordinary validation pending, Draft Artifact

**Editor Support Profile**:
The explicit product promise for manuscript editing environments and author input languages. StoryOS currently supports desktop Chrome with Chinese and English author input; behavior observed in other browsers or input languages is exploratory evidence, not a release gate or an implied support promise.
_Avoid_: Upstream browser matrix as product scope, every available IME, accidental compatibility promise

**Proposal Editing Admission**:
The fail-closed decision that permits one editor session to use full Proposal editing only when it belongs to the Editor Support Profile, its exact editor-contract versions and prior compatibility evidence match, and its live capabilities pass non-destructive checks. Unsupported, unknown, stale, mismatched, or violated evidence selects Proposal Safe Mode rather than weakening an invariant.
_Avoid_: User-Agent allowlist, feature presence as proof, optimistic compatibility

**Proposal Bundle**:
A Proposal subtype whose stable Bundle-level Operations reference exact child Proposal Revisions, selected child Operation IDs, and dependencies without copying child payloads. It declares atomic or ordered-independent execution, and Bundles cannot be nested.
_Avoid_: Mixed-domain Proposal, nested workflow

**Acceptance Eligibility**:
The predicate requiring an exact Proposal Revision to be retained, ready, valid for current targets, open, and selected only over pending Operations. Proposal identity, creator confidence, or a historical Validation Receipt cannot grant eligibility alone.
_Avoid_: Acceptable type, trusted Proposal

**Ready Partial**:
A Proposal generation outcome preserved after production stops before its intended completion. It remains editable but is not eligible for Acceptance until the author explicitly completes the current content or generation finishes.
_Avoid_: Failed Proposal, accepted partial

**Proposal Invalidity**:
The condition in which an exact Proposal Revision violates its own schema, Operation contract, or domain invariants even against its declared base. Invalidity belongs to the Proposal content rather than later target drift.
_Avoid_: Proposal Conflict, creator error message, low confidence

**Proposal Conflict**:
The Core-produced `conflicted` condition on an internally well-formed Proposal Revision's validation axis when its exact target, base Revision, Anchor, or preconditions cannot be proven against current Authoritative State; the detecting Receipt or Event is separate Operational evidence. Any referenced target Revision change conflicts even outside the proposed range, and recovery appends an explicitly replanned Proposal Revision rather than creating another Artifact, mutating the old Revision, or rebasing it in place.
_Avoid_: Proposal Invalidity, stale warning, automatic merge, silent anchor repair

**Proposal Rejection**:
An author's non-destructive decision not to apply selected pending Proposal Operations. Reopen creates a new pending Proposal Revision, which cannot regain Acceptance Eligibility until current targets and preconditions validate successfully.
_Avoid_: Withdrawal, deletion

**Proposal Withdrawal**:
A non-destructive removal of a Proposal from active review by its current producer or the author. Withdrawal is not represented as an author rejection.
_Avoid_: Rejection, deletion

**Safe Compensation Head**:
The condition in which an Applied Acceptance is the current Author Undo Frontier, has not already been compensated, and every affected target's current Head and payload digest exactly match the resulting Authoritative Revision recorded by its Receipt while the prior evidence remains usable. Any non-exact Head requires a Reversal Proposal or an unavailable outcome rather than range-level inference.
_Avoid_: Non-overlapping guess, inverse patch on a later Head, storage rollback

**Undo Acceptance**:
The typed Core handler reached only through `UndoLatestAuthorAction` when the exact Author Undo Frontier is an Acceptance. It appends compensating Authoritative Revisions only against a Safe Compensation Head and, when retained source content and a safe Proposal lineage allow it, a new Proposal Revision containing the previously applied content against the compensated base. Proposal lineage drift may derive a new Proposal but never blocks otherwise safe authoritative compensation; target Head drift instead requires a Reversal Proposal or unavailable outcome. It receives no second Admission or independently retryable command identity.
_Avoid_: Second undo command, history deletion, editor-only undo

**Acceptance Reapplication**:
An author redo of a successfully undone Acceptance is a new Acceptance attempt against the exact reopened Proposal Revision under current Acceptance Eligibility. It uses new command identity, Commit, and Receipt records, never restores the prior attempt, and becomes unavailable after relevant state drift.
_Avoid_: Redo Acceptance, Receipt replay, status rollback

**Author Action**:
The immutable `AuthorActionRef` Operational Record that the transaction of one successfully committed author-owned Core Transition creates. It binds its Author Action Sequence, its canonical Revision, Receipt, or Commit, and its `Forward | Compensation` disposition. A successful author-authored Proposal Revision receives one Forward Author Action, also without an Authoritative Commit. Admission, Editor Input Fence, refused, conflicted, invalid, no-effect, and recovery-Draft evidence create none. Project creation and Project setting changes are not author-owned Core Transitions, and they create none.
_Avoid_: Author Command Admission, physical-human gesture, browser history item, attempted command

**Author Action Sequence**:
The Project Scope-local continuous order assigned once to every successfully committed author-owned Core Transition, spanning authoritative changes, Proposal edits, resolutions, lifecycle decisions, and successful compensations. Automatic producer, validation, and input-safety transitions do not become author actions merely because they are visible or causally follow an Author Command Admission. The sequence binds the Transition's canonical Revision, Receipt, or Commit plus either a typed Forward disposition or a Compensation disposition naming the exact earlier action it settled; exact retries reuse it, while refused and no-effect attempts receive none.
_Avoid_: UUID order, wall-clock order, mutable action ledger, editor-history index

**Author Undo Frontier**:
The latest Forward Author Action Sequence that has not been named by a successful Compensation disposition. At most one committed Compensation may name a Forward action; Compensation entries remain in Author Action order for audit but are never themselves undo candidates, while a Reversal Proposal is a new Forward action and does not compensate its source.
_Avoid_: Maximum Author Action Sequence, compensation of a compensation, redo cursor

**Author Undo Order**:
A single newest-first order over uncompensated Forward author-owned actions, regardless of whether they changed Authoritative State or editable Proposal content. The Author Undo Frontier is its exact current candidate; an unsafe Frontier stops undo and requires its explicit reversal or unavailable disposition, and StoryOS never skips it to undo older work.
_Avoid_: Independent undo stacks, editor-first undo, silent history skip

**Author Undo Disposition**:
The declared Author Undo result of one Forward author-owned command kind and applied result: an exact Compensation of its effect, or a Barrier. A Barrier Frontier stops Author Undo with no Compensation, and Author Undo never skips it, so earlier Forward actions also stay uncompensated. Every Forward command kind declares its disposition.
_Avoid_: Undo support flag, best-effort inverse, compensation selected by Receipt shape

**Author Undo**:
An explicit-editor-command Author Command Admission that requests reversal of the exact Author Undo Frontier through its registered typed Core handler and records one immutable routing Receipt. A successful compensation appends its own Author Action Sequence entry naming that source, but is never a later undo target. Author Undo never skips a Barrier, applies a generic inverse patch, depends on editor history as truth, or creates a durable generic redo.
_Avoid_: Editor-only undo, arbitrary history rollback, universal inverse, redo stack

**Reversal Proposal**:
A Proposal that expresses the inverse of an earlier Acceptance against current Authoritative State when a direct Undo Acceptance would conflict with later changes. It requires ordinary inspection and Acceptance.
_Avoid_: Forced rollback, silent undo

**Acceptance**:
An explicit-editor-command Author Command Admission that applies a nonempty selection of pending Operations from an exact eligible Proposal Revision and current Validation Receipt through a StoryOS-owned domain handler. Domain Proposal selections are atomic, while Proposal Bundles obey their explicit atomic or ordered-independent policy.
_Avoid_: Promotion, status flip, overwrite

**Acceptance Attempt**:
The first Core execution of one schema-valid, authorized Acceptance command after idempotency resolution, with all current eligibility, targets, Anchors, preconditions, and domain effects revalidated before commit. Every Attempt produces one immutable Acceptance Receipt, while an exact retry only returns that Receipt and is not another Attempt.
_Avoid_: Acceptance retry, button click, Receipt lookup

**Acceptance Result**:
The exhaustive settlement of one Acceptance Attempt as Applied, Invalid, Conflicted, Refused, or NoEffect. Only Applied changes Authoritative State and resolves selected Operations as applied; infrastructure uncertainty is not a Result and is reconciled through the command's idempotency key.
_Avoid_: Success boolean, exception text, accepted Proposal state, unknown as failure

**Receipt**:
An immutable Operational Record produced only by StoryOS Core for one validation or domain-command attempt, carrying one exact typed Receipt identity and exactly one `AuthorCommandAdmission | EditorInputFence | AgentRunStep | ToolCall` producer cause. Its exhaustive result names every allocated Authoritative Revision, Proposal Revision, Commit, Author Action, Draft Artifact, or Proposal condition; an exact retry returns the same identities. Domain, Validation, Acceptance, Undo Acceptance, and Author Undo Receipt identities remain distinct, and no Receipt has Artifact revision, derivation, retention, Acceptance, or authority lifecycle.
_Avoid_: Generic receipt ID, Artifact, log text, producer assertion

**Acceptance Receipt**:
The durable outcome of one Acceptance attempt, identifying its command digest and idempotency key, exact Proposal Revision and selected operations, prior and resulting Authoritative Revisions, zero or more Authoritative Commits, child Receipts, and result. Bundle progress may be derived across linked attempt Receipts.
_Avoid_: Success message, accepted Artifact

**Domain Receipt**:
The typed Receipt kind for a Core domain-command attempt whose owning result contract does not define a more specific Validation, Acceptance, Undo Acceptance, or Author Undo Receipt identity. It records success, refusal, redirection, conflict, and no-effect outcomes without serving as a generic fallback for another Receipt kind.
_Avoid_: Generic Receipt, Validation Receipt, Acceptance Receipt, unknown-receipt fallback

**Undo Acceptance Receipt**:
The immutable, idempotent typed Receipt produced by an Undo Acceptance attempt, identifying the original Acceptance Receipt, command digest, and one outcome: compensated with a Commit, reversal required with a Reversal Proposal, or unavailable with a reason.
_Avoid_: Compensation Receipt, undo message

**Validation Receipt**:
The immutable result of StoryOS Core validating an exact Proposal Revision against exact target versions, domain invariants, and preconditions. It remains true for those historical inputs but ceases to be current or applicable after any relevant Proposal or target change.
_Avoid_: Model confidence, creator-approved flag
