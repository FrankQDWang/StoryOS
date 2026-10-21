---
status: accepted
---

# Prove the Target of Author Undo Through the Compensation Lineage of the Chapter Head

The ticket [Let Prose Undo Go Back Many Steps](https://github.com/FrankQDWang/StoryOS/issues/1237) owns this ADR.

## Context

The author makes edit E1 and then edit E2 in one Chapter. Author Undo compensates E2. The Compensation writes a new Authoritative Revision C2. C2 has the prior payload and the prior Blocks of E2, which are the resulting Revision R1 of E1. The prior Revision of C2 is the Head before the Undo.

E1 is now the Author Undo Frontier. Core accepted a prose Undo only when the current Head was the resulting Revision of the Frontier. C2 has the content of R1 but another identity. Thus the next Undo always gave `conflicted` with `wrong_target_head`, and the editor became read-only. Undo Acceptance had the same defect. After an edit and its Undo, the Undo of an Acceptance gave a Reversal Proposal, which is a Barrier.

## Decision

- **Head lineage.** The lineage Revision of a Chapter Head is the Revision that the Head restores. Start at the current Head. When a Compensation wrote that Revision, go to the prior Revision of the source Forward Commit of that Compensation. Continue until a Revision that a Forward action or the Chapter creation wrote. When no Compensation wrote the Head, the lineage Revision is the Head.
- **Evidence.** The walk uses only the recorded Authoritative Commits and Author Actions of the Chapter. It does not compare text. A walk that the records cannot explain does not reach the resulting Revision of the Frontier.
- **Direct Author Action.** Core compensates the Frontier when two conditions are true. The current Head is the expected Head of the command. The lineage Revision of the current Head is the resulting Revision of the Frontier. Otherwise the result stays `conflicted` with `wrong_target_head`.
- **Acceptance.** The current Head must be the expected Head. Otherwise the result is `conflicted` with `wrong_target_head`. When the lineage Revision is the resulting Revision of the Acceptance, the Head is a Safe Compensation Head, and Core compensates the Acceptance. When the lineage Revision is another Revision, the Undo Acceptance outcome is reversal required. When the prior evidence is not usable, the outcome is unavailable.
- **Compensation records.** The Compensation does not change. It writes a new Revision with the prior payload and the prior Blocks of its source. Its prior Revision is the current Head.
- **Order.** Thus each Undo reverses the next older uncompensated Forward action. A chain of N edits can be undone N times. A later action that can be undone is not a Barrier: it becomes the Frontier first. After its Undo, the chain continues.
- **Barrier.** A Forward action with the Barrier disposition of ADR 0044 stops the chain. The reason stays `barrier`. No new reason code is added.
- **Editor.** When the Server reports an unavailable Undo (`no_frontier`, `barrier`, or `source_unavailable`), nothing changed. The editor stays writable and shows the reason. A `conflicted` result and an unknown result still make the editor read-only.
- **Storage.** No migration. The walk uses the existing index on the resulting Revision of an Authoritative Commit.
- **Scope.** Redo stays out of scope. [ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md) gives the same rule for the Undo of a Proposal action, from [#1249](https://github.com/FrankQDWang/StoryOS/issues/1249) and [#1244](https://github.com/FrankQDWang/StoryOS/issues/1244).

## Replaced rules

- The state machine (section 10.2) and the glossary said that a Safe Compensation Head is exactly the resulting Revision of the Acceptance. The lineage Revision of the Head must now be that Revision.
- The Core rule for a Direct Author Action compared the expected Head with the resulting Revision of the Frontier. It now compares the lineage Revision of the current Head.

## Considered options

- **Compare the text.** Compare the payload digest and the Blocks of the current Head with the resulting Revision. Rejected: this is an inference from content, and it accepts a Head from any source.
- **Move the Head back to the old Revision.** Rejected: it needs a migration and a change to the `AuthoritativeRevision` profile and its constraints. A Compensation also must not reactivate an old Revision identity.

## Consequences

- In production, a Reversal Proposal comes only from a Head that the lineage cannot explain.
- The adapter tests in `command_sequence_tests/undo_chain_tests.rs` and the exact-dist journey `s2-undo` prove the chain. The browser-source test `author-undo-stopped` proves the editor rule.
