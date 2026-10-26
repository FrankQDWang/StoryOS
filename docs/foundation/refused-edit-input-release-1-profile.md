# Refused Edit Input Release 1 Profile

[ADR 0073](../adr/0073-remove-the-refused-edit-draft-and-the-inline-proposal.md)
removes the Refused Edit Draft from Release 1. These shapes are not in the
Release 1 contract:

- the structured-source input `replace_structured_selection` and its
  `ordered_selection`
- the Draft retry `retry_source`
- the Draft query `getRefusedEditDraft`
- the Discard `closeEditorFlowDraft`
- the expansion `expandRefusedEditDraftToProposal`

 A request with a removed field or primitive
does not match the closed request schema of `applyAuthorEdit`. The Server
answers HTTP 422 `invalid_request_shape` before Admission and writes nothing.

Migration `0099_remove_refused_edit_draft.sql` keeps the dormant Draft tables
and adds guard constraints. A stored Draft row, or a stored Domain Receipt of a
Draft command or result, stops the migration. The migration deletes no data,
and the tables stay in the Project archive format.

These rules of the earlier profile stay:

- The batch policy `storyos.author-edit-batch.release-1.preview.v1` keeps its
  legacy 250 ms, 240-unit, and 240-primitive selection. Its retained synthetic
  browser measurements and their exact
  [retained policy source](../research/author-edit-batch-prerelease-policy-source.v1.json)
  cover the legacy primitive workload only.
- The active Server requires the current Editor Contract revision. A v2 request
  cannot be silently rewritten or dispatched as v3. A v2 Local Edit Journal
  record in an active partition fails current validation and enters the
  existing `local_journal_unavailable` read-only recovery state. Validation does
  not delete or collect it, and it cannot cause automatic replay.
- The Local Edit Journal read rule of ADR 0073 applies to a stored record of a
  removed Draft path. Such a record counts as resolved, and the Journal never
  sends it.
