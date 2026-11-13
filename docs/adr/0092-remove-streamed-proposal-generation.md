---
status: accepted
---

# Remove Streamed Proposal Generation

On 2026-11-13, the author decided to remove streamed Proposal generation from the product ([record](https://github.com/FrankQDWang/StoryOS/issues/1463#issuecomment-6853155178)). Ticket [#1479](https://github.com/FrankQDWang/StoryOS/issues/1479) of the specification [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451) does the removal. After this ADR, a Proposal opens complete, and each Proposal Revision is `ready`.

## Context

Before this ADR, the Worker could open a Proposal in the generation state `generating` and append its candidate text in batches. An Author Edit in the same Chapter paused the generation: it wrote an Editor Input Fence and a Proposal Pause Fence, and it appended a Proposal Revision with the state `ready_partial`. Two public commands then changed the generation: `completeReadyPartialProposal` made the partial Revision `ready`, and `continueProposalGeneration` started a new generation.

The author gave these reasons for the removal:

- The streamed path started only when the reply form fake read the fixed test message "Stream this passage:". The real route could not start it.
- The Host Tool `propose_edit` opens a Proposal with no stream ([ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md)).
- The product code of the Web Client called neither `continueProposalGeneration` nor `completeReadyPartialProposal`, and it showed no generating, paused, or partial state of a Proposal. Only HTTP tests called the two commands.

A later design that shows an edit while the model writes it starts from the argument stream of `propose_edit`, not from this code.

## Decision

- **Commands.** Release 1 has no `continueProposalGeneration` and no `completeReadyPartialProposal`. Their routes, operation rows, request and response schemas, golden-wire fixtures, TypeScript functions, Command Challenge targets, command kinds, Core Transitions, application inputs, and PostgreSQL settlements go.
- **Streamed open.** No code path opens a Proposal in the state `generating`. The Worker opens a Proposal of a selected prose change in one step, with the generation `ready`.
- **Pause.** An Author Edit does not pause a Proposal generation. The Author Edit transaction has no pause step before its admit step.
- **Storage.** The tables `editor_input_fences`, `proposal_pause_fences`, and `proposal_generation_transitions` go from the release baseline. The command kinds of the two commands, the result kinds `proposal_generation_completed` and `proposal_generation_started`, and their CHECK and trigger branches go. Each CHECK of a generation value permits only `ready`. There is no released database, so the baseline changes in place ([ADR 0079](0079-squash-the-migrations-into-one-baseline-and-derive-the-storage-identity.md)).
- **What stays.** The revision of a candidate ([ADR 0062](0062-make-the-exact-selection-references-of-a-message-the-working-target.md)) and the Proposal Growth ([ADR 0071](0071-let-the-agent-grow-a-block-proposal-over-adjacent-chapter-blocks.md)) write one complete generation batch. Thus the tables `proposal_generations`, `proposal_generation_heads`, and `proposal_stream_events`, the classification of a generation batch, and the read of a revised location through its stream event stay. The column `generation` of a Proposal Revision and the public field `generation` of a Proposal read stay. They hold `ready` only. A removal of this axis is a separate decision.
- **Author.** The author sees no change.

## Superseded parts

This record supersedes these parts of earlier records:

- [ADR 0001](0001-separate-authoritative-state-artifacts-and-operational-records.md): Editor Input Fences in the list of Operational Records.
- [ADR 0038](0038-separate-author-edit-challenge-admission-from-shared-command-admission.md): `completeReadyPartialProposal` and `continueProposalGeneration` in the Proposal creation and review commands.
- [ADR 0043](0043-settle-every-project-command-through-declared-profiles-of-one-sequence.md): the replay rules and the Proposal Operation rule of `completeReadyPartialProposal` and `continueProposalGeneration`, and the Proposal Generation transition record.
- [ADR 0044](0044-settle-author-edit-acceptance-and-author-undo-through-the-command-sequence.md): the transaction that pauses Proposal generation before an Author Edit, and the Barrier disposition of the two commands.
- [ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md): the two commands as examples of a Barrier Frontier.
- [ADR 0076](0076-settle-every-project-command-through-one-sequence-skeleton.md): the transaction that pauses Proposal generation before the admit step, and the Barrier disposition of the two commands.
- The foundation documents that state a rule of the streamed path as current. Each has a note with a link to this ADR. The research records do not change.

## Considered options

- Keep the two commands and the streamed path for a later stream design. Rejected by the author: no product path starts it, and a later design starts from the argument stream of `propose_edit`.
- Remove the generation axis and its three remaining tables too. Rejected for this ADR: the revision of a candidate uses them, and the change is larger than the removal of the streamed path.
- Remove the stream script of the fake only, and keep the commands. Rejected: two public commands that no path can make useful are surface with no use.

## Consequences

- The Smoke Check, the route catalog, the operation table, and the persistence catalog no longer name the two commands or the three tables.
- A request to a removed route gets the Server answer for an unknown route.
- The replay of the Proposal decisions no longer has a decision whose records came after the capture of the preserved Proposal State Axes. Each Proposal decision that reads a pre-capture record gives a historical acknowledgement.
- The reply form fake has no stream script. Its other scripts stay until [#1463](https://github.com/FrankQDWang/StoryOS/issues/1463) removes the reply form.
