// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/open-block-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { test } from "vitest";

import {
  applyAuthorEdit,
  createAgentRun,
  createEditorSession,
  digestApplyAuthorEdit,
  digestCreateAgentRun,
  digestCreateEditorSession,
  getAgentRun,
  getEditorSession,
  getProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateAgentRunRequest,
  CreateEditorSessionRequest,
  GetProposalResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";
import {
  BINDING, PROSE, USER_A, challenged, drainLeftoverWork, id, prepare, settleOnce,
  startRealServer,
} from "../support/acceptance.ts";

const FIRST = "Guard";
const SECOND = "Guard the narrator";

function freshNs(): string {
  return randomBytes(4).toString("hex");
}

function streamRequest(chapterId: string, correlationId: string): CreateAgentRunRequest {
  return {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" },
      author_message: { text: "Stream this passage: keep the voice." },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

async function admitStream(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  key: string,
) {
  const request = streamRequest(chapterId, id(key.slice(-4)));
  const created = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema, await digestCreateAgentRun(request), key,
    (antiForgery) => createAgentRun({ baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request }),
  );
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  return created.effect.run_id;
}

async function waitForOpened(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  runId: string,
  match: (value: GetProposalResponse) => boolean,
): Promise<GetProposalResponse> {
  let last = "";
  for (let attempt = 0; attempt < 250; attempt += 1) {
    const queried = await getAgentRun({ baseUrl, projectId, runId, fetchImpl });
    last = `${queried.status}:${queried.decision.kind}`;
    if (queried.decision.kind === "prose_change" && queried.decision.opened_proposal.kind === "present") {
      const current = await getProposal({
        baseUrl, projectId, proposalId: queried.decision.opened_proposal.proposal_id, fetchImpl,
      });
      last = `${last}:${current.proposal.generation}:${current.proposal.candidate_text}`;
      if (match(current)) return current;
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error(`streamed Proposal inspect timed out (${last})`);
}

test("Worker applies contiguous canonical batches and completes one streamed candidate", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = freshNs();
    const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Stream Novel", ns);
    const runId = await admitStream(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id(`${ns}31`));
    await drainLeftoverWork();
    const queried = await getAgentRun({
      baseUrl: started.baseUrl, projectId: prepared.projectId, runId, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(queried.status, "completed");
    if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
      throw new Error("expected opened prose");
    }
    const inspected = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: queried.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.generation, "ready");
    assert.equal(inspected.proposal.validation, "valid");
    assert.equal(inspected.proposal.candidate_text, PROSE);
    assert.equal(inspected.proposal.validation_receipt.kind, "present");
    await assert.rejects(
      () => getProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: inspected.proposal.proposal_id, fetchImpl: browserFetch(started.baseUrl, "session-b"),
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test("first author input fences the admitted Head and late batches stay ineligible", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = freshNs();
    const prepared = await prepare(started.baseUrl, id(`${ns}41`), "Fence Novel", ns);
    const runId = await admitStream(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id(`${ns}51`));
    await settleOnce();
    const opened = await waitForOpened(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, runId,
      (current) => current.proposal.generation === "generating" && current.proposal.candidate_text === FIRST,
    );
    assert.notEqual(opened.proposal.candidate_text, SECOND);
    assert.equal(opened.proposal.validation_receipt.kind, "absent");
    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      ...BINDING, correlation_id: id(`${ns}61`),
    };
    const session = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/editor-sessions",
      sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest), id(`${ns}62`),
      (antiForgery) => createEditorSession({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id(`${ns}62`), antiForgery, request: sessionRequest,
      }),
    );
    if (session.writer.kind !== "current_writer") throw new Error("expected current writer");
    const beforeFence = await getEditorSession({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      editorSessionId: session.editor_session.editor_session_id, fetchImpl: prepared.fetchImpl,
    });
    const editRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      ...BINDING, correlation_id: id(`${ns}63`),
      editor_session_id: session.editor_session.editor_session_id,
      writer_generation: session.writer.writer_generation,
      chapter_id: session.base_snapshot.chapter_id,
      expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids: [opened.proposal.revision_id],
      target_refs: session.base_snapshot.target_refs,
      observed_ownership_partition: "mixed",
      editor_contract_revision: "storyos.editor-contract.release-1.v3",
      undo_group_id: id(`${ns}64`),
      completed_intent_record_id: id(`${ns}65`),
      local_intent_sequence: "1",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from: 0, to: 5, text: "Keep" }],
        selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 5 },
      }],
    };
    const fenced = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits",
      editRequest.command_schema, await digestApplyAuthorEdit(editRequest), id(`${ns}66`),
      (antiForgery) => applyAuthorEdit({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id(`${ns}66`), antiForgery, request: editRequest,
      }),
    );
    assert.equal(fenced.effect.kind, "conflicted");
    const paused = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(paused.proposal.generation, "ready_partial");
    assert.equal(paused.proposal.candidate_text, FIRST);
    assert.notEqual(paused.proposal.revision_id, opened.proposal.revision_id);
    assert.equal(paused.proposal.validation_receipt.kind, "absent");
    const afterFence = await getEditorSession({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      editorSessionId: session.editor_session.editor_session_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(afterFence.author_undo_frontier_sequence, beforeFence.author_undo_frontier_sequence);
    await settleOnce();
    const late = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(late.proposal.generation, "ready_partial");
    assert.equal(late.proposal.candidate_text, FIRST);
    assert.equal(late.proposal.revision_id, paused.proposal.revision_id);
    assert.notEqual(late.proposal.candidate_text, SECOND);
    assert.notEqual(late.proposal.candidate_text, PROSE);
  } finally {
    await stopRealServer(started.server);
  }
});
