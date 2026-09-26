import { getEditorSession, getProposal } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DraftPayloadPosition, DraftRetry, RefusedEditDraftInspect, ReplacementBlock }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorWorkspace } from "./editor-types.ts";
import { canonicalDraftValue as canonical } from "./refused-edit-discard.ts";
import { installAuthoritativeBaseSnapshot } from "./undo-latest-author-action.ts";
import { persistCandidateSelection, persistDraftRetryUnit, rebuildPendingProjection } from "./local-edit-journal.ts";
import { submitOnePendingAuthorEdit } from "./author-edit-submission.ts";

export function selectedDraftBlocks(draft: RefusedEditDraftInspect, from: DraftPayloadPosition,
  to: DraftPayloadPosition): ReplacementBlock[] {
  const units = draft.payload.author_edit_units, primitives = units[0]?.normalized_primitives;
  const primitive = primitives?.[0];
  if (units.length !== 1 || primitives?.length !== 1 || primitive?.kind !== "replace_structured_selection"
    || [from.block_index, from.offset, to.block_index, to.offset].some((value) => !Number.isInteger(value) || value < 0)
    || from.block_index > to.block_index || to.block_index >= primitive.replacement.length
    || from.block_index === to.block_index && from.offset > to.offset) throw new Error("Select a valid Draft range");
  return primitive.replacement.slice(from.block_index, to.block_index + 1).map((block, index, blocks) => {
    const start = index === 0 ? from.offset : 0, end = index === blocks.length - 1 ? to.offset : block.text.length;
    const boundary = (offset: number) => offset <= block.text.length && !(offset > 0 && offset < block.text.length
      && /[\uD800-\uDBFF]/.test(block.text[offset - 1]!) && /[\uDC00-\uDFFF]/.test(block.text[offset]!));
    if (!boundary(start) || !boundary(end)) throw new Error("Select a complete Unicode character");
    return { block_kind: block.block_kind, text: block.text.slice(start, end) };
  });
}

export type RetryTargetRead = { text: string; authoritativeHead: string; proposalHeads: string[];
  proposal?: Awaited<ReturnType<typeof getProposal>>["proposal"] };

export async function readRetryTarget(workspace: EditorWorkspace, draft: RefusedEditDraftInspect, target: string,
  baseUrl: string, fetchImpl: typeof fetch): Promise<RetryTargetRead> {
  const session = await getEditorSession({ baseUrl, fetchImpl, projectId: workspace.partition.project_scope.project_id,
    editorSessionId: workspace.partition.editor_session_id });
  if (canonical(session.project_scope) !== canonical(workspace.partition.project_scope)
    || canonical(session.editor_session) !== canonical(workspace.session.editor_session)
    || session.base_snapshot.chapter_id !== draft.payload.chapter_id || session.writer.kind !== "current_writer"
    || session.writer.writer_generation !== workspace.partition.writer_generation) throw new Error("Retry target unavailable");
  const result: RetryTargetRead = { text: "", authoritativeHead: session.base_snapshot.authoritative_head_revision_id,
    proposalHeads: session.base_snapshot.proposal_head_revision_ids };
  if (target === "original") return result;
  const source = draft.payload.author_edit_units[0]?.selection_snapshot?.ordered_selection?.sources[Number(target)];
  if (source?.owner.kind === "proposal") {
    const read = await getProposal({ baseUrl, fetchImpl, projectId: session.project_scope.project_id, proposalId: source.owner.proposal_id });
    const proposal = read.proposal;
    if (canonical(read.project_scope) !== canonical(session.project_scope) || proposal.proposal_id !== source.owner.proposal_id
      || proposal.chapter_id !== draft.payload.chapter_id || proposal.closure !== "open"
      || proposal.manuscript_block_id !== source.owner.manuscript_block_id || !result.proposalHeads.includes(proposal.revision_id)) {
      throw new Error("Retry target unavailable");
    }
    return { ...result, text: proposal.candidate_text, proposal };
  }
  const block = source?.owner.kind === "manuscript" ? session.base_snapshot.materialized_revision.blocks
    .find((block) => block.manuscript_block_id === source.owner.manuscript_block_id) : undefined;
  if (!block) throw new Error("Retry target unavailable");
  return { ...result, text: block.text };
}

