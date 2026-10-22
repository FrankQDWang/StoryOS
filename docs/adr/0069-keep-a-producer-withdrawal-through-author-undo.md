---
status: accepted
---

# Keep a Producer Withdrawal Through Author Undo

The ticket [Keep a Producer Withdrawal When Author Undo Restores a Proposal](https://github.com/FrankQDWang/StoryOS/issues/1252) owns this ADR. It adds one rule to the Proposal Compensations of [ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md).

## Context

The current producer of a Proposal can withdraw it (`current_producer_withdrawal`). This Withdrawal changes the closure of the head Revision to `withdrawn` in place. It writes no Author Action, so Author Undo cannot compensate it.

Example: the author makes candidate edit E1. Then the producer withdraws the Proposal. Then the author uses Undo on E1. The Compensation appended a Revision with the closure `open`, so the Proposal was open again. Thus Author Undo changed a decision of the producer, and no record showed it. The Compensations of a Replan and of a reopen of rejected Operations did the same, because they copy the closure `open` of the source Revision.

Only the HTTP route sends a producer Withdrawal today. No Web control and no AgentRun code sends it.

On 2026-10-22 the author decided: Author Undo reverses only the effects of the author. A producer Withdrawal is not an author action, so Author Undo keeps it. The Undo restores the content, and the Proposal stays `withdrawn`. The author can reopen the Proposal with the explicit reopen command.

## Decision

- **Kept closure.** Three Compensations keep a `withdrawn` head: those of a candidate edit, a Replan, and a reopen of rejected Operations. They write the closure `withdrawn` when the current head is `withdrawn`. Otherwise their closure does not change. For a candidate edit, it is `open`. For a Proposal decision, it is the closure of the source Revision.
- **Restored content.** The content, the Operations, the base, and the Anchor set are restored as ADR 0068 tells. The Undo of an earlier action continues as usual.
- **Why the head closure is sufficient.** Author Undo reverses the newest action first. Thus when one of these three Compensations runs, only a producer Withdrawal can explain a `withdrawn` head:
  - An author Withdrawal is a Forward action. Its Undo comes first, and that Compensation appends an `open` Revision.
  - The Compensation of a Refused Edit Draft expansion withdraws the head. But the expansion makes the Proposal, so no earlier action of that Proposal remains.
  - The Undo of an Acceptance does not reopen a `withdrawn` head. It makes a derived Proposal.
  - The head can be a Revision that this rule kept `withdrawn`. It can also be a copy of a kept Revision that the Undo of an explicit reopen wrote.
- **No evidence walk for the kept closure.** A walk to the producer Withdrawal record stops at the Undo of an explicit reopen. The parent of that Compensation Revision is `open`. Thus a walk can miss a producer Withdrawal and open the Proposal again. The head closure does not have this gap. A `withdrawn` head that no record explains also stays `withdrawn`. This is the safe result.
- **Explicit reopen.** The client has only the Withdrawal event that the producer response gave. After the Undo, the head is a new Revision, and the Withdrawal record names an older Revision. Thus the reopen finds the Withdrawal through a walk from the head. The walk goes from a Revision C to its parent when these conditions are all true:
  - A recorded Proposal Compensation appended C. Its Undo Receipt payload records C and its parent, as ADR 0068 tells, and its Compensation Author Action names the compensated Forward action.
  - That Forward action is a candidate edit, a Replan, or a reopen of rejected Operations.
  - The parent of C is `withdrawn`.
  The Withdrawal event matches when its Revision is the head or a Revision of this walk. The reopen copies the current head, so the reopened Proposal shows the restored content.
- **Reopen after the Undo of a reopen.** The walk does not go through the Compensation of an explicit reopen. Each Withdrawal event also has at most one reopen record. Thus after the Undo of an explicit reopen, a second reopen with the same Withdrawal gives `withdrawal_event_mismatch`. This occurs for an author Withdrawal too. The ticket [Close the Gaps in the Undo and Reopen of a Withdrawal](https://github.com/FrankQDWang/StoryOS/issues/1261) owns this limit.
- **Storage.** No migration and no new record. No trigger examines the closure of an appended Proposal Revision. The trigger branch of migration 0096 examines only the recorded Revision and its parent.

## Considered options

- **Walk to the producer Withdrawal record for the kept closure.** Rejected for the reasons in "No evidence walk for the kept closure".
- **Write a new Withdrawal record for the kept closure.** Rejected: the client does not know the new event. Also, a Withdrawal record with an Undo Receipt mixes two producers.
- **Make the Undo a Barrier when the head is withdrawn.** Rejected: the author decided that the Undo restores the content.

## Consequences

- The adapter tests in `command_sequence_tests/producer_withdrawal_undo_tests.rs` prove the kept closure for the three Compensations and for the Undo of an earlier edit. They also prove the explicit reopen after each Undo and the chain through the Undo of an explicit reopen.
- Three cases stay out of scope:
  - The Undo of an author Withdrawal W still opens the Proposal. This occurs after W, a reopen D, a producer Withdrawal of the Revision of D, and the Undo of D. The ticket [Close the Gaps in the Undo and Reopen of a Withdrawal](https://github.com/FrankQDWang/StoryOS/issues/1261) owns this case.
  - A Withdrawal event cannot be reopened again after the Undo of its reopen. The ticket [Close the Gaps in the Undo and Reopen of a Withdrawal](https://github.com/FrankQDWang/StoryOS/issues/1261) owns this case.
  - The state machine lets only the producer of the current Proposal Revision withdraw it. The code compares the producer of the Proposal, because a Proposal Revision records no producer. With the state machine rule, the producer cannot withdraw a Revision that the author wrote. Then these chains cannot occur, and this ADR stays as a defense. The ticket [Decide Which Producer Can Withdraw a Proposal](https://github.com/FrankQDWang/StoryOS/issues/1262) owns this question.
