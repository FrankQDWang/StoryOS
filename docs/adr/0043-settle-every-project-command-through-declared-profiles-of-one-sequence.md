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
    - Its first use keeps some facts of its acknowledgement in the Command Idempotency Fence. Later commands can change these facts: the base snapshot, the writer state, and the Author Undo Frontier. An exact retry returns the first acknowledgement from these facts and from the Revision payload. A fence without them predates migration 0084 and gives `409 historical_acknowledgement_unavailable` (ADR 0032). Before 2026-10-09, an exact retry read the current Editor Session, and returned a later base after an edit ([issue 1029](https://github.com/FrankQDWang/StoryOS/issues/1029)).
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
  - For `expandRefusedEditDraftToProposal`, the target read and the decode of the retained Draft payload now come before the Admission insert. When the writer is stale and the target read or the payload decode fails, the command gives `503 project_store_unavailable` instead of `409 draft_expansion_binding_conflict`.
- A first use that fails rolls back its transaction, and a rollback that fails gives a store fault. Before, `createVolume`, `updateVolume`, `deleteVolume`, `createChapter`, `updateChapter`, `deleteChapter`, `updateProject`, `archiveProject`, `updateProjectAssistance`, `setCurrentChapter`, and `takeOverProjectWriter` ignored a failed rollback and returned the first error.
- Every Core classifier in scope returns a Core Transition Outcome. Each reason enum implements `ReasonCode`, and each code is equal to the existing SQL text and the existing wire text.
- The Server has one admission sequence for these commands. Each route declares its canonical body form, whether it holds the first acknowledgement, and its problem codes and messages. The persisted canonical command digests do not change.

### One replay rule

- Every command in scope replays through one replay query. A command can add one query that reads its own effect rows in the same read-only transaction.
- A missing settled record or an unknown reason gives a binding conflict. A pre-capture record gives `historical_acknowledgement_unavailable` (ADR 0032), but only for a command that has a response record. Damaged evidence is a store fault.
- These observable changes follow from this rule:
  - For `rejectProposalOperations`, `withdrawProposal`, `replanProposal`, `reopenWithdrawnProposal`, `reopenRejectedOperations`, `completeReadyPartialProposal`, and `continueProposalGeneration`, a missing settled record or an unknown result kind gives `409 idempotency_binding_conflict` instead of `409 historical_acknowledgement_unavailable`.
  - For `completeReadyPartialProposal` and `continueProposalGeneration`, an applied Receipt without its Proposal Generation transition record or its Forward Author Action is damaged evidence. An applied Receipt without its `transition` payload value is also damaged evidence. Each gives `503 project_store_unavailable` instead of `409 historical_acknowledgement_unavailable`. The transition record has no nullable stored value, so it has no pre-capture form.
  - For `completeReadyPartialProposal` and `continueProposalGeneration`, a stored state value of the Proposal Generation transition record that its Proposal State Axis does not have is damaged evidence. It gives `503 project_store_unavailable`. Before, replay returned that value. The transition record has no check for these values.
  - For `rejectProposalOperations`, `withdrawProposal`, `replanProposal`, `reopenWithdrawnProposal`, and `reopenRejectedOperations`, an applied Receipt without its `rejection_reason` or `transition` payload value is damaged evidence. It gives a store fault. Before, replay ignored that value. The Receipt shape checks already prevent such a record.
  - For `steerAgentRun`, `pauseAgentRun`, and `cancelAgentRun`, damaged acknowledgement evidence gives `503 project_store_unavailable` instead of `409 idempotency_binding_conflict`.
  - For `setCurrentChapter`, a partial set of Author Action, Snapshot, and tree revision evidence gives `503 project_store_unavailable` instead of `409 idempotency_binding_conflict`.
  - For `setCurrentChapter`, a newest tree revision value that is not decimal text gives `503 project_store_unavailable`. Before, replay skipped that value and used an older one.
  - For `takeOverProjectWriter`, a settled Receipt without its Activity record gives `503 author_edit_store_unavailable` instead of `409 idempotency_binding_conflict`. The Activity payload checks already prevent a payload with a missing field.
  - `closeEditorFlowDraft` and `expandRefusedEditDraftToProposal` replay in a separate read-only transaction, not in the write transaction. The response does not change.
  - For `closeEditorFlowDraft`, damaged acknowledgement evidence gives `503 project_store_unavailable` instead of `409 draft_binding_conflict`. Such evidence is a Receipt without a payload field, or an applied Receipt without its Forward Author Action. It is also a Draft close event that does not agree with its Receipt. The Receipt payload checks and the Draft close settlement trigger already prevent such a record.
  - For `expandRefusedEditDraftToProposal`, damaged acknowledgement evidence gives `503 project_store_unavailable` instead of `409 draft_expansion_binding_conflict`. Such evidence is an applied Receipt without its Forward Author Action, its Draft close event, its Proposal Revision, or its `reason` payload value. It is also a Draft close event or Proposal Revision that does not agree with its Receipt. The Draft expansion settlement trigger already prevents such a record.
  - For both Draft commands, damaged evidence is also a Receipt Draft revision, event, or Proposal identity that is not UUID text. It is also a payload digest that is not 64 lowercase hexadecimal digits, or an unknown observed closure. For the expansion, it is also a missing target Revision field, or an applied Receipt without a target Revision.
  - For both Draft commands, a zero-authority Receipt must record a null close event identity. For the expansion, it must also record null Proposal identities. Another value or an absent field is damaged evidence.
  - For each `ActionOnly` command, an applied Receipt whose Author Action is not Forward is damaged evidence and gives a store fault.
  - For every command in scope, a zero-authority Receipt with an Author Action is damaged evidence and gives a store fault. Before, the two Draft commands gave a binding conflict for such a record, and the other commands ignored the Author Action. The receipt relation trigger already prevents such a record.
  - For every command in scope, a Receipt, Activity, or effect payload field with the wrong JSON type is damaged evidence and gives a store fault. Before, replay read such a value as text. The payload checks already prevent a wrong type in most fields.
  - For the six Manuscript Structure Transition commands, `updateProject`, `archiveProject`, `updateProjectAssistance`, `setCurrentChapter`, and `takeOverProjectWriter`, a required Activity field that is absent or null is damaged evidence. It gives a store fault instead of `409 idempotency_binding_conflict`. An identity that is not UUID text and a number that is not unsigned decimal text are also damaged evidence. Before, replay returned such a value.
  - For `createVolume` and `createChapter`, only an absent Receipt order is a historical acknowledgement. A null order or an order of zero is damaged evidence and gives a store fault. Before, a null order gave the historical acknowledgement, and an order of zero gave a binding conflict.
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
  - A preserved value that its Proposal State Axis does not have is damaged evidence and gives `503 project_store_unavailable`. The checks of migration 0083 already prevent such a value in the five effect tables.

### Amendment for Specification C: AgentRun commands and export admissions

Specification C ([#1043](https://github.com/FrankQDWang/StoryOS/issues/1043)) moves `createAgentRun`, `pauseAgentRun`, `cancelAgentRun`, `steerAgentRun`, and the admissions of the two exports into the sequence. Each ticket adds its sequence changes and the observable changes of its commands to this section.

#### Refusal before Admission

- Each command declares its settlement error. A command that refuses before its Admission declares `RefusableCommandError` with its own refusal type. The other commands declare `ProjectCommandError`, and their errors and problems do not change.
- The fact load returns the refusal as a typed error. The sequence rolls back the transaction, so it writes no Admission, Receipt, or Command Idempotency Fence row, and the Command Challenge stays unused. A rollback that fails gives a store fault.
- The Server maps the refusal to the problem code of the command on `main`.
- The same typed error can carry the missing-Admission diagnosis of `acceptProposal` (ADR 0044).

#### AgentRun Admission forms

- An Admission without an Editor Session declares one action class: `explicit_project_command`, `agent_run_start`, or `agent_run_control`. The three forms write the same columns. Only the action class differs.
- A missing Admission gives `InvalidChallenge`, as on `main`.

#### `pauseAgentRun`

- `pauseAgentRun` uses the `agent_run_control` Admission form, the `ActivityOnly` profile with the Activity kind `agent_run_paused`, and the Command-response Project. It locks the Project row and then the AgentRun row. It does not refuse an archived Project.
- A missing AgentRun is a refusal before Admission. It gives `404 resource_unavailable`, as on `main`.
- The route uses the generic project command admission with the problem order of `main`. The problem texts use the name "AgentRun control", as on `main`.
- The applied outcome, `already_paused`, `terminal_run`, the Run update, and the rows do not change.
- These observable changes follow from the sequence:
  - A failed rollback after a failed first use or a refusal gives `503 project_store_unavailable`. Before, it was ignored.
  - Damaged replay evidence gives `503 project_store_unavailable` instead of `409 idempotency_binding_conflict`. Such evidence is a damaged Command-response Project record or an applied Receipt without its Activity record. It is also an Activity `run_id` or `fence_generation` value that is absent, null, or not a string. The receipt relation trigger and the Activity payload checks already prevent the Activity faults.
  - An Activity `run_id` that is not UUID text, or a `fence_generation` that is not unsigned decimal text, gives `503 project_store_unavailable`. Before, replay returned the `run_id` text and parsed a `fence_generation` with leading zeros.
  - A zero-authority Receipt with an Author Action gives a store fault. Before, replay ignored the Author Action. The receipt relation trigger already prevents such a record.
  - An applied Receipt with a `reason` payload value replays as applied. Before, it gave `409 idempotency_binding_conflict`. The Receipt shape checks already require an empty applied payload, so this change is not observable.

#### `cancelAgentRun`

- `cancelAgentRun` uses the `agent_run_control` Admission form, the `ActivityOnly` profile with the Activity kind `agent_run_cancelled`, and the Command-response Project. It locks the Project row and then the AgentRun row. It does not refuse an archived Project.
- A missing AgentRun is a refusal before Admission. It gives `404 resource_unavailable`, as on `main`.
- The route uses the generic project command admission with the problem order of `main`, as `pauseAgentRun` does.
- The applied outcome keeps its order of effects. It reads the in-flight Model Attempt before the status update. It sets the wakeup only for an in-flight Model Attempt. It prohibits the automatic successor. The fence token, lease, and wakeup values of each update do not change. Thus the Worker sends one Abort and fences late output as before (ADR 0039).
- The applied outcome, `already_cancelled`, `terminal_run`, and the rows do not change. The zero-authority outcomes write no Activity record, as on `main`.
- The Core classifier returns a Core Transition Outcome with the reasons `already_cancelled` and `terminal_run`.
- The observable changes of `pauseAgentRun` in the list above also apply to `cancelAgentRun`, with the same causes. The behavior-equivalence review against `main` found no other difference.

#### Contention refusal

- A command can declare the refusal for a serialization failure, a unique violation, or a deadlock of its first-use transaction. This includes the commit. The sequence finds such a failure in the source chain of a store fault. It rolls back, so no row stays and the Command Challenge stays unused.
- Only `createAgentRun` declares a contention refusal: `conversation_busy`. For the other commands, such a failure stays a store fault.
- A failure to begin the transaction, a failure of the Challenge consumption, and a failure of a rollback keep their own errors.

#### `createAgentRun`

- `createAgentRun` uses the `agent_run_start` Admission form, the `ActivityOnly` profile with the Activity kind `agent_run_created`, and the Command-response Project. Its Core classifier returns a Core Transition Outcome that has only the applied outcome.
- An archived Project, unavailable assistance, an invalid Chapter join, an inaccessible conversation, and a busy conversation are refusals before Admission. Their problems do not change: `422 archived_project`, `422 assistance_unavailable`, `422 invalid_chapter_join`, `404 resource_unavailable`, and `422 conversation_busy`. A missing candidate target stays `409 idempotency_binding_conflict`.
- The `apply` step writes the Project Agent, the conversation, the memory settings, and the AgentRun with status `queued`. It also writes the Context Assembly rows at decision position 0. The captured grant, Model Use Binding revision, and memory settings revision do not change.
- The route uses the generic project command admission. A new body validation option keeps the problem order of `main`. The revision, the correlation identity, the challenge headers, and the challenge secret come first. The conversation and Working Target identities come after them. The message limit of 8000 characters is a route check after the Project store check, with `409 idempotency_binding_conflict`, as on `main`.
- Replay decodes the applied effect from the Activity record and requires the AgentRun row of the Receipt.
- These observable changes follow from the sequence:
  - A failed rollback after a failed first use or a refusal gives `503 project_store_unavailable`. Before, it was ignored.
  - An applied Receipt without its Activity record or its AgentRun row gives `503 project_store_unavailable`. Before, it gave `409 idempotency_binding_conflict`. The receipt relation trigger already prevents a missing Activity record.
  - An Activity identity field that is absent, null, not a string, or not UUID text gives `503 project_store_unavailable`. An Activity `run_id` that is not the `run_id` of the AgentRun row of the Receipt gives the same store fault. Before, replay read the identities from the AgentRun row and ignored the Activity fields. The Activity payload checks already prevent a field that is absent, null, or not a string.
  - Replay returns the conversation identity text of the first acknowledgement. Before, it returned the canonical UUID text of the stored row. The two texts are different when the client sends a UUID in another form, for example in uppercase.
  - A stored Receipt of another result kind gives `409 idempotency_binding_conflict`. Before, replay ignored the result kind.
  - A Project lifecycle value other than `active` or `archived` gives a store fault. Before, the command used the value as `active`. The Project lifecycle check already prevents such a value.
  - A serialization failure, a unique violation, or a deadlock in an AgentRun read or in a Worker Context Assembly is now a store fault. Before, the read error was the busy conversation error. A read-only Repeatable Read transaction cannot have such a failure, and the Worker reported both errors as a store fault.

#### Admit step

- A command of the admit step declares its kind, isolation level, missing-Admission error, rate-limited Challenge error, and one work query. The work query takes the owner, Project, and Admission identities and gives one JSON object.
- The step runs these steps in this order in one transaction:
  1. Begin at the declared isolation level and set Project Scope.
  2. Consume the Command Challenge.
  3. Lock the Project row.
  4. Load the command facts.
  5. Insert the Admission.
  6. Write the work rows.
  7. Record the response record on the fence.
  8. Commit. The fence stays `in_progress`.
- The step writes no Domain Receipt and no Activity record.
- The fact load can refuse before Admission. The command declares its Admission form for its facts, so `applyAuthorEdit` can declare its own form (ADR 0044).
- An exact retry in progress calls the admitted replay of the command. The default replay reads the fence, the Admission, and the work query in one read-only transaction. A fence that is not `in_progress` with the command digest is a binding conflict. A fence without its Admission, or an Admission without its work rows, is damaged evidence. A pre-capture response record gives `historical_acknowledgement_unavailable`. A command can replace this replay, for example `applyAuthorEdit` (ADR 0044).
- An exact retry of a settled command reads the one replay query and the work query by the Admission of the Receipt. The command accepts only the result kinds and reasons that its later settlement writes. Another result kind or reason is a binding conflict. Then the work and the response record are decoded. Damaged evidence is a store fault.
- Every path returns the first admission: the command and Admission identities, the admitted work, and the response record. It has no Receipt identity, because the admit step writes no Receipt.

#### `exportHumanReadableManuscript`

- `exportHumanReadableManuscript` uses the admit step with the explicit project command Admission form and the Command-response Project. A rate-limited Challenge gives a store fault, as on `main`. The work rows are the export operation row and the Pinned Export Source row. Their columns do not change. The Worker settlement does not change.
- An archived Project is a refusal before Admission. It gives `422 archived_project`, as on `main`.
- The route uses the generic project command admission with the problem order of `main`. The problem texts use the name "human-readable export", as on `main`. The `202` response body does not change.
- The application binding self-check and the Store trait of the command are removed.
- These observable changes follow from the sequence:
  - A failed rollback after a failed first use or a refusal gives `503 project_store_unavailable`. Before, it was ignored.
  - An exact retry in progress whose operation row or pinned Snapshot is missing gives `503 project_store_unavailable`. Before, it gave `409 idempotency_binding_conflict`. An in-progress fence without its Admission also gives the store fault. Foreign keys already prevent a missing Admission or Snapshot.
  - A settled Receipt whose operation row or pinned Snapshot is missing gives `503 project_store_unavailable`. Before, a refused Receipt gave `409 idempotency_binding_conflict` for such a record.
  - An applied Receipt with its readable export row but without its operation row gives `503 project_store_unavailable`. Before, replay read the export identity and the pinned Snapshot from the export row and returned the admitted operation. The Worker never deletes an operation row, so only a manual change makes such a record.
  - A settled retry reads the pinned Snapshot of the operation row. Before, an applied Receipt read the pinned Snapshot of the export row. The Worker writes the same Snapshot in the two rows.
  - A settled Receipt with a result kind other than `authoritative_applied` or `refused`, or with a refusal reason other than `archived_project` or `pinned_export_source_unavailable`, gives `409 idempotency_binding_conflict`. Before, each `refused` reason replayed as admitted. The Receipt shape check already prevents such a record.
  - A Receipt field with the wrong JSON type is damaged evidence and gives a store fault. A numeric work field that is not unsigned decimal text is also damaged evidence.

#### `steerAgentRun`

- `steerAgentRun` uses the `agent_run_control` Admission form, the `ActivityOnly` profile with the Activity kind `agent_run_steering_retained`, and the Command-response Project. It locks the Project row and then the AgentRun row. It does not refuse an archived Project.
- The Core classifier returns a Core Transition Outcome with no applied outcome. A Run that is not terminal gives `no_effect` with the reason `steering_retained`. A terminal Run gives `conflicted` with the reason `terminal_run`.
- A missing AgentRun and a conversation that is not the conversation of the Run are refusals before Admission. Each gives `404 resource_unavailable`, as on `main`. An input that makes the effective author input exceed the Context item bound is a refusal before Admission. It gives `413 steering_input_limit`, as on `main`.
- The retained outcome declares `EffectWithActivity`. It resumes a paused Run without a change to its fence token, and it writes the `agent_run_steering_retained` Activity record at the next input position. The payload keys and their string types do not change, because the Worker reads them. The `terminal_run` outcome writes no Activity record, as on `main`.
- The route uses the generic project command admission. It checks the conversation identity before the session. It checks the message limit of 8000 characters after the session, header, and Command Challenge secret checks. Thus the problem order of `main` does not change. A message that is empty or above the limit gives `409 idempotency_binding_conflict`, as on `main`.
- These observable changes follow from the sequence:
  - A failed rollback after a failed first use or a refusal gives `503 project_store_unavailable`. Before, it was ignored.
  - Damaged replay evidence gives `503 project_store_unavailable` instead of `409 idempotency_binding_conflict`. Such evidence is a damaged Command-response Project record or a `steering_retained` Receipt without its Activity record. It is also an Activity `run_id`, `steering_input_id`, or `input_position` value that is absent, null, or not a string. The receipt relation trigger and the Activity payload checks already prevent the Activity faults.
  - An Activity `run_id` or `steering_input_id` that is not UUID text gives `503 project_store_unavailable`. Before, replay returned that text. The Activity payload checks already require an `input_position` of unsigned decimal text without leading zeros.
  - A zero-authority Receipt with an Author Action gives a store fault. Before, replay ignored the Author Action. The receipt relation trigger already prevents such a record.
- The behavior-equivalence review against `main` found no other difference.

#### `exportProjectArchive`

- `exportProjectArchive` uses the admit step with the explicit project command Admission form and the Command-response Project. A rate-limited Challenge gives a store fault, as on `main`. The work rows are the export operation row with its two archive profiles and the Pinned Export Source row. Their columns do not change. The Worker settlement does not change.
- The fact load reads the latest canonical Snapshot and collects the exportable families before the Admission. An archived Project gives `422 archived_project`, and each archive build refusal gives its `422` problem code, as on `main`. Each is a refusal before Admission.
- The Pinned Export Source keeps the Admission row and the operation row of the export, as on `main`. The command reads these two families again after it writes the two rows.
- The route uses the generic project command admission with the problem order of `main`. The route refuses an archive profile or an archive path profile that is not the current profile with `400 invalid_request`, as on `main`. The problem texts use the name "Project Export Archive", as on `main`. The `202` response body does not change.
- The application binding self-check, the Store trait, and the error type of the command are removed. The archive build steps use an adapter error that only the adapter sees.
- These observable changes follow from the sequence:
  - A failed rollback after a failed first use or a refusal gives `503 project_store_unavailable`. Before, it was ignored.
  - An archive build refusal comes before a store fault of the Admission insert or the operation insert. Before, the store fault came first. After the Challenge consumption, these two inserts fail only on a store fault.
  - An exact retry in progress whose operation row or pinned Snapshot is missing gives `503 project_store_unavailable`. Before, it gave `409 idempotency_binding_conflict`. An in-progress fence without its Admission also gives the store fault. Foreign keys already prevent a missing Admission or Snapshot.
  - A settled Receipt whose operation row or pinned Snapshot is missing gives `503 project_store_unavailable`. Before, a refused Receipt, or an applied Receipt without its export manifest row, gave `409 idempotency_binding_conflict` for such a record.
  - An applied Receipt with its export manifest row but without its operation row gives `503 project_store_unavailable`. Before, replay read the export identity and the pinned Snapshot from the manifest row and returned the admitted operation. The Worker never deletes an operation row, so only a manual change makes such a record.
  - A settled retry reads the pinned Snapshot of the operation row. Before, an applied Receipt read the pinned Snapshot of the manifest row. The Worker writes the same Snapshot in the two rows.
  - Replay reads the two archive profiles from the operation row. Before, replay returned the current profiles. The checks of the operation row allow only the current profiles, so this change is not observable.
  - A settled Receipt with a result kind other than `authoritative_applied` or `refused`, or with a refusal reason other than `archived_project` or `pinned_export_source_unavailable`, gives `409 idempotency_binding_conflict`. Before, each `refused` reason replayed as admitted. The Receipt shape check already prevents such a record.
  - A Receipt field with the wrong JSON type is damaged evidence and gives a store fault. A numeric work field that is not unsigned decimal text is also damaged evidence.

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
