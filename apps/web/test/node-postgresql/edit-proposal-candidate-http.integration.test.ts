// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/stream-proposal-generation-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  applyAuthorEdit,
  createAgentRun,
  createChapter,
  createEditorSession,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  digestApplyAuthorEdit,
  digestCreateAgentRun,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  digestUndoLatestAuthorAction,
  digestUpdateProjectAssistance,
  getAgentRun,
  getApplyAuthorEditOutcome,
  getChapter,
  getEditorSession,
  getProposal,
  undoLatestAuthorAction,
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateAgentRunRequest,
  CreateEditorSessionRequest,
  UndoLatestAuthorActionRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  runStoryOSWorker,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const bin = (name: string) => join(repositoryRoot, "target", "release-package", process.platform === "win32" ? `${name}.exe` : name);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};
const PROSE = "Guard the narrator voice in this passage.";
const REVISED = "Keep the narrator voice in this passage.";

function id(suffix: string): string {
  return `018f0000-0000-7001-8000-${suffix.padStart(12, "0")}`;
}

function runRequest(chapterId: string, correlationId: string): CreateAgentRunRequest {
  return {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" },
      author_message: { text: "Revise this passage: keep the voice." },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary: bin("storyos-server"),
    sessions: { "session-a": USER_A },
  });
}

async function settleOnce() {
  await runStoryOSWorker({
    repositoryRoot,
    workerBinary: bin("storyos-worker"),
    args: ["--once"],
  });
}

async function drainLeftoverWork() {
  for (let attempt = 0; attempt < 16; attempt += 1) {
    const leftover = await queryPostgres(`
      SELECT count(*)::text
        FROM storyos.agent_runs
       WHERE status IN ('queued', 'claimed');
    `);
    if (leftover === "0") return;
    await settleOnce();
  }
  throw new Error("leftover AgentRun work did not drain");
}

async function challenged<T>(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  method: string,
  route: string,
  schema: string,
  digest: Awaited<ReturnType<typeof digestCreateAgentRun>>,
  key: string,
  send: (antiForgery: string) => Promise<T>,
): Promise<T> {
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: { method, route_template: route, command_schema: schema, canonical_command_digest: digest, idempotency_key: key },
  }));
  return send(challenge.nonce);
}

async function prepare(baseUrl: string, createKey: string, title: string, ns: string) {
  const fetchImpl = browserFetch(baseUrl, "session-a");
  const createRequest = {
    command_schema: "storyos.command.create-project.request.v1" as const,
    create_project_input: { title, ...BINDING, correlation_id: id(`${ns}0`) },
    idempotency_key: createKey,
  };
  const created = await createProjectChallenge({ baseUrl, request: createRequest, fetchImpl });
  await createProject({
    baseUrl,
    fetchImpl,
    idempotencyKey: createKey,
    antiForgery: created.nonce,
    request: {
      command_schema: createRequest.command_schema,
      prospective_project_id: created.prospective_project_id,
      create_project_input: createRequest.create_project_input,
    },
  });
  const projectId = created.prospective_project_id;
  const assistance = {
    command_schema: "storyos.command.update-project-assistance.request.v1" as const,
    update_project_assistance_input: {
      availability: "available" as const,
      expected_assistance_revision: "0",
      ...BINDING,
      correlation_id: id(`${ns}1`),
    },
  };
  await challenged(baseUrl, fetchImpl, projectId, "PUT", "/api/v1/projects/{project_id}/assistance", assistance.command_schema, await digestUpdateProjectAssistance(assistance), id(`${ns}2`), (antiForgery) => updateProjectAssistance({ baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}2`), antiForgery, request: assistance }));
  const volume = {
    command_schema: "storyos.command.create-volume.request.v1" as const,
    create_volume_input: { title: "Volume A", expected_tree_revision: "1", ...BINDING, correlation_id: id(`${ns}3`) },
  };
  const createdVolume = await challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/volumes", volume.command_schema, await digestCreateVolume(volume), id(`${ns}4`), (antiForgery) => createVolume({ baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}4`), antiForgery, request: volume }));
  if (createdVolume.effect.kind !== "authoritative_applied") throw new Error("Create Volume must apply");
  const volumeId = createdVolume.effect.volume_id;
  const chapter = {
    command_schema: "storyos.command.create-chapter.request.v1" as const,
    create_chapter_input: { title: "Chapter A", expected_tree_revision: "2", ...BINDING, correlation_id: id(`${ns}5`) },
  };
  const createdChapter = await challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters", chapter.command_schema, await digestCreateChapter(chapter), id(`${ns}6`), (antiForgery) => createChapter({ baseUrl, projectId, volumeId, fetchImpl, idempotencyKey: id(`${ns}6`), antiForgery, request: chapter }));
  if (createdChapter.effect.kind !== "authoritative_applied") throw new Error("Create Chapter must apply");
  return { fetchImpl, projectId, chapterId: createdChapter.effect.chapter_id };
}

