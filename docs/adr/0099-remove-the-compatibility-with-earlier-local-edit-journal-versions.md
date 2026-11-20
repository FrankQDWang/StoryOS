---
status: accepted
---

# Remove the Compatibility With Earlier Local Edit Journal Versions

The author decided this on 2026-11-20, during the cleanup of the Stage 5 to Stage 7 code ([#1715](https://github.com/FrankQDWang/StoryOS/issues/1715)). Ticket [#1724](https://github.com/FrankQDWang/StoryOS/issues/1724) delivers it. This ADR amends [ADR 0036](0036-bound-active-journal-reads-with-retained-history.md), [ADR 0064](0064-accept-a-proposal-on-an-earlier-base-by-replacing-its-target-text.md), and [ADR 0073](0073-remove-the-refused-edit-draft-and-the-inline-proposal.md).

## Context

The Local Edit Journal is IndexedDB version 5. Before this decision, the Web had these paths for data that an earlier client wrote:

- The upgrade of a version 3 or version 4 Journal to version 5 (ADR 0036, ADR 0065).
- A read-only reader of a version 4 Journal that did not upgrade. It gave the local unsent text of each Chapter in a panel and in a file.
- The read rule of ADR 0073 for a stored record or Submission Group of the removed Refused Edit Draft path.
- The removal at load of a stored Replan entry of an earlier page (ADR 0064).
- The Block reconfirmation of a pending record with the primitive `replace_selection` and no Proposal target, and its button.

No production deployment exists ([ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md)). No author data is in a Journal of an earlier version, and no author data is in a stored record of a removed path. These paths only add code and tests.

## Decision

- **No upgrade.** The Web opens a version 5 Journal or creates a new one. When IndexedDB asks for an upgrade from an earlier version, the Web aborts the version-change transaction. The open then fails, and the earlier Journal stays with its version and each row.
- **Fail closed.** A Journal that does not open uses the existing path for a Journal of an unknown version. The editor enters read-only local recovery with `local_journal_unavailable`. The editor does not show the text of that Journal, and it never shows that text as saved text. The readable export of the Server text stays available.
- **No reader of a removed path.** The Journal has no read rule for a record or a group of the removed Draft path. The Journal validation accepts only the shapes of the current contract. A `draft_retry` source, a frozen request with `retry_source`, a `replace_structured_selection` primitive, or a zero-authority settlement with an effect that the contract does not have, for example `refused_to_draft`, fails closed, and the editor enters local recovery. This check only refuses. No reader converts or sends an earlier shape.
- **Retained recovery record.** A retained recovery record of another Editor Session with such a shape stays in the metadata store, and the recovery panel shows no text of it. The other retained records stay available.
- **Stored Replan entry.** The editor does not remove a stored Replan entry at load. The entry stays in the metadata store. No reader reads it, and no request sends it.
- **Authoritative `replace_selection`.** The Web writes the primitive `replace_selection` only for a Proposal candidate. An earlier client wrote it also for a Chapter edit, with no Block identity, and the Block reconfirmation converted it. Now such a record fails closed, and the editor enters local recovery. The Journal never sends it, so the Server cannot apply it to the wrong Block kind.
- **Current version.** The other reads and validations of a version 5 Journal do not change.
- **Rule for a later version.** After the first release that has author data, a change of the Journal version must have an upgrade path from each released earlier version, and a test of that path.

## Considered options

- Keep the upgrade and the read-only reader. Rejected: no Journal of an earlier version exists, and the paths cost about 1,300 lines of code and tests.
- Delete a Journal of an earlier version and create a new one. Rejected: a Journal can hold author text that the Server does not have. The fail-closed path keeps that text in the browser.

## Consequences

- The Web removes `removed-draft-path.ts`, `stored-replan-records.ts`, `local-unsent-text.ts`, the local unsent text panel, and the `Stored*` types that only these paths used.
- The tests of the removed paths go. New tests open a version 3 and a version 4 Journal and prove that the open fails, that the page shows the Server text read-only, and that the Journal stays unchanged. Other new tests prove that each stored shape of a removed path fails closed, and that a retained record with such a shape does not hide the other retained records.
- The [Web Editor Session contract](../foundation/web-editor-session-synchronization-and-recovery-semantics.md#32-immutable-intent-records) gives the record shape and the open rule.
