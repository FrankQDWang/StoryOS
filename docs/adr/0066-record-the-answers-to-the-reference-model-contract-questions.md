---
status: accepted
---

# Record the Answers to the Reference Model Contract Questions

[Answer the Eleven Contract Questions of the Reference Model Study](https://github.com/FrankQDWang/StoryOS/issues/1087) owns this ADR. The reference model study of 2026-10-05 found 11 questions where the code selects a behavior that no contract states ([CONTRACT-QUESTIONS.md](../research/reference-model/CONTRACT-QUESTIONS.md)). A-007 was a model error and is retired.

## Context

The study used an earlier product base. On 2026-10-20 the executor examined each question again on `main` `e448962b`. The triage of 2026-10-17 recorded two author decisions: A-001 and A-009 keep the current behavior. On 2026-10-20 the author also decided A-004, A-005, A-008, and A-011. For the other questions, this ADR records the current behavior as the contract.

## Decision

| ID | Question | Answer | Owning text | Contract test |
| --- | --- | --- | --- | --- |
| A-001 | Can the author read the tree of an archived Project? | No. The Project query of an archived Project gives its Project. The tree query gives HTTP 404 `resource_unavailable`. The editor does not open the Project, and the author can export it. Author decision of 2026-10-17. | Glossary, Canonical Manuscript Tree | `archive-project-http.integration.test.ts` |
| A-002 | What are the first revision values? | A new Project starts at Manuscript Tree Revision 1 and Project Revision 1. The Project query does not give the Project Revision; the list, update, and archive responses give it. A Project with no assistance binding takes the expected assistance revision 0 and gets revision 1. | Glossary, Manuscript Tree Revision; this ADR | `manuscript-tree-http`, `archive-project-http`, `update-project-assistance-http` |
| A-003 | Which refusal of Set Current Chapter comes first? | `archived_project`, then `invalid_chapter_join`, then `empty_project`. A removed Chapter on an Empty Project gives `invalid_chapter_join`. | This ADR | `delete-chapter-http`, Core `set_current_chapter_tests.rs` |
| A-004 | What does a schema-valid zero revision or order give? | HTTP 400 `invalid_request` before Admission. The generated schema does not change. Author decision of 2026-10-20. | Protocol section 4.1 and the error table | `update-volume-http.integration.test.ts` |
| A-005 | Can the author undo an earlier direct edit after an Undo of a later one? | Yes. Prose Undo goes back many steps, as in a usual text editor. Author decision of 2026-10-20. [Let Prose Undo Go Back Many Steps](https://github.com/FrankQDWang/StoryOS/issues/1237) fixed the earlier `wrong_target_head` defect ([ADR 0067](0067-prove-the-target-of-author-undo-through-the-compensation-lineage-of-the-chapter-head.md)). Proposal candidate edits and author withdrawals also go back many steps ([ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md)). | Glossary, Author Undo Order and Head Lineage Revision; ADR 0067 | Adapter `undo_chain_tests.rs`: `author_undo_reverses_three_prose_edits_in_reverse_order`, `author_undo_continues_after_a_later_edit_is_also_undone`, `a_barrier_between_two_prose_edits_stops_the_chain`. Exact-dist `s2-undo.integration.test.ts` |
| A-006 | When does a candidate edit get its validation? | In the same Transition as the edit. The next read gives a new Validation Receipt and never `pending`. A Replan Revision starts at `pending`. | State machine sections 6.4, 7.2, and 7.4 | Existing `edit-proposal-candidate-http.integration.test.ts` |
| A-008 | Does Acceptance have a `NoEffect` result? | No. The foundation result set loses `NoEffect`, and the public contract does not change. Author decision of 2026-10-20. | State machine section 8.1 and its matrix; Artifact domain model; glossary, Acceptance Result | Existing Core `accept_proposal_tests.rs` |
| A-009 | Which Chapter becomes current after a Chapter create or delete? | The first Chapter on an Empty Project becomes current. A delete of the Current Chapter selects the next Chapter in tree order, also in another Volume, else the previous one, else an Empty Project. Author decision of 2026-10-17. | Glossary, Current Chapter | `delete-chapter-http`, `create-chapter-http` |
| A-010 | Can the author undo an Operation rejection? | No. A rejection is a Barrier, as ADR 0044 records. Author Undo gives `unavailable` with the reason `barrier`, and the rejection stays. | ADR 0044; state machine section 10.1 | `undo-latest-author-action-http.integration.test.ts` |
| A-011 | Can a Proposal edit give a structural sub-outcome? | No. In Release 1 the Server refuses a structural primitive on a Proposal target with HTTP 422 `command_target_refused` before Admission. `ProposalRevised` has no validation sub-outcome. Author decision of 2026-10-20. | State machine section 7.2 and its matrix; glossary, Proposal Structural Reshaping | Server `author_edit_tests.rs` |
| A-012 | What does an unknown Operation in a Proposal target give? | Admission does not examine it. Core gives `Refused` with `target_mismatch` when the expected Proposal Heads are current, else `Conflicted` with `proposal_head_present`. | State machine sections 2.2 and 7.2 | `edit-inline-proposal-http.integration.test.ts` |

## Considered options

- Add `NoEffect` to the public Acceptance effect (A-008). Rejected: no command can cause it, and it is a wire change.
- Make the structural sub-outcome reachable (A-011). Rejected: it needs a wire field, a Server profile change, and a new Core result.
- Make the generated schema refuse zero (A-004). Rejected: it is a wire change, and the expected assistance revision uses zero.

## Consequences

- The research report stays an evidence record. This ADR and the owning text are the answers.
- The acceptance criterion of #1087 that each observable answer has a contract test does not apply to A-005 in this ADR. The A-005 answer is not the current behavior, so a test of it fails until the fix. A test that keeps the one-step behavior is not permitted. #1237 has the test as an acceptance criterion: three prose edits are undone in reverse order. #1237 then writes the test names into the A-005 row, and it did so with ADR 0067.
- `empty_project` stays in the Set Current Chapter contract, but the current create and delete rules do not let a request get it.
