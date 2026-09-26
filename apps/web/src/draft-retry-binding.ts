import type { ApplyAuthorEditRequest, ApplyAuthorEditResponse, DraftRetry }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { canonicalDraftValue as canonical } from "./refused-edit-discard.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const hex = (value: unknown): value is string => typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
const exact = (value: unknown, names: string[]) => value !== null && typeof value === "object"
  && Object.keys(value).length === names.length && names.every((name) => Object.hasOwn(value, name));
const position = (value: DraftRetry["selected_payload_range"]["from"]) => exact(value, ["block_index", "offset"])
  && [value.block_index, value.offset].every((number) => Number.isInteger(number) && number >= 0 && number <= 4294967295);

export function validDraftRetry(value: unknown): value is DraftRetry {
  if (!exact(value, ["kind", "source_draft_kind", "source_draft_id", "source_current_draft_revision_id",
    "source_draft_payload_digest", "expected_source_draft_closure", "selected_payload_range"])) return false;
  const retry = value as DraftRetry, range = retry.selected_payload_range;
  return retry.kind === "draft_retry" && retry.source_draft_kind === "refused_edit"
    && UUID.test(retry.source_draft_id) && UUID.test(retry.source_current_draft_revision_id)
    && hex(retry.source_draft_payload_digest) && retry.expected_source_draft_closure === "open"
    && exact(range, ["kind", "coordinate_profile", "from", "to", "slice_digest"])
    && range.kind === "exact_structured_range" && range.coordinate_profile === "storyos.draft-replacement.block-utf16.v1"
    && position(range.from) && position(range.to) && hex(range.slice_digest)
    && (range.from.block_index < range.to.block_index
      || range.from.block_index === range.to.block_index && range.from.offset <= range.to.offset);
}

export function validRetrySettlement(request: ApplyAuthorEditRequest, response: Pick<ApplyAuthorEditResponse, "effect" | "receipt" | "source_draft_disposition">): boolean {
  const retry = request.retry_source, source = response.source_draft_disposition, effect = response.effect;
  const provenance = effect?.kind === "refused_to_draft" ? effect.replacement_provenance : undefined;
  if (retry === undefined) return source === undefined && provenance === undefined;
  if (!validDraftRetry(retry) || !source || source.source_draft_kind !== "refused_edit"
    || source.source_draft_id !== retry.source_draft_id) return false;
  if (source.kind === "unchanged") {
    const closure = source.current_closure;
    return exact(source, ["kind", "source_draft_kind", "source_draft_id", "requested_source_draft_revision_id",
      "current_source_draft_revision_id", "current_source_draft_payload_digest", "current_closure"])
      && source.requested_source_draft_revision_id === retry.source_current_draft_revision_id
      && UUID.test(source.current_source_draft_revision_id) && hex(source.current_source_draft_payload_digest)
      && (closure?.kind === "open" ? exact(closure, ["kind"])
        : closure?.kind === "closed" && exact(closure, ["kind", "close_reason", "closure_event_ref"])
          && ["abandoned", "superseded"].includes(closure.close_reason) && UUID.test(closure.closure_event_ref))
      && ["no_effect", "conflicted", "refused"].includes(effect?.kind)
      && (effect.kind === "conflicted" || closure.kind === "open"
        && source.current_source_draft_revision_id === retry.source_current_draft_revision_id
        && source.current_source_draft_payload_digest === retry.source_draft_payload_digest)
      && canonical(response.receipt.draft_artifact_refs) === "[]"
      && canonical(response.receipt.artifact_lifecycle_event_refs) === "[]" && provenance === undefined;
  }
  if (source.kind !== "closed_superseded" || !exact(source, ["kind", "source_draft_kind", "source_draft_id",
    "source_draft_revision_id", "source_draft_payload_digest", "prior_closure", "resulting_closure", "close_reason", "closure_event_ref"])
    || source.source_draft_revision_id !== retry.source_current_draft_revision_id
    || source.source_draft_payload_digest !== retry.source_draft_payload_digest || source.prior_closure !== "open"
    || source.resulting_closure !== "closed" || source.close_reason !== "superseded" || !UUID.test(source.closure_event_ref)
    || !["authoritative_applied", "proposal_revised", "refused_to_draft"].includes(effect?.kind)) return false;
  return canonical(response.receipt.draft_artifact_refs) === canonical(effect.kind === "refused_to_draft"
      ? [effect.draft_id, source.source_draft_id] : [source.source_draft_id])
    && canonical(response.receipt.artifact_lifecycle_event_refs) === canonical(effect.kind === "refused_to_draft"
      ? [effect.creation_event_id, source.closure_event_ref] : [source.closure_event_ref])
    && (effect.kind === "refused_to_draft" ? effect.refusal_origin === "draft_retry_replacement"
      && canonical(provenance) === canonical({ source_draft_id: source.source_draft_id,
        source_draft_revision_id: source.source_draft_revision_id, source_draft_payload_digest: source.source_draft_payload_digest,
        closure_event_ref: source.closure_event_ref, selected_payload_range: retry.selected_payload_range }) : provenance === undefined);
}
