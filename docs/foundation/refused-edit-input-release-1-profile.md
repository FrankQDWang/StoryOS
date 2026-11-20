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

The Release 1 baseline has no Draft table
([ADR 0079](../adr/0079-squash-the-migrations-into-one-baseline-and-derive-the-storage-identity.md)).
The command kind and result kind domains refuse a Domain Receipt of a Draft
command or result. The Project archive format has no Draft table family.

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
- A stored record or group of the removed Draft path is not a shape of the
  current contract
  ([ADR 0099](../adr/0099-remove-the-compatibility-with-earlier-local-edit-journal-versions.md)).
  It fails current validation and enters the `local_journal_unavailable`
  read-only recovery state, and the Journal never sends it.
