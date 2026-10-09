---
status: accepted
---

# Give the Pending Edit Projection One Owner in the Editor Session Writing Controller

On 2026-10-09 the author accepted this decision for [Give the Pending Edit Projection One Owner in an Editor Session Writing Controller](https://github.com/FrankQDWang/StoryOS/issues/1042). It also decides [Keep continuous input in one Proposal candidate across its own edit settlements](https://github.com/FrankQDWang/StoryOS/issues/946). It applies to the Protected Web Client only. The Server, the public protocol, and the Local Edit Journal format do not change.

## Context

Three defects had one cause: an older read wrote the editor content over newer author input.

- [Make the exact-dist multi-location journey wait until the Proposal candidate block accepts input](https://github.com/FrankQDWang/StoryOS/issues/928): a reordered Proposal read started an Acceptance check. The editor was then read-only for some milliseconds.
- [Keep the editor editable while the Proposal checks run again after an edit settles](https://github.com/FrankQDWang/StoryOS/issues/943): the re-check after a settlement made the editor read-only.
- [Diagnose the Long-Session Browser Input Focus Failure](https://github.com/FrankQDWang/StoryOS/issues/707): an early Local Edit Journal read installed after a later input, and the editor removed the typed character.

Each fix was local. At `main` `58b70b3f`, the Pending Edit Projection has more than one owner:

- Ten statements in three modules write the mutable `pending` field of `EditorReadyState`. React state keeps a second copy.
- The idle controller serializes Author Edit. `ManuscriptEditor` serializes Author Undo. [ADR 0038](0038-separate-author-edit-challenge-admission-from-shared-command-admission.md) couples the two.
- `BlockProposalDisplay` reads the candidate text from the Journal itself.
- Five copies of "quiesce the editor, then run a command" are in `stage1-view.tsx` and `proposal-navigation.ts`.
- The production-unused textarea controller `attachManualInput` has its own queue, and it writes the projection too.

Issue #946 has the same cause. A Journal record of candidate input binds one exact Proposal Revision. Thus the editor refuses candidate input while an earlier edit of the same candidate is in progress. The A′ decision of issue #943 also locks the candidate after its own `proposal_revised` settlement, until the display shows the new Proposal Revision. An author who types without a pause in one candidate loses keystrokes.

## Decision

### One owner

- Each current-writer Editor Session has one Editor Session writing controller. The controller does not use React.
- Only the controller installs a Pending Edit Projection. Only the controller changes the editor text from a projection. `EditorReadyState` has no `pending` field.
- The controller serializes Author Edit and Author Undo in one queue. It owns the Author Edit idle batch, the Challenge wait, and Author Undo abandonment of ADR 0038.
- The controller has one entry that runs a structural command after the editor is quiet. This entry keeps the current two conditions. A chapter selection needs a journaled current intent only. A current-chapter change, a deletion, and a Proposal navigation also need no unsettled input.
- Views read the projection only through a subscription and a snapshot. The snapshot includes the candidate text of each Proposal operation, also input that the controller holds.
- `ManuscriptEditor` only translates Tiptap transactions into edits for the controller.
- The Local Edit Journal exposes one append entry for author edits. Its persist variants are private.

### Stale reads

The controller counts each captured input. A projection read that started before the newest captured input never installs. The controller installs a newer projection after the next append or settlement. Thus a stale read cannot change the editor text.

### Candidate input across its own settlement

- While an edit of one Proposal candidate is in progress, the editor accepts more input in that candidate. Typing, IME composition confirmation, and paste use this rule. IME composition behavior does not change.
- The controller holds that input in memory. It uses the bounded in-memory queue of ADR 0038 (240 operations).
- When the earlier edit settles as `proposal_revised`, the controller journals the held input against the new Proposal Revision. The new expected Proposal Heads replace the earlier Revision with the new Revision only.
- When the earlier edit settles with another result, the controller journals the held input against its original target. The controller does not submit it. The save state is `needs_attention`, and the text stays in the editor and in the Journal.
- While the earlier outcome is unknown, or while a Challenge waits for `Retry-After`, the controller continues to hold the input. When the queue exceeds its limit, the existing failure path applies: the editor becomes read-only, and the save state is `needs_attention`.
- A crash can lose the held input. The loss is at most the input of one candidate round trip. This is the same class of loss as the ADR 0038 queue.

This replaces the candidate lock of the #943 A′ decision. The purpose of A′ stays: input after a candidate settlement never targets the earlier Proposal Revision.

### Textarea write path

The client removes `attachManualInput` and its textarea-only test cases. [ADR 0017](0017-adopt-tiptap-for-production-block-replacement.md) retired the textarea write path. The exact-dist journeys keep the production IME and clipboard evidence.

## Considered options

- **Journal records with a relative Proposal target.** Rejected: it changes the IndexedDB record format and the database version.
- **The Server accepts an edit against the prior Proposal Revision of the same Editor Session.** Rejected: it changes the Author Edit contract.
- **Keep the candidate lock and tell the author that the candidate is saving.** Rejected: the author still loses keystrokes.
- **Discard held input after a failed earlier edit.** Rejected: it removes text that the author saw.
- **Keep `attachManualInput` as a second adapter of the controller.** Rejected: only tests use it, and it tests a retired write path.

## Consequences

- Three specifications under issue #1042 deliver the work in this order:
  1. The controller core and the fix of issue #946.
  2. Author Undo and the quiet entry.
  3. The Journal append entry and the removal of the textarea path.
- Deterministic tests go through the controller interface with injected timers and transport. Regression tests cover the three earlier defect paths and issue #946.
- The Web Editor Session contract, sections 5.1 and 5.3, records the owner and the hold rule.
