import type {
  JournalIntentRecord,
  JournalSubmissionGroup,
  StoredApplyAuthorEditRequest,
  StoredAuthorEditUnit,
} from "./editor-types.ts";

const isStructuredPrimitive = (primitive: { kind: string }) => primitive.kind === "replace_structured_selection";

/** A stored record of a removed Draft path (ADR 0073). The Journal never sends it, and it is resolved. */
export function isRemovedDraftRecord(record: JournalIntentRecord) {
  return record.retry_source?.kind === "draft_retry"
    || ((record.author_edit_unit as StoredAuthorEditUnit | undefined)?.normalized_primitives ?? []).some(isStructuredPrimitive);
}

/** A stored group of a removed Draft path (ADR 0073). The Journal never sends it, and it is resolved. */
export function isRemovedDraftGroup(group: JournalSubmissionGroup) {
  const request = group.frozen_request_body as StoredApplyAuthorEditRequest;
  return request.retry_source !== undefined
    || request.author_edit_units.some((unit) => unit.normalized_primitives.some(isStructuredPrimitive))
    || (group.settlement.kind === "zero_authority_receipt_settled"
      && group.settlement.effect.kind === "refused_to_draft");
}
