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

| Scenario | Result | Draft Artifact | Condition | Preserved surface | Available author actions |
| --- | --- | --- | --- | --- | --- |
| `authoritative` | authoritative | — | — | manuscript | none |
| `proposal` | Proposal | — | — | editable Proposal | accept, reject |
| `refused` | refused | Refused Edit Draft | — | complete Refused Edit Draft | narrow, copy, expand into a Proposal, discard |
| `conflicted` | conflicted | — | Proposal Conflict | complete Proposal | replan from current authority, copy, reject |
| `no-effect` | no-effect | — | — | none | none |
| `recovery-draft` | recovery | Recovery Draft | — | complete Recovery Draft | retry, copy, discard |
| `proposal-recovery-conflict` | recovery | — | Proposal Recovery Conflict | complete Proposal | replan from current authority, copy, withdraw |

The Refused Edit Draft preserves all 50 characters across three lines. The
Recovery Draft preserves all 41 characters across three lines. Both Proposal
conflict paths display the same complete eight-paragraph, 287-character Proposal
that remains visible in the editor. No recovery surface exposes a stale accept
control. Refused Edit Draft and Recovery Draft are the matrix's only Draft
Artifacts. Proposal Conflict is a validation-axis condition and Proposal
Recovery Conflict is a fail-closed recovery condition; both remain conditions
on the preserved Proposal surface rather than new Artifacts.

The real Chrome interaction pass proved these transitions without a hidden or
partial write:

- copy reported an author-visible confirmation, and native `Meta-v` pasted the
  exact 50-character Refused Edit Draft into the narrowing editor while the
  immutable full attempt remained visible;
- narrowing retains the complete Refused Edit Draft while the author selects a
  smaller retry. The demonstration names the known single target
  `current chapter manuscript · chapter 12 end`, marks it authoritative-only,
  and shows one representative Core-reclassified authoritative settlement;
- expanding a Refused Edit Draft creates a fresh Proposal containing all three
  preserved lines;
- retrying a Recovery Draft appends all three lines exactly once, then settles
  to authoritative with no recovery controls;
- replanning a Proposal Conflict creates one fresh Proposal containing the exact
  eight preserved paragraphs;
- rejecting, withdrawing, or discarding settles to no-effect, removes the
  applicable candidate or recovery surface, and leaves authoritative prose
  unchanged;
- accepting a fresh Proposal writes its complete text atomically and removes all
  Proposal controls.

Narrowed retry and Proposal expansion deliberately do not share a result.
Expansion creates a Proposal. The authoritative-only narrowed example is
representative UX evidence, not a client-selected write route: every production
retry remains one `ApplyAuthorEdit` that Issue #46 Core reclassifies from current
Heads, Anchors, and ownership as authoritative, Proposal revised, refused,
conflicted, or no-effect.

The author-facing narrowed state uses only product language: `重试位置`,
`第十二章末尾 · 仅正文`, and an explanation that StoryOS will recheck the
current location before submitting the selected text. Classifier and prototype
boundary details remain in this evidence and the hidden harness, not in the
default writing surface. The narrowed authoritative settlement and Recovery
Draft retry both project `没有待处理提案`; neither fabricates Proposal
Acceptance, exposes `撤销接受并重新打开提案`, or retains recovery controls.
The ordinary Proposal accept path still records Acceptance and exposes its
applicable undo.

Tracked evidence:

- [approved source beside the Refused Edit Draft](artifacts/issue-45-approved-refused-comparison.png)
- [approved source beside the narrowed retry target](artifacts/issue-45-approved-narrowed-comparison.png)
- [Refused Edit Draft](artifacts/issue-45-refused-edit-draft.png)
- [authoritative-only narrowed retry target](artifacts/issue-45-narrowed-authoritative-target.png)
- [narrowed authoritative settlement](artifacts/issue-45-narrowed-authoritative-settled.png)
- [Proposal Conflict](artifacts/issue-45-proposal-conflict.png)
- [Recovery Draft](artifacts/issue-45-recovery-draft.png)
- [Recovery Draft authoritative settlement](artifacts/issue-45-recovery-draft-settled.png)
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
