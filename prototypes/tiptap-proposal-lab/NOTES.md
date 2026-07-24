# Tiptap Proposal Lab

> Disposable prototype. Delete or absorb it after the Wayfinder decision is recorded.

## Question

Can a rough Tiptap implementation demonstrate the settled editable-Proposal interaction while preserving the approved three-column StoryOS writing experience?

## Run

```bash
npm run dev
```

## What this pass demonstrates

- a fixed volume → chapter tree, manuscript editor, and collapsible Agent transcript;
- an ordinary Tiptap review projection whose Proposal paragraphs remain directly editable;
- accept, reject, reopen-after-reject, and safe undo-accept interactions;
- Proposal edits creating a new revision and resetting validation;
- Agent stream transactions excluded from ProseMirror history;
- synchronous author-priority stream pausing on `compositionstart`, `beforeinput`, paste, drop, and cut;
- whole refusal of mixed authoritative/Proposal transactions;
- exclusive same-block Proposal boundaries: exact start/end insertions remain
  authoritative while strict-interior insertions create a Proposal revision;
- refused cross-owner typing, paste, cut, and delete preserving the authoritative
  document while recording a `Refused Edit Draft`;
- version-evidence and per-session runtime-capability admission, with editable
  authoritative prose and disabled Proposal editing in `Proposal Safe Mode`;
- one newest-first undo route across editor history and Proposal acceptance,
  including a new Receipt for acceptance reapplication and an unsafe-action stop;
- structural Block ID checks for split, join, atomic move, StoryOS copy, one-to-one
  retype, and exact undo/redo restoration;
- Proposal Block Exclusivity and conflict-on-structural-reshape contract checks;
- local scratch persistence and reload of the document, Proposal axes, and transcript;
- a hidden prototype harness behind the lower-left settings control for stream,
  conflict, reload, reset, inline ownership, Safe Mode, and Block ID scenarios.

## Issue #45 production UX matrix

The production-shaped pass keeps the approved writing workspace and adds the
author-facing refusal, conflict, and recovery states directly after the affected
manuscript content. The harness remains hidden; each state is also reproducible
with `?scenario=<id>`.

| Scenario | Result | Preserved artifact | Available author actions |
| --- | --- | --- | --- |
| `authoritative` | authoritative | — | none |
| `proposal` | Proposal | editable Proposal | accept, reject |
| `refused` | refused | Refused Edit Draft | narrow, copy, expand into a Proposal, discard |
| `conflicted` | conflicted | Proposal Conflict | replan from current authority, copy, reject |
| `no-effect` | no-effect | — | none |
| `recovery-draft` | recovery | Recovery Draft | retry, copy, discard |
| `proposal-recovery-conflict` | recovery | Proposal Recovery Conflict | replan from current authority, copy, withdraw |

The Refused Edit Draft preserves all 50 characters across three lines. The
Recovery Draft preserves all 41 characters across three lines. Both Proposal
conflict paths display the same complete eight-paragraph, 287-character Proposal
that remains visible in the editor. No recovery surface exposes a stale accept
control.

The real Chrome interaction pass proved these transitions without a hidden or
partial write:

- copy reported an author-visible confirmation, and native `Meta-v` pasted the
  exact 50-character Refused Edit Draft into the narrowing editor while the
  immutable full attempt remained visible;
- narrowing retains the complete Refused Edit Draft while the author selects a
  smaller retry, and the selected text becomes one fresh Proposal;
- expanding a Refused Edit Draft creates a fresh Proposal containing all three
  preserved lines;
- retrying a Recovery Draft appends all three lines exactly once, then settles
  to authoritative with no recovery controls;
- replanning a Proposal Conflict creates one fresh Proposal containing the exact
  eight preserved paragraphs;
- rejecting, withdrawing, or discarding settles to no-effect, removes the
  candidate artifact, and leaves authoritative prose unchanged;
- accepting a fresh Proposal writes its complete text atomically and removes all
  Proposal controls.

Tracked evidence:

- [approved source beside the Refused Edit Draft](artifacts/issue-45-approved-refused-comparison.png)
- [Refused Edit Draft](artifacts/issue-45-refused-edit-draft.png)
- [Proposal Conflict](artifacts/issue-45-proposal-conflict.png)
- [Recovery Draft](artifacts/issue-45-recovery-draft.png)
- [Proposal Recovery Conflict](artifacts/issue-45-proposal-recovery-conflict.png)
- [machine-readable matrix](artifacts/issue-45-production-ux-matrix.json)

## Package baseline

- Tiptap React / StarterKit / UniqueID: `3.27.3`
- ProseMirror model / state / view: `1.25.11` / `1.4.4` / `1.42.1`
- React: `19.2.0`
- Vite: `6.4.2`

## Browser evidence from the contract pass

- Product support scope: desktop Google Chrome with Chinese and English author input.
- Real Google Chrome: the support-profile gate entered `FULL`; exact
  start/interior/end ownership passed; cross-owner typing, backward delete, and
  forward delete left the document unchanged and preserved the attempted result
  in a Draft. Runtime-capability and invariant failures entered Safe Mode and
  refused direct Proposal editing.
- Real Google Chrome: the Tiptap Block ID matrix passed for split, join, atomic move,
  StoryOS copy, one-to-one retype, and exact undo/redo.
- Real Google Chrome: after direct Acceptance, `Mod-z` reopened the Proposal while
  preserving the original Receipt; `Mod-Shift-z` performed a new Acceptance and
  created a distinct Receipt.
- Author manual verification on 2026-07-17: real Chinese Pinyin input completed
  correctly in the contract probe on supported desktop Google Chrome. This
  closes the prototype's real OS IME evidence gap; English direct input had
  already passed separately.
- Real Google Chrome native clipboard verification on 2026-07-17: `Mod-v` over
  the mixed `｜提` selection emitted `native_paste`, left the authoritative
  document unchanged, and preserved the attempted `原生粘贴` result in a
  `Refused Edit Draft`. `Mod-x` emitted `native_cut`, left the document
  unchanged, preserved its Draft, and placed the actually selected `｜提` text
  on the clipboard.
- Author manual verification on 2026-07-17: dragging the mixed `｜提` selection
  across the ownership boundary in real desktop Chrome emitted
  `native_dragstart`, `native_drop`, and `refused_edit_draft`. The authoritative
  document remained exactly `权威开头｜提案片段｜权威结尾`, while the attempted
  moved result was preserved as a `Refused Edit Draft`.
- `UniqueID` does not repair an arbitrary programmatic copy that retains the source
  ID. The StoryOS copy command must clear identity and record provenance before
  insertion so `UniqueID` can allocate the new Block ID.

## Reuse by the current production UX validation

- The real Chrome evidence above closes the generic input-mechanism questions for
  Chinese Pinyin, native paste and cut, and cross-owner text drag/drop. The
  reopened production UX validation must reuse this evidence rather than repeat
  event-capture tests.
- The remaining drag/drop work is the production-shaped refusal and recovery
  experience around the accepted mechanism: preserve the complete attempted text,
  make the non-authoritative result understandable, and expose the same applicable
  next actions as other refused edits.

## Boundary beyond this disposable browser/editor prototype

- This pass validates the author-facing Recovery Draft and Proposal Recovery
  Conflict presentation, complete-text preservation, and action semantics. It
  does not implement or redesign the durable Core reconciliation owned by issues
  #46 and #70.
