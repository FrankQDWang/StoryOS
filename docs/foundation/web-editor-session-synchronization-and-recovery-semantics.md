# Web Editor Session, Synchronization, and Recovery Semantics

- Status: current
- Canonical issue: [Specify Web Editor Session, Local Journal, Projection, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/70)
- Evidence owner: [Validate Production Editor Session, Synchronization, and Recovery Semantics](https://github.com/FrankQDWang/StoryOS/issues/69)
- Author admission: [Author Command Admission](author-command-admission.md)
- Core state machine: [Manuscript Revision and Proposal State Machine](manuscript-revision-proposal-state-machine.md)
- Public protocol: [Versioned Command, Query, Artifact, and Event Protocol](versioned-command-query-artifact-event-protocol.md)

## 1. Purpose and owner

This specification is the sole owner of Web Editor Session identity, local edit
continuity, pending projection, browser persistence, acknowledgement/Event
convergence, Snapshot resynchronization, writer takeover, and local garbage
collection.

StoryOS Core and PostgreSQL own authoritative state, command classification,
Receipts, Heads, and Project Activity. The Web Client maintains an immediate,
non-authoritative editing projection over those facts.

## 2. Editor Session and writer ownership

An `EditorSessionId` identifies one browser editing session for one exact User
and Project Scope. The Server grants one active Project writer generation at a
time. Tabs without the current generation remain read-only and continue to
observe Project Activity.

An explicit takeover obtains a newer writer generation, fences the prior
writer, and begins from a Server Snapshot plus reconciled local journal. A
stale generation cannot create a new Author Command Admission. The prior tab
retains its unsubmitted text as a Recovery Draft or copyable local draft.

## 3. Local Edit Journal

The active writer stores pending author work in an IndexedDB Local Edit
Journal. Every journal entry binds:

- User and Project Scope;
- Editor Session and writer generation;
- chapter and target identities;
- monotonically increasing local intent order;
- the completed editor intent or explicit editor command;
- base Snapshot, expected Heads, and editor-contract identity;
- idempotency key and canonical command digest when formed;
- local schema revision and creation time; and
- current local disposition.

The journal is non-authoritative. It contains only pending client continuity
data and never replaces a Core command, Receipt, Authoritative Revision,
Proposal Revision, Snapshot, or Project Activity record.

The Web Client verifies journal availability and schema before admitting new
editing input. A quota, corruption, or persistence failure preserves the
current in-memory text as a copyable Recovery Draft and moves the editor to a
read-only recovery state until journal continuity is restored.

## 4. Immediate pending projection

Completed author input appears immediately in a Pending Edit Projection. The
projection presents three author-visible states:

| State | Meaning |
| --- | --- |
| `saving` | journaled locally and awaiting a durable Core settlement |
| `saved` | the matching Receipt and resulting authoritative or Proposal Head are observed |
| `needs_attention` | Core refused or conflicted, resynchronization is required, or a Recovery Draft needs an explicit author action |

The projection preserves the exact current selection, IME result, local undo
group, and pending Draft. It does not change authority ownership or select a
Core write route.

## 5. Submission and convergence

Local intents remain ordered. The production risk prototype determines the
bounded submission cadence that preserves complete IME input, author-visible
undo, crash recovery, and the performance envelope. Each submitted unit binds
the current writer generation and uses the Core command meaning owned by the
Manuscript state machine plus one `AuthorCommandAdmissionId`.

HTTP acknowledgement and Project Activity may arrive in either order:

- an acknowledgement links the idempotency record and typed Receipt;
- Project Activity advances the authoritative projection and exact Heads;
- duplicates are idempotent;
- a sequence gap pauses new submissions and requests Snapshot/resync; and
- convergence requires both durable settlement and an observed projection at
  or beyond that settlement.

Only then may the matching journal payload be garbage-collected.

## 6. Reload, crash, resync, and chapter switching

On reload or process restart, the client:

1. opens and validates the exact Project journal;
2. obtains the current writer generation or performs explicit takeover;
3. fetches a bounded Server Snapshot and replay position;
4. reconciles every journal idempotency key with its admission and Receipt;
5. automatically invokes only the same exact, unexpired, unsettled
   `direct_editor_action` admission while every admission binding still
   matches;
6. surfaces every `requires_reconfirmation` settlement for an explicit,
   expired, binding-changed, or intent-unrecoverable command and never executes
   it automatically;
7. projects committed results in Server order; and
8. converts expired, stale, or unprovable intent into a Recovery Draft.

Chapter switching first commits pending browser state to IndexedDB. Each
chapter projection retains its own pending status while the Project keeps one
writer generation and one ordered submission queue.

A cursor below the replay floor, incompatible journal schema, lost writer
generation, or mismatched Head enters Snapshot/resync. The author continues
from the resulting durable projection plus any preserved Recovery Draft.

## 7. Proposal and refusal recovery

Proposal edits use the same journal and pending-projection machinery while
remaining Proposal-owned until Core settlement. Mixed ownership produces one
complete Refused Edit Draft. Structural target drift, stale Heads, or
unprovable replay produces one complete Recovery Draft or Proposal Recovery
Conflict.

The editor offers typed actions owned by the Core state machine: narrow, retry,
copy, expand or replan a Proposal, reject or withdraw, and discard. Every
state-changing action receives its own Author Command Admission and Receipt.

## 8. Deterministic verification

The required prototype and final contract tests cover:

- Chinese and English IME, paste, delete, selection replacement, and local undo;
- immediate projection, journal durability before network submission, and
  recovery after each local and server durable boundary;
- HTTP-before-SSE, SSE-before-HTTP, duplicates, gaps, and Snapshot/resync;
- reload, process crash, IndexedDB quota/corruption, and schema migration;
- automatic direct-edit reconciliation versus explicit-command
  reconfirmation after crash;
- chapter switching with pending edits;
- second-tab read-only behavior, takeover, stale-writer fencing, and preserved
  prior-tab text;
- authoritative, Proposal-owned, refused, conflicted, and no-effect Core
  outcomes; and
- garbage collection only after durable settlement and projection convergence.
