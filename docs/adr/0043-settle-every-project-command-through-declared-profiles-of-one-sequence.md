---
status: accepted
---

# Settle Every Project Command Through Declared Profiles of One Sequence

On 2026-10-05 the author accepted this decision. It extends [ADR 0041](0041-own-the-project-command-sequence-in-one-adapter-module.md) from the six Manuscript Structure Transition commands to every implemented project command except Author Edit, `acceptProposal`, and Author Undo. One settlement sequence in `storyos-adapter-postgres` stays the only owner of the step order. Each command adapter declares a small set of profiles, and the sequence writes the records that those profiles name. A new project command must settle through this sequence.

## Context

At `main` `47922480`, the 20 commands in this scope still use hand-written transactions. None of them fits the ADR 0041 adapter as it is, because the database fixes a different set of authority records for each command family:

- The Project setting commands `updateProject`, `archiveProject`, and `updateProjectAssistance` write one Project Activity record. They write no Authoritative Commit, Author Action, or Snapshot. The AgentRun commands `createAgentRun`, `pauseAgentRun`, and `cancelAgentRun`, and the Activity of `steerAgentRun`, have the same shape.
- `setCurrentChapter` writes an Author Action, an Activity record, and a canonical Snapshot, but no Authoritative Commit (ADR 0026). Author Undo recognizes a Current Chapter action because it has no Commit.
- The Proposal decision and Refused Edit Draft commands write one Forward Author Action and no Activity, Commit, or Snapshot. Their applied Receipt result kind is specific to the command, for example `proposal_revised`.
- `steerAgentRun` and `takeOverProjectWriter` write effect rows and an Activity record for a `no_effect` outcome.
- The two export commands commit only the Author Command Admission and their work rows. The Worker writes the Domain Receipt later.

The receipt relation trigger, the Receipt shape checks, and the Activity payload key checks enforce these shapes. The replay paths also disagree: for the same fault, one command gives a binding conflict, another gives `historical_acknowledgement_unavailable`, and a third gives a store fault.

## Decision

### Scope

- A project command in this decision is a command that consumes one Project Command Challenge and settles one Author Command Admission. Each one settles through the sequence.
- These operations are not project commands in this sense, and they stay outside the sequence:
  - `createEditorSession`. It is a Challenge-fenced creation of an Editor Session without an Admission or a Receipt, at the default isolation level.
  - The current-producer form of `withdrawProposal`. It is an AgentRun decision without a Command Challenge or an Admission. The author form of `withdrawProposal` is in scope.
  - The Worker settlement of an admitted export. It is a fenced Worker transaction. The admission of each export is in scope.
- Author Edit, `acceptProposal`, and Author Undo need sequence capabilities that a later decision records. These are an `outcome_unknown` settlement with admission recovery, the Pre-Admission Refusal Record (ADR 0013), and the Compensation disposition.

### What a command adapter declares

- **Admission profile.** The action class (`explicit_project_command`, `explicit_editor_command`, `agent_run_start`, or `agent_run_control`). An editor command also supplies its Editor Session, Chapter, expected Authoritative Revision, and target references, and the Admission insert requires the current writer generation. An Admission insert that inserts no row returns the error that the command declares today, and the transaction rolls back.
- **Settlement profile of an applied outcome.** One of:
  - `Structural`: the ADR 0041 records.
  - `ChapterSelection`: an Author Action without an Authoritative Commit, an Activity record, a canonical Snapshot, and the writer-base rebind of the named Editor Session.
  - `ActionOnly`: one Forward Author Action.
  - `ActivityOnly`: one Activity record.
  The profile fixes which scope sequences the sequence allocates and which authority records it writes. The applied Receipt result kind is part of the profile: `authoritative_applied` or the command's own result kind.
- **Effects of a zero-authority outcome.** A command can write effect rows for a `no_effect`, `conflicted`, or `refused` outcome. It can also write an Activity record where its schema requires one. Such an outcome never allocates an Authoritative Commit or an Author Action.
- **Receipt shape.** The command supplies the head arrays, the Revision and Proposal Revision references, the draft and lifecycle references, and the payload of each outcome. The sequence inserts the Receipt.
- **Activity record.** The command supplies the event kind and the payload fields. The sequence adds `tree_revision` only for the `Structural` profile.
- **Response record.** One of these three:
  - the Command-response Project.
  - the Command-response Project with the Project assistance record.
  - no response record. Such a command decodes its whole acknowledgement from the stored Receipt and effect rows.
- **Admit-only first use.** For the two exports only, the first use commits the Admission, the work rows, and the Command-response Project. It leaves the Command Idempotency Fence `in_progress`. An exact retry of an admitted export replays the admitted operation. The Worker settles the fence later.
- **Refusal before Admission.** The fact load can refuse with a reason of the command, for example an archived Project or a missing AgentRun. This writes no row, and the Server keeps its problem code. It is not a Pre-Admission Refusal Record.

### What the sequence owns

- The step order of ADR 0041 applies to every command in scope. Every command locks the Project row first. Commands that did not lock the Project row before now lock it. Every command inserts its Admission after classification.
- Every Core classifier in scope returns a Core Transition Outcome. Each reason enum implements `ReasonCode`, and each code is equal to the existing SQL text and the existing wire text.
- The Server has one admission sequence for these commands. Each route declares its canonical body form, whether it holds the first acknowledgement, and its problem codes and messages. The persisted canonical command digests do not change.

### One replay rule

