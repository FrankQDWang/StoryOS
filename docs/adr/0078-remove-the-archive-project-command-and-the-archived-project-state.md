---
status: accepted
---

# Remove the Archive Project Command and the Archived Project State

On 2026-10-31, the author decided [#897](https://github.com/FrankQDWang/StoryOS/issues/897): Release 1 does not open Project archive to the author, and it removes the archive command.

## Context

The `archiveProject` command set a Project to the `archived` lifecycle state. Archive was one-way: no command set an archived Project to `active` again. The Web Client has had no archive control since commit `be9b6611`. Only the `s2-05` exact-dist journey called `apps/web/src/archive-project.ts`. A single author with few Projects has little need to hide a Project, and a one-way control that the author can press by mistake has more risk than value.

The Project Archive export (`exportProjectArchive`), the human-readable export, and their restore paths are a different feature with the same word. They stay. This record removes only their refusal of an archived Project, because that state no longer exists.

## Decision

- **Command.** Release 1 has no `archiveProject` command. Its route `PUT /api/v1/projects/{project_id}/archival` goes, with its contract, its schemas, and its Project Command Challenge target. Its Core Transition, its application input, and its PostgreSQL settlement go. Its Web client file and the `s2-05` journey go. The `archiveProject` value of `DomainReceiptCommandKind` goes.
- **Lifecycle state.** Release 1 has one Project lifecycle state: `active`. Migration `0103_remove_archived_project.sql` sets each archived Project to `active`, and the lifecycle check of `storyos.projects` then refuses each other value. A `CHECK (false) NOT VALID` constraint refuses a new row in `project_archival_decisions`. The stored decisions, the `archiveProject` Receipts, and the `project_archival_changed` Activity records stay as history. The Project Archive export keeps their families, and the `project_archival_changed` event kind stays in the Project Activity contract.
- **Readers.** The project commands, the editor session, the Manuscript Tree and search reads, the AgentRun admission, and the writer takeover no longer refuse an archived Project. The refusal reason `archived_project` goes from `createVolume`, `updateVolume`, `deleteVolume`, `createChapter`, `updateChapter`, `deleteChapter`, and `setCurrentChapter`, and the problem `422 archived_project` goes from `createAgentRun`. The `lifecycle` field goes from the Project list item, and the Web shows each listed Project as active.
- **Exports.** The export contracts lose their archived refusal, because the state no longer exists. The `Refused` effect and its refusal reason type go from the `exportProjectArchive` and `exportHumanReadableManuscript` responses, because `archived_project` was their only reason. The admissions, the export operation and human-readable export reads, the Project Export Archive ZIP download, and the Worker settlements no longer give or record `archived_project`. A Worker that finds no Project row for its claim now gives an unavailable error, not a failed settlement. The Core and the application keep only the missing Project refusal. Each other export behavior stays: the normal export, `pinned_export_source_unavailable`, and the restore paths.
- **Recovery proof.** The lifecycle range of `pass_recovery_visibility_proof` now counts each Project that is not `active`, and the proof keeps the hold when the count is not zero. Before, it compared each archived Project with its archival decision.
- **Old client and old Receipt.** A request to the removed route gets the Server answer for an unknown route. A stored Receipt of a removed refusal reason does not decode, so an exact retry of that command gives `unavailable`. An export settlement Receipt that records `archived_project` is no longer a known settlement, so an exact retry of that export gives `409 idempotency_binding_conflict`.

## Superseded parts

This record supersedes these parts of earlier records:

- [ADR 0019](0019-stage-pinned-export-source-at-admission.md): the `archived_project` settlement reason of an archived Project.
- [ADR 0038](0038-separate-author-edit-challenge-admission-from-shared-command-admission.md): `archiveProject` in the Project settings and lifecycle commands.
- [ADR 0043](0043-settle-every-project-command-through-declared-profiles-of-one-sequence.md): `archiveProject` in the Project setting commands, the archived Project refusal of the Manuscript Structure Transition commands and of `createAgentRun`, the `archived` value of the Project lifecycle, the `422 archived_project` refusal of the export admissions, and `archived_project` in the known export settlement reasons. Its other export rules stay.
- [ADR 0044](0044-settle-author-edit-acceptance-and-author-undo-through-the-command-sequence.md): `archiveProject` in the commands outside Author Undo Order.
- [ADR 0066](0066-record-the-answers-to-the-reference-model-contract-questions.md): answer A-001, and the `archived_project` refusal of answer A-003. A removed Chapter on an Empty Project still gives `invalid_chapter_join` before `empty_project`.
- [ADR 0076](0076-settle-every-project-command-through-one-sequence-skeleton.md): `archiveProject` in the `ActivityOnly` profile and in the commands outside Author Undo Order.

## Considered options

- Add an archive control in the library and an unarchive command. Rejected by the author: a single author has little need to hide Projects, and it is a new command and a contract change.
- Remove the command and keep the archived state and each reader check. Rejected: no command or row can make the state, so each check is unreachable code with no test.
- Keep the archived refusal of the exports and their contracts. Rejected by the author: the state no longer exists, so a public refusal that no author can get is surface with no use.
- Delete the archival decisions and their Receipts. Rejected: they are history, the Receipt relation triggers require them, and the Project Archive export packs them.
- Drop the `lifecycle_state` column. Rejected: the history and recovery functions read it, so a drop must emit those functions again for no product effect.

## Consequences

- The Smoke Check, the route catalog, the operation registry, and the persistence catalog no longer name `archiveProject`.
- The recovery drill no longer archives its export Projects. It still proves that the completed human-readable export and the completed Project Export Archive survive a physical recovery.
- No Rust type models the Project lifecycle. Only the database column and its check keep the `active` value.
