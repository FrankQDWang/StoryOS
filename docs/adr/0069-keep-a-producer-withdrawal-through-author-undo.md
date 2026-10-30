---
status: accepted
---

# Keep a Producer Withdrawal Through Author Undo

The ticket [Keep a Producer Withdrawal When Author Undo Restores a Proposal](https://github.com/FrankQDWang/StoryOS/issues/1252) owns this ADR. It adds one rule to the Proposal Compensations of [ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md). The ticket [Close the Gaps in the Undo and Reopen of a Withdrawal](https://github.com/FrankQDWang/StoryOS/issues/1261) amended it on 2026-10-31.

## Context

The current producer of a Proposal can withdraw it (`current_producer_withdrawal`). This Withdrawal changes the closure of the head Revision to `withdrawn` in place. It writes no Author Action, so Author Undo cannot compensate it.

Example: the author makes candidate edit E1. Then the producer withdraws the Proposal. Then the author uses Undo on E1. The Compensation appended a Revision with the closure `open`, so the Proposal was open again. Thus Author Undo changed a decision of the producer, and no record showed it. The Compensations of a Replan and of a reopen of rejected Operations did the same, because they copy the closure `open` of the source Revision.

Only the HTTP route sends a producer Withdrawal today. No Web control and no AgentRun code sends it.

On 2026-10-22 the author decided: Author Undo reverses only the effects of the author. A producer Withdrawal is not an author action, so Author Undo keeps it. The Undo restores the content, and the Proposal stays `withdrawn`. The author can reopen the Proposal with the explicit reopen command.

## Decision

- **Kept closure.** Three Compensations keep a `withdrawn` head: those of a candidate edit, a Replan, and a reopen of rejected Operations. They write the closure `withdrawn` when the current head is `withdrawn`. Otherwise their closure does not change. For a candidate edit, it is `open`. For a Proposal decision, it is the closure of the source Revision.
- **Restored content.** The content, the Operations, the base, and the Anchor set are restored as ADR 0068 tells. The Undo of an earlier action continues as usual.
- **Why the head closure is sufficient.** Author Undo reverses the newest action first. Thus when one of these three Compensations runs, only a producer Withdrawal can explain a `withdrawn` head:
  - An author Withdrawal is a Forward action. Its Undo comes first. That Compensation appends an `open` Revision, or a `withdrawn` Revision when a producer Withdrawal keeps the head withdrawn.
  - The Compensation of a Refused Edit Draft expansion withdraws the head. But the expansion makes the Proposal, so no earlier action of that Proposal remains.
  - The Undo of an Acceptance does not reopen a `withdrawn` head. It makes a derived Proposal.
  - The head can be a Revision that this rule kept `withdrawn`. It can also be a copy of a kept Revision that the Undo of an explicit reopen wrote.
- **No evidence walk for the kept closure of these three Compensations.** The head closure is sufficient for them, and it is the safe result: a `withdrawn` head that no record explains also stays `withdrawn`.
- **Undo of an author Withdrawal.** The head is always `withdrawn` when this Compensation runs, so the head closure tells nothing. The Compensation writes the closure `withdrawn` when a producer Withdrawal keeps the head withdrawn. The walk of the explicit reopen finds these Withdrawals. Otherwise it writes `open`, as before. Example: author Withdrawal W, reopen D with W, producer Withdrawal P of the Revision of D, Undo D, and Undo W. The Proposal stays `withdrawn`, and the reopen with P reopens it.
- **Explicit reopen.** The client has only the Withdrawal event that the response gave. After the Undo, the head is a new Revision, and the Withdrawal record names an older Revision. Thus the reopen finds the Withdrawals that keep the head withdrawn through a walk from the head. The walk starts at a `withdrawn` head and steps from a Revision C when a recorded Proposal Compensation appended C. Its Undo Receipt payload records C and its parent, as ADR 0068 tells, and its Compensation Author Action names the compensated Forward action. The steps are:
  - To the parent of C, when the parent is `withdrawn` and the Forward action is a candidate edit, a Replan, a reopen of rejected Operations, an explicit reopen, or an author Withdrawal.
  - To the withdrawn Revision of the explicit reopen, when the Forward action is an explicit reopen. Its reopen record names that Revision.
  The Withdrawal event matches when its Revision is the head or a Revision of this walk, and Author Undo did not compensate it. The reopen copies the current head, so the reopened Proposal shows the restored content.
- **Reopen after the Undo of a reopen.** The Undo of an explicit reopen gives the Withdrawal back. The walk goes through that Compensation, so the author can reopen the same Withdrawal again. This applies to an author Withdrawal and to a producer Withdrawal. Each reopen keeps its own record, so migration `0101_reopen_a_withdrawal_again.sql` removes the unique key on the Withdrawal event of `proposal_withdrawal_reopenings`. The stored records stay.
- **Storage.** No new record. Migration 0101 removes only one unique key. No trigger examines the closure of an appended Proposal Revision. The trigger branch of migration 0096 examines only the recorded Revision and its parent.

## Considered options

- **Walk to the producer Withdrawal record for the kept closure.** Rejected for the reasons in "No evidence walk for the kept closure".
- **Write a new Withdrawal record for the kept closure.** Rejected: the client does not know the new event. Also, a Withdrawal record with an Undo Receipt mixes two producers.
- **Make the Undo a Barrier when the head is withdrawn.** Rejected: the author decided that the Undo restores the content.

## Consequences

- The adapter tests in `command_sequence_tests/producer_withdrawal_undo_tests.rs` prove the kept closure for the three Compensations and for the Undo of an earlier edit. They also prove the explicit reopen after each Undo and the chain through the Undo of an explicit reopen.
- The adapter tests in `command_sequence_tests/withdrawal_reopen_undo_tests.rs` prove the second reopen of an author Withdrawal after the Undo of its reopen. They also prove that the Undo of an author Withdrawal keeps a later producer Withdrawal.
- Amendment (2026-10-26): the ticket [Decide Which Producer Can Withdraw a Proposal](https://github.com/FrankQDWang/StoryOS/issues/1262) closed a question that this ADR left open. The state machine let only the producer of the current Proposal Revision withdraw it, but the code compares the producer of the Proposal. The author decided that the producer of the Proposal can withdraw it, as the code does. The state machine and the glossary now say this. Thus the producer can withdraw a Revision that the author wrote, and the chains of this ADR can occur. The rule of this ADR is necessary, not only a defense.