- Every command in scope replays through one replay query. A command can add one query that reads its own effect rows in the same read-only transaction.
- A missing settled record or an unknown reason gives a binding conflict. A pre-capture record gives `historical_acknowledgement_unavailable` (ADR 0032), but only for a command that has a response record. Damaged evidence is a store fault.
- These observable changes follow from this rule:
  - For `rejectProposalOperations`, `withdrawProposal`, `replanProposal`, `reopenWithdrawnProposal`, `reopenRejectedOperations`, `completeReadyPartialProposal`, and `continueProposalGeneration`, a missing settled record or an unknown result kind gives `409 idempotency_binding_conflict` instead of `409 historical_acknowledgement_unavailable`.
  - For `completeReadyPartialProposal` and `continueProposalGeneration`, an applied Receipt without its Proposal Generation transition record or its Forward Author Action is damaged evidence. It gives `503 project_store_unavailable` instead of `409 historical_acknowledgement_unavailable`. The transition record has no nullable stored value, so it has no pre-capture form.
  - For `steerAgentRun`, `pauseAgentRun`, and `cancelAgentRun`, damaged acknowledgement evidence gives `503 project_store_unavailable` instead of `409 idempotency_binding_conflict`.
  - For `setCurrentChapter`, a partial set of Author Action, Snapshot, and tree revision evidence gives `503 project_store_unavailable` instead of `409 idempotency_binding_conflict`.
  - For `takeOverProjectWriter`, a settled Receipt without its Activity record gives `503 author_edit_store_unavailable` instead of `409 idempotency_binding_conflict`. The Activity payload checks already prevent a payload with a missing field.
  - `closeEditorFlowDraft` and `expandRefusedEditDraftToProposal` replay in a separate read-only transaction, not in the write transaction. The response does not change.
  - For `closeEditorFlowDraft`, damaged acknowledgement evidence gives `503 project_store_unavailable` instead of `409 draft_binding_conflict`. Such evidence is a Receipt without a payload field, or an applied Receipt without its Forward Author Action. It is also a Draft close event that does not agree with its Receipt. The Receipt payload checks and the Draft close settlement trigger already prevent such a record.
  - For `expandRefusedEditDraftToProposal`, damaged acknowledgement evidence gives `503 project_store_unavailable` instead of `409 draft_expansion_binding_conflict`. Such evidence is an applied Receipt without its Forward Author Action, its Draft close event, or its Proposal Revision. It is also a Draft close event or Proposal Revision that does not agree with its Receipt. The Draft expansion settlement trigger already prevents such a record.
  - For the six Manuscript Structure Transition commands and `setCurrentChapter`, a later canonical Snapshot at the same Activity position no longer breaks an exact retry. Replay reads the earliest canonical Snapshot at that position, which is the one that the command wrote. Before this decision, such a retry gave `503 project_store_unavailable`.
- Each specification that moves commands lists every other observable difference in its behavior-equivalence review. An unlisted difference is a defect.

### Defect fixes in moved commands

A specification that moves commands can also fix a defect of those commands. Each fix is listed here with its observable change:

- For `reopenRejectedOperations`, `completeReadyPartialProposal`, and `continueProposalGeneration`, a Proposal with more than one Proposal Operation no longer gives `503 project_store_unavailable`. `reopenRejectedOperations` reads only the selected Proposal Operation, and an unknown one gives `operation_not_rejected`. The two generation decisions read the first Proposal Operation by identity, as `reopenWithdrawnProposal` does.
- For `rejectProposalOperations`, `withdrawProposal`, `replanProposal`, `reopenWithdrawnProposal`, and `reopenRejectedOperations`, an exact retry returns the Proposal State Axes values of the first delivery. Before, the retry read current Proposal Revision and Proposal Operation rows, which a later command can change in place.
  - Migration 0083 adds typed `preserved_*` columns with checks to the rejection, Proposal Withdrawal, replan, and reopening records. It adds the `proposal_withdrawal_reopenings` event table for `reopenWithdrawnProposal`. The Project archive export contains the new columns and the new table.
  - Each of the five commands writes these values in the transaction of its Receipt, and replay reads only these values. The current-producer form of `withdrawProposal` writes no preserved values. The migration has no backfill, because no production data exists.
  - An exact retry of a multi-operation rejection returns the resolution event of the first selected Proposal Operation, as the first delivery does. Before, the retry returned the event of the lowest Proposal Operation identity.
  - A settled effect row whose `preserved_*` columns are NULL is a pre-capture record and gives `409 historical_acknowledgement_unavailable`. An applied Receipt without its effect row or its Forward Author Action is damaged evidence and gives `503 project_store_unavailable`. Before, a missing effect row gave `409 historical_acknowledgement_unavailable` for four of the five commands, and a missing Forward Author Action gave it for all five.

## Relation to ADR 0041 and the glossary

ADR 0041 stays in force. ADR 0041 lets the sequence allocate authority records for an `Applied` outcome only in the `Structural` shape. This decision replaces that statement: the settlement profile now fixes the authority records of an `Applied` outcome. The glossary term Core Transition Outcome changes in the same way. Only Applied changes the target of the command, and its settlement profile fixes which authority records it allocates.

## Considered options

- One sequence for each command family, with shared record and replay functions, was rejected. It brings back several copies of the step order, which is the problem that ADR 0041 removes.
- One fault mapping for each command, which keeps every current replay byte, was rejected. It keeps several replay rules in one module. No production data exists, so the changes above have no migration cost.
- Moving `createEditorSession` into the sequence was rejected. It would add Admission and Receipt rows and change the isolation level.

## Consequences

- Rows, migrations, persisted formats, isolation levels, reason code texts, and HTTP statuses do not change, except as this decision lists.
- The single-method Store traits, the application binding self-checks, and the per-command replay queries of the commands in scope are removed. Each one goes when the last command of its family moves.
- A new project command needs a Core classifier with reason codes, one command adapter that declares its profiles, one Server route, and one contracts operation. A command whose records do not fit a declared profile needs a decision that adds a profile.
