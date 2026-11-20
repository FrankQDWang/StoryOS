---
status: accepted
---

# Remove the Three Proposal Recovery Commands That No Client Sends

The coordinator decided this on 2026-11-20 in the cleanup of Stage 5 to Stage 7 ([#1715](https://github.com/FrankQDWang/StoryOS/issues/1715)). Ticket [#1723](https://github.com/FrankQDWang/StoryOS/issues/1723) delivers it. This ADR amends [ADR 0064](0064-accept-a-proposal-on-an-earlier-base-by-replacing-its-target-text.md), [ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md), and [ADR 0069](0069-keep-a-producer-withdrawal-through-author-undo.md).

## Context

The Server had three Proposal recovery commands:

- `replanProposal` appended a pending Revision to a conflicted Proposal.
- `reopenWithdrawnProposal` opened a withdrawn Proposal again.
- `reopenRejectedOperations` made rejected Operations pending again.

ADR 0064 removed Replan from the editor and kept the command on the Server. The editor has no control for the two reopen commands. Thus no client sends the three commands. Only their HTTP tests, their Smoke Check rows, and the adapter tests call them. An Acceptance on an earlier base applies (ADR 0064), so a Proposal does not need a Replan. The author has three Proposal actions: accept, reject, and edit the candidate text.

## Decision

1. The three commands go from each layer: the contract rows, the command kinds, the route catalog, the Server handlers, the application entry points, the core command types, the PostgreSQL adapter commands, and the closed lists of the baseline migration ([ADR 0079](0079-squash-the-migrations-into-one-baseline-and-derive-the-storage-identity.md)).
2. The Author Undo of a Replan and of the two reopens goes with them.
3. The Author Undo of a candidate edit, of an author Withdrawal, of a rejection, and of an Acceptance does not change. The Author Undo of a rejection stays a Barrier.
4. The Compensation of an author Withdrawal keeps its check of a producer Withdrawal (ADR 0069). The two Withdrawal commands require the closure `open`, so without the explicit reopen no command can make a producer Withdrawal after an author Withdrawal on the same Proposal. The check stays as a defense, and no test makes that chain.
5. The tables `proposal_replans`, `proposal_withdrawal_reopenings`, and `proposal_operation_reopenings` stay in the baseline. The Project Export Archive reads them. No command writes them now. A later ticket can remove them with the archive format.

## Considered options

- **Keep the commands for a later client.** Refused: no client sends them, and each change of the Proposal state, of the Author Undo, or of the command sequence must keep them and their tests. A later need is a new ADR.
- **Keep the Author Undo of the removed commands.** Refused: no command can write the record that this Undo reverses.
- **Remove the three tables in the same change.** Refused: the Project Export Archive format names them, and that change is a separate decision.

## Consequences

- ADR 0064, ADR 0068, and ADR 0069 name the three commands, the Author Undo of each, and their tests. Those rules are historical. Each of these ADRs links to this ADR.
- The rules of the [Manuscript Revision and Proposal State Machine](../foundation/manuscript-revision-proposal-state-machine.md) for Replan and for the two explicit reopens are historical. A note at the top of that specification says so.
- A rejected Operation stays rejected, because the Author Undo of a rejection is a Barrier. A withdrawn Proposal opens again only through the Author Undo of an author Withdrawal. A conflicted Proposal stays acceptable, and a candidate edit validates it again (ADR 0064).