export async function retryRefusedEdit({ workspace, draft, from, to, target, targetFrom, targetTo,
  baseUrl, fetchImpl, isCurrent, targetRead }: { workspace: EditorWorkspace; draft: RefusedEditDraftInspect;
  from: DraftPayloadPosition; to: DraftPayloadPosition; target: string; targetFrom: number; targetTo: number;
  baseUrl: string; fetchImpl: typeof fetch; isCurrent: () => boolean; targetRead: RetryTargetRead }) {
  const pending = await rebuildPendingProjection(workspace);
  const session = await getEditorSession({ baseUrl, fetchImpl, projectId: workspace.partition.project_scope.project_id,
    editorSessionId: workspace.partition.editor_session_id });
  if (!isCurrent() || pending.save_state !== "saved" || pending.unsettled_intent_count !== 0
    || workspace.partition.disposition !== "current_writer_open" || session.writer.kind !== "current_writer"
    || session.writer.writer_generation !== workspace.partition.writer_generation
    || canonical(session.project_scope) !== canonical(workspace.partition.project_scope)
    || canonical(session.editor_session) !== canonical(workspace.session.editor_session)
    || session.base_snapshot.chapter_id !== draft.payload.chapter_id || draft.closure !== "open"
    || draft.retention_state !== "retained"
    || session.base_snapshot.authoritative_head_revision_id !== targetRead.authoritativeHead
    || canonical(session.base_snapshot.proposal_head_revision_ids) !== canonical(targetRead.proposalHeads)) throw new Error("Draft retry requires the current writer and source");
  const replacement = selectedDraftBlocks(draft, from, to);
  const digest = await workspace.cryptoImpl.subtle.digest("SHA-256", new TextEncoder().encode(canonical(replacement)));
  const retrySource: DraftRetry = { kind: "draft_retry", source_draft_kind: "refused_edit", source_draft_id: draft.draft_id,
    source_current_draft_revision_id: draft.draft_revision_id, source_draft_payload_digest: draft.payload_digest,
    expected_source_draft_closure: "open", selected_payload_range: { kind: "exact_structured_range",
      coordinate_profile: "storyos.draft-replacement.block-utf16.v1", from, to,
      slice_digest: [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("") } };
  const original = draft.payload.author_edit_units[0]!;
  const source = original.selection_snapshot?.ordered_selection?.sources[Number(target)];
  if (target !== "original" && (replacement.length !== 1 || replacement[0]?.block_kind !== "paragraph"
    || !source || !Number.isInteger(targetFrom) || !Number.isInteger(targetTo) || targetFrom < 0 || targetTo < targetFrom)) {
    throw new Error("Select a supported target and complete paragraph range");
  }
  const currentTarget = await readRetryTarget(workspace, draft, target, baseUrl, fetchImpl);
  if (canonical(currentTarget) !== canonical(targetRead)) throw new Error("Retry target changed");
  const candidate = targetRead.proposal;
  if (!isCurrent()) throw new Error("Retry view changed");
  await installAuthoritativeBaseSnapshot(workspace, session.base_snapshot);
  workspace.session = session;
  if (candidate) {
    const proposal = candidate, text = replacement[0]!.text;
    await persistCandidateSelection(workspace, { kind: "candidate_selection", retrySource,
      target: { proposal_id: proposal.proposal_id, operation_id: proposal.operation_id,
        revision_id: proposal.revision_id, manuscript_block_id: proposal.manuscript_block_id },
      expectedProposalHeads: session.base_snapshot.proposal_head_revision_ids, priorText: proposal.candidate_text,
      from: targetFrom, to: targetTo, text, resultingBody: proposal.candidate_text.slice(0, targetFrom) + text
        + proposal.candidate_text.slice(targetTo), inputOrigin: "selection_replacement" }, workspace.cryptoImpl);
  } else if (target === "original") {
    await persistDraftRetryUnit(workspace, { ...original,
      normalized_primitives: [{ kind: "replace_structured_selection", replacement }] }, retrySource);
  } else {
    if (source?.owner.kind !== "manuscript") throw new Error("Retry target unavailable");
    const blocks = session.base_snapshot.materialized_revision.blocks;
    const primitive = blocks.length === 1 && blocks[0]?.block_kind === "paragraph"
      ? { kind: "replace_selection" as const, from: targetFrom, to: targetTo, text: replacement[0]!.text }
      : { kind: "replace_block_selection" as const, manuscript_block_id: source.owner.manuscript_block_id,
        from: targetFrom, to: targetTo, text: replacement[0]!.text };
    await persistDraftRetryUnit(workspace, { normalized_primitives: [primitive], selection_snapshot: {
      coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: targetFrom, to: targetTo } }, retrySource);
  }
  const guardedFetch: typeof fetch = (input, init) => {
    if (!isCurrent()) throw new Error("Retry view changed");
    return fetchImpl(input, init);
  };
  return submitOnePendingAuthorEdit({ workspace, baseUrl, fetchImpl: guardedFetch, cryptoImpl: workspace.cryptoImpl });
}