async function admitProse(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  key: string,
) {
  const request = runRequest(chapterId, id(key.slice(-4)));
  const created = await challenged(
    baseUrl,
    fetchImpl,
    projectId,
    "POST",
    "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema,
    await digestCreateAgentRun(request),
    key,
    (antiForgery) => createAgentRun({ baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request }),
  );
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  await settleOnce();
  return getAgentRun({ baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
}

test("applyAuthorEdit revises one Proposal candidate in place and Root Undo restores it", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("c811"), "Edit Proposal Novel", "c82");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("c831"));
    assert.equal(queried.decision.kind, "prose_change");
    if (queried.decision.kind !== "prose_change") throw new Error("expected prose");
    assert.equal(queried.decision.opened_proposal.kind, "present");
    if (queried.decision.opened_proposal.kind !== "present") throw new Error("expected opened");
    const proposalId = queried.decision.opened_proposal.proposal_id;
    const opened = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.proposal.candidate_text, PROSE);
    assert.equal(opened.proposal.validation_receipt.kind, "present");
    if (opened.proposal.validation_receipt.kind !== "present") throw new Error("expected receipt");
    const openedReceiptId = opened.proposal.validation_receipt.validation_receipt_id;
    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      ...BINDING,
      correlation_id: id("c841"),
    };
    const session = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/editor-sessions",
      sessionRequest.command_schema,
      await digestCreateEditorSession(sessionRequest),
      id("c842"),
      (antiForgery) => createEditorSession({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("c842"),
        antiForgery,
        request: sessionRequest,
      }),
    );
    if (session.writer.kind !== "current_writer") throw new Error("expected current writer");
    const editRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      ...BINDING,
      correlation_id: id("c843"),
      editor_session_id: session.editor_session.editor_session_id,
      writer_generation: session.writer.writer_generation,
      chapter_id: session.base_snapshot.chapter_id,
      expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids: [opened.proposal.revision_id],
      proposal_target: {
        proposal_id: opened.proposal.proposal_id,
        operation_id: opened.proposal.operation_id,
        revision_id: opened.proposal.revision_id,
        manuscript_block_id: opened.proposal.manuscript_block_id,
      },
      target_refs: session.base_snapshot.target_refs,
      observed_ownership_partition: "mixed",
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
      undo_group_id: id("c844"),
      completed_intent_record_id: id("c845"),
      local_intent_sequence: "1",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from: 0, to: 5, text: "Keep" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 5,
        },
      }],
    };
    const editOptions = {
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("c846"),
      antiForgery: "",
      request: editRequest,
    };
    const edited = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits",
      editRequest.command_schema,
      await digestApplyAuthorEdit(editRequest),
      id("c846"),
      (antiForgery) => {
        editOptions.antiForgery = antiForgery;
        return applyAuthorEdit(editOptions);
      },
    );
    assert.equal(edited.effect.kind, "proposal_revised");
    if (edited.effect.kind !== "proposal_revised") throw new Error("expected ProposalRevised");
    assert.equal(edited.receipt.result, "proposal_revised");
    assert.match(edited.effect.proposal_revision_id, UUID_V7);
    assert.notEqual(edited.effect.proposal_revision_id, opened.proposal.revision_id);
    assert.deepEqual(edited.receipt.proposal_revision_ids, [edited.effect.proposal_revision_id]);
    assert.deepEqual(edited.receipt.authoritative_revision_ids, []);
    assert.deepEqual(edited.receipt.authoritative_commit_ids, []);
    assert.equal(edited.receipt.author_action_sequence, edited.effect.author_action_sequence);
    assert.match(edited.effect.author_action_sequence, /^[1-9][0-9]*$/);
    assert.deepEqual(await applyAuthorEdit(editOptions), edited);
    const editOutcome = await getApplyAuthorEditOutcome({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      fetchImpl: prepared.fetchImpl,
      idempotencyKey: id("c846"),
      antiForgery: editOptions.antiForgery,
    });
    assert.equal(editOutcome.outcome.outcome_kind, "committed");
    if (editOutcome.outcome.outcome_kind !== "committed") {
      throw new Error("expected committed outcome");
    }
    assert.deepEqual(editOutcome.outcome.response, edited);
    const revised = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(revised.proposal.proposal_id, opened.proposal.proposal_id);
    assert.equal(revised.proposal.operation_id, opened.proposal.operation_id);
    assert.equal(revised.proposal.revision_id, edited.effect.proposal_revision_id);
    assert.equal(revised.proposal.candidate_text, REVISED);
    assert.equal(revised.proposal.validation, "valid");
    assert.equal(revised.proposal.validation_receipt.kind, "present");
    if (revised.proposal.validation_receipt.kind !== "present") throw new Error("expected new receipt");
    assert.match(revised.proposal.validation_receipt.validation_receipt_id, UUID_V7);
    assert.notEqual(revised.proposal.validation_receipt.validation_receipt_id, openedReceiptId);
    assert.equal(revised.proposal.validation_receipt.result, "valid");
    const ambiguousRequest: ApplyAuthorEditRequest = {
      ...editRequest,
      correlation_id: id("c849"),
      expected_proposal_head_revision_ids: [revised.proposal.revision_id],
      undo_group_id: id("c850"),
      completed_intent_record_id: id("c851"),
      local_intent_sequence: "2",
    };
    delete ambiguousRequest.proposal_target;
    const ambiguous = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits",
      ambiguousRequest.command_schema,
      await digestApplyAuthorEdit(ambiguousRequest),
      id("c852"),
      (antiForgery) => applyAuthorEdit({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("c852"),
        antiForgery,
        request: ambiguousRequest,
      }),
    );
    assert.equal(ambiguous.effect.kind, "refused");
    const ordinaryRequest: ApplyAuthorEditRequest = {
      ...ambiguousRequest,
      correlation_id: id("c853"),
      undo_group_id: id("c854"),
      completed_intent_record_id: id("c855"),
      local_intent_sequence: "3",
      author_edit_units: [{
        normalized_primitives: [{
          kind: "replace_block_selection",
          manuscript_block_id: opened.proposal.manuscript_block_id,
          from: 0,
          to: 0,
          text: "Ordinary input",
        }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 0,
        },
      }],
    };
    const ordinary = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits",
      ordinaryRequest.command_schema,
      await digestApplyAuthorEdit(ordinaryRequest),
      id("c856"),
      (antiForgery) => applyAuthorEdit({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("c856"),
        antiForgery,
        request: ordinaryRequest,
      }),
    );
    assert.equal(ordinary.effect.kind, "conflicted");
    const unchangedCandidate = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(unchangedCandidate.proposal.revision_id, revised.proposal.revision_id);
    assert.equal(unchangedCandidate.proposal.candidate_text, REVISED);
    const afterEdit = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(afterEdit.chapter, before.chapter);
    const refreshed = await getEditorSession({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      editorSessionId: session.editor_session.editor_session_id,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(refreshed.author_undo_frontier_sequence, edited.effect.author_action_sequence);
    const undoRequest: UndoLatestAuthorActionRequest = {
      command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: {
        expected_author_undo_frontier_sequence: edited.effect.author_action_sequence,
        expected_authoritative_revision_id: before.chapter.current_revision.revision_id,
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id("c847"),
      },
    };
    const undone = await challenged(
      started.baseUrl,
      prepared.fetchImpl,
      prepared.projectId,
      "POST",
      "/api/v1/projects/{project_id}/author-actions/undo",
      undoRequest.command_schema,
      await digestUndoLatestAuthorAction(undoRequest),
      id("c848"),
      (antiForgery) => undoLatestAuthorAction({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("c848"),
        antiForgery,
        request: undoRequest,
      }),
    );
    assert.equal(undone.effect.kind, "compensated");
    const restored = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(restored.proposal.proposal_id, opened.proposal.proposal_id);
    assert.equal(restored.proposal.operation_id, opened.proposal.operation_id);
    assert.equal(restored.proposal.candidate_text, PROSE);
    assert.notEqual(restored.proposal.revision_id, revised.proposal.revision_id);
    assert.notEqual(restored.proposal.revision_id, opened.proposal.revision_id);
    assert.equal(restored.proposal.validation_receipt.kind, "present");
    if (restored.proposal.validation_receipt.kind !== "present") throw new Error("expected undo receipt");
    assert.notEqual(restored.proposal.validation_receipt.validation_receipt_id, revised.proposal.validation_receipt.kind === "present" ? revised.proposal.validation_receipt.validation_receipt_id : "");
    const afterUndo = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(afterUndo.chapter, before.chapter);
  } finally {
    await stopRealServer(started.server);
  }
});
