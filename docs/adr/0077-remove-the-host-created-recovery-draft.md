---
status: accepted
---

# Remove the Host-Created Recovery Draft

On 2026-10-31, the author decided not to build [Deliver Host-created Recovery Drafts for admitted author edits](https://github.com/FrankQDWang/StoryOS/issues/881). StoryOS removes the placeholders of that capability instead.

## Context

The contract specified a Host-created Recovery Draft. When an admitted `ApplyAuthorEdit` settled as `RequiresReconfirmation`, the Host could create a Draft Core Artifact from the retained command body, with an Editor Recovery Creator and an evidence record. The outcome Query returned its identity in `recovery_draft_ref`.

No code creates such a Draft. The admission recovery always writes `recovery_draft_ref` as `None`, and the column of `author_command_admission_reconfirmations` is always NULL. The ingress is rare: only an admitted Author Edit that settles as `RequiresReconfirmation` reaches it. A complete delivery needs a new Artifact family with retention, Archive, export, restore, and a Web display. [ADR 0073](0073-remove-the-refused-edit-draft-and-the-inline-proposal.md) removed the Refused Edit Draft for the same reason: no production path created it.

No production deployment exists ([ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md)).

## Decision

- **No Host-created Recovery Draft.** StoryOS has no Recovery Draft, no Editor Recovery Creator, no `EditorRecovery` Artifact Creator, and no Editor Recovery evidence record. The manuscript editor recovery flow creates no Draft Artifact.
- **Outcome contract.** The `requires_reconfirmation` branch of the outcome Query has no `recovery_draft_ref` field. The branch keeps `command_id`, `author_command_admission_id`, and `reconfirmation_reason`. The closed schema refuses the removed field.
- **Stored rows.** Migration `0102_remove_recovery_draft_reference.sql` drops the column `recovery_draft_ref` from `author_command_admission_reconfirmations`. A stored non-NULL reference stops the migration, and the row stays. The Project archive packs the rows of the table without the column.
- **Browser-local recovery stays.** `RequiresReconfirmation` still tells the author to confirm again. The unsent author text stays in the Local Edit Journal, and the local recovery panel shows it. The author can copy it into a new edit with a new Admission.
- **Old client.** A Web Client from before this change reads a `requires_reconfirmation` outcome with no `recovery_draft_ref` as an invalid envelope. It keeps the Submission Group unsettled and its local text. The release package ships the Web Client and the Server together.

## Superseded parts

This record supersedes these parts of earlier records:

- [ADR 0001](0001-separate-authoritative-state-artifacts-and-operational-records.md): the Recovery Draft in the list of non-authoritative Draft Artifacts.
- [ADR 0003](0003-specify-manuscript-revision-and-proposal-state-machine.md): the recovery Draft that author input may require in a crash window, and the Recovery Draft of the state machine contract.
- [ADR 0073](0073-remove-the-refused-edit-draft-and-the-inline-proposal.md): the Recovery Draft as the only Draft Artifact of the editor recovery flow, and its copy control.
- Section 9 of the [Web Editor Session contract](../foundation/web-editor-session-synchronization-and-recovery-semantics.md): the Host ingress, the evidence record, and the ingress matrix of the Recovery Draft.
- The Recovery Draft parts of the [Artifact domain model](../foundation/artifact-domain-model.md), the [state machine](../foundation/manuscript-revision-proposal-state-machine.md), [Author Command Admission](../foundation/author-command-admission.md), the [protocol](../foundation/versioned-command-query-artifact-event-protocol.md), the [threat model](../foundation/storyos-service-client-external-trust-boundaries-threat-model.md), and the [verification gates](../foundation/deterministic-verification-and-failure-recovery-gates.md).

The rejected option of [ADR 0044](0044-settle-author-edit-acceptance-and-author-undo-through-the-command-sequence.md) that names issue #881 stays as history.

## Considered options

- Build the Host-created Recovery Draft. Rejected: it needs a new Artifact family for a rare ingress, and the browser-local recovery already keeps the text.
- Keep the field and the column as an always-empty placeholder. Rejected: a reader must then learn a capability that does not exist.
- Keep the column and add only `CHECK (recovery_draft_ref IS NULL)`. Rejected: no trigger or function uses the column, so a drop has no risk, and it removes the placeholder from the stored format.

## Consequences

- The contract, application, Server, and PostgreSQL adapter types of `RequiresReconfirmation` have no draft reference. The generated JSON Schema, TypeScript client, and route catalog change with them.
- The Web Client types and the outcome Query parser have no `recovery_draft_ref`. A stored Local Edit Journal settlement that still holds `recovery_draft_ref: null` stays readable, because the Journal does not check the field.
- The glossary has no Editor Recovery Creator. The Recovery Draft term stays only to tell that StoryOS does not have it.
