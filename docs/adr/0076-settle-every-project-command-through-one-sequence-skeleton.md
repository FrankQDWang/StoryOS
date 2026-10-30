---
status: accepted
---

# Settle Every Project Command Through One Sequence Skeleton

On 2026-10-30 the author accepted this decision for phase 4 of the repository cleanup ([#1317](https://github.com/FrankQDWang/StoryOS/issues/1317)). Ticket [#1318](https://github.com/FrankQDWang/StoryOS/issues/1318) adds it. It supersedes [ADR 0043](0043-settle-every-project-command-through-declared-profiles-of-one-sequence.md) and [ADR 0044](0044-settle-author-edit-acceptance-and-author-undo-through-the-command-sequence.md). Their text stays as history: the behavior-equivalence records of the finished migration and the changes of each moved command. It amends [ADR 0075](0075-prove-each-behavior-at-one-behavior-owner.md). [ADR 0041](0041-own-the-project-command-sequence-in-one-adapter-module.md) stays in force.

## Context

ADR 0043 and ADR 0044 have approximately 10,000 words, and they mix the current rules with the records of a finished migration. Three entry points copied the transaction start, the Command Challenge use, the exact-retry branch, the commit, and the span records. The `ProjectCommand` trait had parts with one value or no user, and hooks with one user.

## Decision

### Scope

- A project command consumes one Project Command Challenge and settles one Author Command Admission. Every project command settles through `command_sequence` in `storyos-adapter-postgres`. The author decided to keep the `ProjectCommand` trait and one settlement order, and not to rewrite the commands into plain functions.
- These operations are not project commands, and they stay outside the sequence: `createEditorSession`, the current-producer form of `withdrawProposal`, and the Worker settlement of an admitted export.

### The sequence skeleton

- One function, `consume_challenge`, owns the transaction steps. It begins the Serializable transaction and sets Project Scope. It consumes the Command Challenge and runs the first use of the entry point. It commits a first use that succeeded, and it rolls back every other path. It maps a contended first use to the declared contention refusal, and it records `replayed` on the command span for an exact retry of a settled command. A failed rollback is a store fault.
- An entry point gives its first use and answers the two exact-retry branches. The three entry points are the settle step of `ProjectCommand`, the admit step of the two exports, and the admit and settle steps of Author Edit.
- Each entry function declares its own span with `#[tracing::instrument]` at its definition. [CODING_STANDARDS.md](../../CODING_STANDARDS.md) requires this form for a `command_sequence` entry and forbids `.instrument(...)` at a call site. The span name is static, and [ADR 0047](0047-write-server-and-worker-diagnostic-projections-as-json-lines-to-stderr.md) and the diagnostic canary tests use it. Thus the skeleton cannot own the span declaration. It owns the span records: `consume_challenge` records `replayed`, and one function, `record_result`, records the identifiers or the reason code of the result.

### Settlement order

The first use of a project command runs these steps in this order:

1. Lock the Project row. A missing Project is refused before Admission.
2. Load the command facts and classify through Core. The fact load can refuse before Admission with a typed reason of the command (`RefusableCommandError`).
3. Insert the Admission. An insert that inserts no row gives `invalid_challenge`, or the diagnosis that the command declares.
4. For an applied outcome: allocate the profile sequences, insert the Domain Receipt and the settlement link, write the command effect rows, write the profile records, and then write the effect rows that refer to the profile records. For a zero-authority outcome: insert the Domain Receipt, and write the effect rows and the Activity record that the outcome declares.
5. Write the response record and settle the Command Idempotency Fence.

A refusal before Admission and a failure of a step roll back the transaction. No row stays, and the Command Challenge stays unused. A command can declare a contention refusal for a serialization failure, a unique violation, or a deadlock of its first use, including the commit. Only `createAgentRun` declares one: `conversation_busy`.

### Replay rule

- One replay query reads the settled evidence in a read-only REPEATABLE READ transaction. A command can add one effect query that takes the Receipt identity.
- Replay checks the binding of the command first, then the authority evidence of the profile, and then decodes the command effect. Thus a binding conflict comes before damaged evidence, and a decode never hides damaged authority evidence.
- A missing settled record or an unknown reason is a binding conflict. A pre-capture record gives `historical_acknowledgement_unavailable` ([ADR 0032](0032-accept-historical-acknowledgement-unavailable.md)), only for a command that has a response record. Damaged evidence is a store fault. A zero-authority Receipt with an Author Action is damaged evidence.
- An exact retry in progress is a binding conflict, except in the admit step of an export.
- Only a Command Challenge issue applies the Challenge rate limit. The use, commit, and rollback of a Challenge never give `RateLimited`, so a command declares no rate-limited error, and the sequence maps such an error as a store fault.

### Admission forms

- **Project.** No Editor Session. The action class is `explicit_project_command`, `agent_run_start`, or `agent_run_control`. The three classes write the same columns.
- **Explicit editor command.** The insert requires the current writer generation of the Editor Session. It records the Chapter, the expected Authoritative Revision, and the target references.
- **Writer takeover.** The insert requires the observed writer generation of another Editor Session.
- **Direct editor action.** Author Edit. The insert requires the client writer generation, the expected Revision and head of the Chapter, and an active Project.

### Settlement profiles

A profile fixes the scope sequences and the authority records of an applied outcome. The applied variant of the command gives the Receipt result kind and the Author Action disposition: none, `Forward` with its Forward command kind, or `Compensation`.

| Profile | Records of an applied outcome | Commands |
| --- | --- | --- |
| `Structural` | The guarded Project update, the Activity record with `tree_revision`, the Authoritative Commit, the Forward Author Action, the canonical Snapshot, and the rebind of the current writer base when the command asks for it | The six structure commands |
| `ChapterSelection` | The guarded Project update, the Activity record with `base_snapshot_id`, a Forward Author Action without an Authoritative Commit, the canonical Snapshot, and the required rebind of the writer base of the Editor Session | `setCurrentChapter` |
| `ActivityOnly` | One Activity record | `updateProject`, `archiveProject`, `updateProjectAssistance`, `takeOverProjectWriter`, and the four AgentRun commands |
| `ActionOnly` | One Forward Author Action | The seven Proposal decisions |
| `AuthoritativeRevision` | The Revision with its payload, members, and Envelope, the guarded Head update, the Authoritative Commit, the Author Action, the `project_activity_events` row, the writer-base update, and the canonical Snapshot | `acceptProposal`, the applied Author Edit, and the prose and Acceptance compensations |

- `Structural` and `ChapterSelection` write their records through one shared step, in the order of the table. `Structural` and `AuthoritativeRevision` share one allocation of the Author Action, Authoritative Commit, and Activity counters. The compensation adapters use the same Author Action insert and writer-base update.
- `persist` takes the envelope and only the other settlement facts that the profile reads: the applied variant (`ActionOnly`), the Activity kind (`ActivityOnly`), or the locked Project facts and the Activity kind (`Structural`, `ChapterSelection`).
- A profile of one record set allocates its sequences for every plan. Only `AuthorEditProfile` and `UndoCompensation` have more than one record set, and their plan selects the set. Each profile keeps its own replay evidence check.
- A zero-authority outcome never allocates an Authoritative Commit or an Author Action. It can write effect rows, and an Activity record where its schema requires one (`steerAgentRun`, `takeOverProjectWriter`).
- The response record is the Command-response Project, the Command-response Project with the Project assistance record, or none. A command without a response record decodes its acknowledgement from the Receipt and its effect rows.

### Trait shape

- The trait keeps only the parts that more than one command needs. A step of one command is in the code of that command when the step order stays the same, for example the zero-authority Acceptance Receipt of `acceptProposal`.
- A hook stays when its step runs between two sequence steps that the command does not own, for example `apply_after_authority` after the profile records. The contention refusal is a declared value of the command.

### Export admit step

- The first use locks the Project row, loads the facts, inserts the Admission, writes the work rows, and records the response record on the fence. It commits, and the fence stays `in_progress`. It writes no Domain Receipt and no Activity record. The Worker settles the fence later.
- An exact retry in progress reads the fence, the Admission, and the work rows in one read-only transaction, and replays the admitted work. A fence that is not `in_progress` with the command digest is a binding conflict. A missing Admission or work row is damaged evidence.
- An exact retry of a settled command reads the replay query and the work rows of the Admission of the Receipt. The command accepts only the result kinds and reasons that its settlement writes. Another value is a binding conflict.
- Every path returns the first admission: the command and Admission identities, the work, and the response record.

### Author Edit admitted-command step

- The admit step reads the Project row without a lock, inserts the Admission, and commits. The fence stays `in_progress`. The transaction that pauses Proposal generation runs before it.
- The settle step runs in a new Serializable transaction. It reads the Project row without a lock and requires an Admission that has no settlement, is not expired, and has the current writer. Otherwise it gives `admission_expired`, `stale_writer`, or a binding conflict. A settled Admission replays its Receipt. When a concurrent settle step settles the same Admission first, the step replays that Receipt.
- `applyAuthorEdit` runs both steps in one request. The outcome query runs the settle step for an Admission without settlement, or settles it as `RequiresReconfirmation`. An exact retry of a `RequiresReconfirmation` fence is a binding conflict.
- An Admission without a terminal settlement is `pending`. When the outcome query cannot settle it yet, it reports `outcome_unknown`. No row records this report, and the table `author_command_admission_outcome_unknown_observations` has no writer.

### Acceptance refusal before Admission

The diagnosis of `acceptProposal` gives `stale_writer`, `session_changed`, or `invalid_challenge`. A retained refusal for the same idempotency key is a refusal before Admission. After the rollback, the `acceptProposal` method retains the Pre-Admission Refusal Record in its own Serializable transaction ([ADR 0013](0013-trust-the-storyos-web-client-for-author-command-admission.md)).

### Compensation adapters

Each Forward command family has one compensation adapter in the module of its forward command. The adapter loads the Forward evidence under lock, gives the Author Undo Frontier kind, writes the Compensation records, and decodes them on replay. Author Undo selects the adapter from the Forward command kind through one exhaustive match. A new Forward command kind does not compile until it declares its Author Undo Disposition. Replay selects the decoder from the command kind of the source Receipt.

### Author Undo Disposition

| Forward command kind | Author Undo Disposition |
| --- | --- |
| `applyAuthorEdit`, `authoritative_applied` | Compensation: restore the prior Revision ([ADR 0067](0067-prove-the-target-of-author-undo-through-the-compensation-lineage-of-the-chapter-head.md)). |
| `applyAuthorEdit`, `proposal_revised` | Compensation: restore the candidate of the edited Operation in a new Proposal Revision ([ADR 0068](0068-prove-the-target-of-a-proposal-author-undo-through-the-compensation-lineage-of-the-proposal-head.md)). |
| `acceptProposal` | Compensation when the Head is a Safe Compensation Head. Otherwise reversal required or unavailable. |
| The six structure commands | Compensation ([ADR 0030](0030-restore-prior-identities-for-structure-compensation.md)). |
| `setCurrentChapter` | Compensation when the prior Chapter is a lawful target. Otherwise Barrier. |
| `withdrawProposal`, author form | Compensation: reopen the Proposal (ADR 0068). |
| `replanProposal`, `reopenRejectedOperations`, `reopenWithdrawnProposal` | Compensation: append the Revision before the decision again, and reject the reopened operations or withdraw the Proposal again (ADR 0068). |
| `rejectProposalOperations`, `completeReadyPartialProposal`, `continueProposalGeneration` | Barrier. |
| `undoLatestAuthorAction`, `reversal_required` | Barrier. |

A Barrier stops Author Undo, and Author Undo never skips it. These product decisions of ADR 0044 stay in force:

- A Proposal edit records its edited Operation and that candidate before the edit (migration 0085). The Compensation restores that candidate to that Operation.
- `createProject`, `updateProject`, `archiveProject`, and `updateProjectAssistance` write no Author Action and are outside Author Undo Order.
- The `compensated` effect omits `authoritative_revision` and `authoritative_commit_id` when the Compensation writes no such record.

### Amendment of ADR 0075

Inside `storyos-adapter-postgres`, `command_sequence_tests/` owns the generic guarantees for each command row: replay equality, the declared records only, rollback, an exact retry in progress, damaged and pre-capture evidence, the contention refusal, and the missing Admission. A per-command test owns the classification and the effect rows of its command, its scope isolation, and its own refusals.

## Considered options

- Rewrite the commands into plain functions without the trait. The author rejected it: one trait keeps one settlement order.
- A skeleton that also answers the replay branches through a trait of each entry point. Rejected: the replay of each entry point is different, and the trait implementations were longer than the branches.
- Amend ADR 0043 and ADR 0044 again. Rejected: a reader must still read 10,000 words.

## Consequences

- A fix to the transaction start, the Challenge use, the commit, the contention mapping, or the span records is in one place.
- A new project command needs a Core classifier, one `ProjectCommand` implementation that declares its profile, one Server route, and one contracts operation. A record set that no profile fits needs a decision.
- A new Forward command kind declares its Author Undo Disposition in `ForwardCommand::disposition` and in the table of this ADR.
