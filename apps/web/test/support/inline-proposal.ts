import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { acceptProposal, applyAuthorEdit, createAgentRun, createEditorSession, digestAcceptProposal,
  digestApplyAuthorEdit, digestCreateAgentRun, digestCreateEditorSession, getAgentRun, getProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, ApplyAuthorEditRequest, BlockProposalInspect, CreateAgentRunRequest,
  CreateEditorSessionRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, PROSE, challenged, id, settleOnce } from "./acceptance.ts";

export const INLINE_CANDIDATE = "narrator tone";
export const SOURCE_SLICE = "narrator voice";
const GOLDEN_DIGEST =
  "sha256:bc395ed925fa201c3c0ecd550ed362f89e6069001d6c23af2ce214b107d9ce40";

export function sliceDigest(manuscriptBlockId: string): string {
  return `sha256:${createHash("sha256").update(JSON.stringify({
    base_slice: SOURCE_SLICE,
    block_kind: "paragraph",
    coordinate_profile: "prosemirror-token-utf16.v1",
    from: 10,
    manuscript_block_id: manuscriptBlockId,
    manuscript_schema_version: 1,
    to: 24,
  })).digest("hex")}`;
}

export function phraseRequest(chapterId: string, correlationId: string): CreateAgentRunRequest {
  return {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" },
      author_message: { text: "Revise this phrase: keep the voice." },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

export async function writePassage(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  ns: string,
  text = PROSE,
) {
  const sessionRequest: CreateEditorSessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}1`),
  };
  const session = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/editor-sessions",
    sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest),
    id(`${ns}2`),
    (antiForgery) => createEditorSession({
      baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}2`), antiForgery,
      request: sessionRequest,
    }),
  );
  if (session.writer.kind !== "current_writer") throw new Error("expected current writer");
  const editRequest: ApplyAuthorEditRequest = {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}3`),
    editor_session_id: session.editor_session.editor_session_id,
    writer_generation: session.writer.writer_generation,
    chapter_id: session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: [],
    target_refs: session.base_snapshot.target_refs,
    observed_ownership_partition: "authoritative",
    editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: id(`${ns}4`),
    completed_intent_record_id: id(`${ns}5`),
    local_intent_sequence: "1",
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from: 0, to: 0, text }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 0,
      },
    }],
  };
  const written = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/manuscript/author-edits",
    editRequest.command_schema, await digestApplyAuthorEdit(editRequest), id(`${ns}6`),
    (antiForgery) => applyAuthorEdit({
      baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}6`), antiForgery,
      request: editRequest,
    }),
  );
  if (written.effect.kind !== "authoritative_applied") {
    throw new Error("expected written prose");
  }
  return {
    session,
    writerGeneration: session.writer.writer_generation,
    authoritativeRevisionId: written.effect.authoritative_revision.revision_id,
    nextSequence: "2",
  };
}

export async function admitPhrase(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  key: string,
) {
  const request = phraseRequest(chapterId, id(key.slice(-4)));
  const created = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema, await digestCreateAgentRun(request), key,
    (antiForgery) => createAgentRun({
      baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request,
    }),
  );
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  await settleOnce();
  return getAgentRun({ baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
}

export async function openInline(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  ns: string,
) {
  const writer = await writePassage(baseUrl, fetchImpl, projectId, `${ns}0`);
  const queried = await admitPhrase(baseUrl, fetchImpl, projectId, chapterId, id(`${ns}1`));
  assert.equal(queried.status, "completed");
  assert.equal(queried.decision.kind, "prose_change");
  if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened inline");
  }
  const opened = await getProposal({
    baseUrl, projectId, proposalId: queried.decision.opened_proposal.proposal_id, fetchImpl,
  });
  assert.equal(opened.proposal.kind, "inline_edit");
  assert.equal(opened.proposal.candidate_text, INLINE_CANDIDATE);
  assert.equal(opened.proposal.validation, "valid");
  assert.equal(opened.proposal.validation_receipt.kind, "present");
  assert.equal(opened.proposal.anchors.length, 1);
  const anchor = opened.proposal.anchors[0];
  if (anchor === undefined) throw new Error("expected one Anchor");
  assert.equal(anchor.manuscript_block_id, opened.proposal.manuscript_block_id);
  assert.equal(anchor.coordinate_profile, "prosemirror-token-utf16.v1");
  assert.equal(anchor.boundary_profile, "exclusive-authoritative-edges.v1");
  assert.equal(anchor.from, 10);
  assert.equal(anchor.to, 24);
  assert.equal(PROSE.slice(anchor.from, anchor.to), SOURCE_SLICE);
  assert.equal(sliceDigest("block-a"), GOLDEN_DIGEST);
  assert.equal(anchor.base_slice_digest, sliceDigest(anchor.manuscript_block_id));
  return { queried, opened, writer };
}

export function replaceUnit(
  from: number,
  to: number,
  text: string,
  writer: Awaited<ReturnType<typeof writePassage>>,
  ns: string,
  proposalRevisionId: string,
): ApplyAuthorEditRequest {
  return {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}3`),
    editor_session_id: writer.session.editor_session.editor_session_id,
    writer_generation: writer.writerGeneration,
    chapter_id: writer.session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: writer.authoritativeRevisionId,
    expected_proposal_head_revision_ids: [proposalRevisionId],
    target_refs: writer.session.base_snapshot.target_refs,
    observed_ownership_partition: "mixed",
    editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: id(`${ns}4`),
    completed_intent_record_id: id(`${ns}5`),
    local_intent_sequence: writer.nextSequence,
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from, to, text }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to,
      },
    }],
  };
}

/** Accepts the current Operation of `proposal` with its Validation Receipt on the expected Chapter Head. */
export async function acceptInline(
  options: { baseUrl: string; projectId: string; fetchImpl: typeof fetch },
  writer: Awaited<ReturnType<typeof writePassage>>,
  proposal: BlockProposalInspect,
  expectedRevisionId: string,
  ns: string,
) {
  if (proposal.validation_receipt.kind !== "present") throw new Error("expected a Validation Receipt");
  const request: AcceptProposalRequest = { command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: { ...BINDING, correlation_id: id(`${ns}1`), proposal_revision_id: proposal.revision_id,
      validation_receipt_id: proposal.validation_receipt.validation_receipt_id,
      selected_operation_ids: [proposal.operation_id], expected_authoritative_revision_id: expectedRevisionId,
      editor_session_id: writer.session.editor_session.editor_session_id } };
  return challenged(options.baseUrl, options.fetchImpl, options.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", request.command_schema,
    await digestAcceptProposal(request), id(`${ns}2`), (antiForgery) => acceptProposal({
      ...options, proposalId: proposal.proposal_id, request, antiForgery, idempotencyKey: id(`${ns}2`) }));
}
