// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/stream-proposal-generation-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "vitest";

import {
  applyAuthorEdit,
  createEditorSession,
  digestApplyAuthorEdit,
  digestCreateEditorSession,
  digestUndoLatestAuthorAction,
  pauseAgentRun, digestPauseAgentRun, steerAgentRun, digestSteerAgentRun,
  getAgentRun,
  getApplyAuthorEditOutcome,
  getChapter,
  getEditorSession,
  getProposal,
  undoLatestAuthorAction,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateEditorSessionRequest,
  UndoLatestAuthorActionRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";
import { BINDING, UUID_V7, admitProse, challenged, drainLeftoverWork, id, prepare, settleOnce,
  startServer } from "../support/http-harness.ts";

import { admitCandidateRevision, candidateReference } from "../support/candidate-revision.ts";

function assertEmptyBaseComparison(
  proposal: Awaited<ReturnType<typeof getProposal>>["proposal"],
  candidateText: string,
) {
  assert.equal(proposal.candidate_text, candidateText);
  assert.deepEqual(proposal.revision_comparison, {
    kind: "present",
    base_authoritative_revision_id: proposal.base_authoritative_revision_id,
    candidate_revision_id: proposal.revision_id,
    operation_id: proposal.operation_id,
    spans: [{
      base_from: 0,
      base_to: 0,
      candidate_from: 0,
      candidate_to: candidateText.length,
      base_text: "",
      candidate_text: candidateText,
    }],
  });
}

test("applyAuthorEdit revises one Proposal candidate in place and Root Undo restores it", async () => {
  const started = await startServer();
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
    let opened = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    await admitCandidateRevision(started.baseUrl, prepared.fetchImpl, prepared.projectId,
      opened.proposal, opened.proposal.operation_id, id("c833"), "revise the candidate consistently.");
    await settleOnce();
    opened = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId, proposalId, fetchImpl: prepared.fetchImpl });
    const PROSE = "Guard the narrator voice in this passage. Keep the voice consistent.";
    const REVISED = "Keep the narrator voice in this passage. Keep the voice consistent.";
    assert.equal(opened.proposal.candidate_text, PROSE);
    const baseBlock = before.chapter.current_revision.blocks.find((block) =>
      block.manuscript_block_id === opened.proposal.manuscript_block_id);
    assert.equal(baseBlock?.text, "");
    assertEmptyBaseComparison(opened.proposal, PROSE);
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
      editor_contract_revision: "storyos.editor-contract.release-1.v3",
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
    const chapterBefore = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    const wrong = structuredClone(editRequest);
    wrong.correlation_id = id("c857");
    wrong.completed_intent_record_id = id("c858");
    wrong.proposal_target!.operation_id = id("c859");
    const refused = await challenged(started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", wrong.command_schema,
      await digestApplyAuthorEdit(wrong), id("c85a"), (antiForgery) => applyAuthorEdit({
        ...editOptions, idempotencyKey: id("c85a"), antiForgery, request: wrong }));
    assert.deepEqual(refused.effect, { kind: "refused", reason: "target_mismatch" });
    assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl })).chapter, chapterBefore.chapter);
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId, proposalId,
      fetchImpl: prepared.fetchImpl })).proposal, opened.proposal);
    const stale = await admitCandidateRevision(started.baseUrl, prepared.fetchImpl, prepared.projectId,
      opened.proposal, opened.proposal.operation_id, id("c847"));
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
    await settleOnce();
    if (stale.created.effect.kind !== "admitted") throw new Error("expected admitted stale request");
    const staleRun = await getAgentRun({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      runId: stale.created.effect.run_id, fetchImpl: prepared.fetchImpl });
    if (staleRun.decision.kind !== "prose_change") throw new Error("expected selected stale revision");
    assert.deepEqual(staleRun.decision.locations?.map((location) => location.outcome), [{ kind: "refused", reason: "stale_candidate" }]);
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId, proposalId,
      fetchImpl: prepared.fetchImpl })).proposal, revised.proposal);
    assert.equal(revised.proposal.proposal_id, opened.proposal.proposal_id);
    assert.equal(revised.proposal.operation_id, opened.proposal.operation_id);
    assert.equal(revised.proposal.revision_id, edited.effect.proposal_revision_id);
    assert.notEqual(revised.proposal.revision_id, opened.proposal.revision_id);
    assertEmptyBaseComparison(revised.proposal, REVISED);
    assert.equal(revised.proposal.validation, opened.proposal.validation);
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
    // The Compensation writes no Authoritative Revision and no Authoritative Commit, so the effect names neither.
    assert.deepEqual([Object.hasOwn(undone.effect, "authoritative_revision"), Object.hasOwn(undone.effect, "authoritative_commit_id")],
      [false, false]);
    assert.deepEqual([undone.receipt.resulting_heads, undone.receipt.authoritative_revision_ids, undone.receipt.authoritative_commit_ids],
      [[before.chapter.current_revision.revision_id], [], []]);
    const restored = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(restored.proposal.proposal_id, opened.proposal.proposal_id);
    assert.equal(restored.proposal.operation_id, opened.proposal.operation_id);
    assertEmptyBaseComparison(restored.proposal, PROSE);
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


test("a long shared prefix keeps an exact comparison after a candidate edit and a restart", async () => {
  let started = await startServer();
  try {
    await drainLeftoverWork();
    const { fetchImpl, projectId, chapterId } = await prepare(started.baseUrl, id("cc11"), "Long prefix comparison", "cc2");
    const prefix = "a".repeat(9_000);
    const [baseTail, candidateTail] = ["b".repeat(400), "c".repeat(400)];
    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1", ...BINDING, correlation_id: id("cc31"),
    };
    const session = await challenged(started.baseUrl, fetchImpl, projectId, "POST",
      "/api/v1/projects/{project_id}/editor-sessions", sessionRequest.command_schema,
      await digestCreateEditorSession(sessionRequest), id("cc32"), (antiForgery) => createEditorSession({
        baseUrl: started.baseUrl, projectId, fetchImpl, idempotencyKey: id("cc32"), antiForgery, request: sessionRequest }));
    if (session.writer.kind !== "current_writer") throw new Error("expected current writer");
    const writer = session.writer;
    async function authorEdit(ns: string, fields: Partial<ApplyAuthorEditRequest>, from: number, to: number, text: string) {
      const request: ApplyAuthorEditRequest = {
        command_schema: "storyos.command.apply-author-edit.request.v1", ...BINDING, correlation_id: id(`${ns}1`),
        editor_session_id: session.editor_session.editor_session_id, writer_generation: writer.writer_generation,
        chapter_id: chapterId, expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
        expected_proposal_head_revision_ids: [], target_refs: session.base_snapshot.target_refs,
        observed_ownership_partition: "authoritative", editor_contract_revision: "storyos.editor-contract.release-1.v3",
        undo_group_id: id(`${ns}2`), completed_intent_record_id: id(`${ns}3`), local_intent_sequence: "1",
        author_edit_units: [{ normalized_primitives: [{ kind: "replace_selection", from, to, text }],
          selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to } }],
        ...fields,
      };
      return challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/manuscript/author-edits",
        request.command_schema, await digestApplyAuthorEdit(request), id(`${ns}4`), (antiForgery) => applyAuthorEdit({
          baseUrl: started.baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}4`), antiForgery, request }));
    }
    const based = await authorEdit("cc4", {}, 0, 0, prefix + baseTail);
    if (based.effect.kind !== "authoritative_applied") throw new Error("expected an applied base edit");
    const queried = await admitProse(started.baseUrl, fetchImpl, projectId, chapterId, id("cc51"));
    if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") throw new Error("expected candidate");
    const proposalId = queried.decision.opened_proposal.proposal_id;
    const opened = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl });
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    const edited = await authorEdit("cc6", {
      expected_authoritative_revision_id: based.effect.authoritative_revision.revision_id,
      expected_proposal_head_revision_ids: [opened.proposal.revision_id],
      proposal_target: { proposal_id: proposalId, operation_id: opened.proposal.operation_id,
        revision_id: opened.proposal.revision_id, manuscript_block_id: opened.proposal.manuscript_block_id },
      observed_ownership_partition: "mixed", local_intent_sequence: "2",
    }, 0, opened.proposal.candidate_text.length, prefix + candidateTail);
    if (edited.effect.kind !== "proposal_revised") throw new Error("expected ProposalRevised");
    const revised = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl });
    assert.deepEqual(revised.proposal.revision_comparison, {
      kind: "present",
      base_authoritative_revision_id: based.effect.authoritative_revision.revision_id,
      candidate_revision_id: edited.effect.proposal_revision_id,
      operation_id: opened.proposal.operation_id,
      spans: [{ base_from: 9_000, base_to: 9_400, candidate_from: 9_000, candidate_to: 9_400,
        base_text: baseTail, candidate_text: candidateTail }],
    });
    assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter, chapter.chapter);
    await stopRealServer(started.server);
    started = await startServer();
    const reloadedFetch = browserFetch(started.baseUrl, "session-a");
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl: reloadedFetch })).proposal, revised.proposal);
    assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl: reloadedFetch })).chapter, chapter.chapter);
  } finally { await stopRealServer(started.server); }
}, 120_000);

test("a fresh instruction revises the exact pending candidate through the real Worker", async () => {
  let started = await startServer();
  try {
    await drainLeftoverWork();
    const { fetchImpl, projectId, chapterId } = await prepare(started.baseUrl, id("ca11"), "AI candidate revision", "ca2");
    const original = await admitProse(started.baseUrl, fetchImpl, projectId, chapterId, id("ca31"));
    if (original.decision.kind !== "prose_change" || original.decision.opened_proposal.kind !== "present") throw new Error("expected original candidate");
    const proposalId = original.decision.opened_proposal.proposal_id;
    const before = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl });
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    const { created, repeat } = await admitCandidateRevision(started.baseUrl, fetchImpl, projectId,
      before.proposal, before.proposal.operation_id, id("ca42"));
    if (created.effect.kind !== "admitted") throw new Error("expected fresh candidate request");
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl })).proposal, before.proposal);
    const runId = created.effect.run_id;
    const pause = { command_schema: "storyos.command.pause-agent-run.request.v1" as const,
      pause_agent_run_input: { ...BINDING, correlation_id: id("ca45") } };
    await challenged(started.baseUrl, fetchImpl, projectId, "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause", pause.command_schema,
      await digestPauseAgentRun(pause), id("ca45"), (antiForgery) => pauseAgentRun({
        baseUrl: started.baseUrl, projectId, runId, fetchImpl, request: pause,
        idempotencyKey: id("ca45"), antiForgery }));
    const guidance = { command_schema: "storyos.command.steer-agent-run.request.v1" as const,
      steer_agent_run_input: { conversation_id: created.conversation_id,
        author_message: { text: "Keep the narrator calm in this passage." }, ...BINDING, correlation_id: id("ca46") } };
    await challenged(started.baseUrl, fetchImpl, projectId, "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/steering-inputs", guidance.command_schema,
      await digestSteerAgentRun(guidance), id("ca46"), (antiForgery) => steerAgentRun({
        baseUrl: started.baseUrl, projectId, runId, fetchImpl, request: guidance,
        idempotencyKey: id("ca46"), antiForgery }));
    await settleOnce();
    const revised = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl });
    const run = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
    assert.equal(revised.proposal.operation_id, before.proposal.operation_id);
    assert.notEqual(revised.proposal.revision_id, before.proposal.revision_id);
    assert.equal(revised.proposal.candidate_text, "Keep the narrator calm in this passage.");
    assert.notDeepEqual(revised.proposal.validation_receipt, before.proposal.validation_receipt);
    assert.equal(run.status, "completed");
    assert.deepEqual(await repeat(), created);
    assert.deepEqual(revised.proposal.source, before.proposal.source);
    if (run.decision.kind !== "prose_change") throw new Error("expected typed revision");
    if (revised.proposal.validation_receipt.kind !== "present") throw new Error("expected fresh validation");
    assert.deepEqual(run.decision.locations, [{ chapter_id: chapterId, manuscript_block_id: before.proposal.manuscript_block_id,
      base_authoritative_revision_id: chapter.chapter.current_revision.revision_id, candidate_text: revised.proposal.candidate_text,
      explanation: "Revise the selected candidate under the new author instruction.",
      outcome: { kind: "revised", proposal_id: proposalId, operation_id: before.proposal.operation_id,
        revision_id: revised.proposal.revision_id, prior_revision_id: before.proposal.revision_id,
        validation_receipt_id: revised.proposal.validation_receipt.validation_receipt_id },
      current: { revision_id: revised.proposal.revision_id, generation: "ready", validation: "valid",
        closure: "open", resolution: "pending", reservation_state: "unresolved" } }]);
    const sent = run.evidence.find((item) => item.kind === "sent_content");
    if (sent?.kind !== "sent_content") throw new Error("expected actual sent bytes");
    const wire = JSON.parse(sent.content);
    assert.deepEqual(wire.selection_references.map(({ selected_text }: { selected_text: string }) => selected_text),
      [before.proposal.candidate_text]);
    assert.deepEqual(wire.selected, run.context.selected.map((source) => ({ ...source, token_count: Number(source.token_count) })));
    const retained = JSON.parse(await queryPostgres(`SELECT payload->'wire' FROM storyos.model_attempts WHERE run_id='${run.run_id}'::uuid`));
    assert.equal(retained.serialized_payload, sent.content);
    assert.equal(retained.digest, `sha256:${createHash("sha256").update(sent.content).digest("hex")}`);
    await assert.rejects(() => admitCandidateRevision(started.baseUrl, fetchImpl, projectId,
      { ...revised.proposal, chapter_id: id("ca90") }, revised.proposal.operation_id, id("ca92")), { status: 422 });
    for (const [index, invalid] of [revised.proposal, before.proposal].entries()) {
      const { created: unavailable } = await admitCandidateRevision(started.baseUrl, fetchImpl, projectId, invalid,
        index === 0 ? id("ca91") : invalid.operation_id, id(`ca9${index + 3}`));
      if (unavailable.effect.kind !== "admitted") throw new Error("expected an admitted unavailable reference");
      const blocked = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: unavailable.effect.run_id, fetchImpl });
      assert.equal(blocked.context.sufficiency.kind, "blocked");
      assert.deepEqual(blocked.model_attempt, { kind: "absent" });
    }
    await assert.rejects(() => admitCandidateRevision(started.baseUrl, browserFetch(started.baseUrl, "session-b"), projectId,
      revised.proposal, revised.proposal.operation_id, id("ca99")), { status: 404 });
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl })).proposal, revised.proposal);
    assert.deepEqual(candidateReference(run.context), { kind: "proposal_candidate_text", chapter_id: chapterId,
      proposal_id: proposalId, operation_id: before.proposal.operation_id, revision_id: before.proposal.revision_id,
      coordinate_profile: "storyos.editor.utf16-code-unit.v1", start_offset: 0,
      end_offset: before.proposal.candidate_text.length });
    assert.deepEqual(run.context.selected.filter((item) => item.source_class === "working_target")
      .map(({ source_version, content }) => ({ source_version, content })), [
      { source_version: chapter.chapter.current_revision.revision_id, content: chapter.chapter.current_revision.body },
      { source_version: before.proposal.revision_id, content: before.proposal.candidate_text }]);
    await stopRealServer(started.server);
    started = await startServer();
    const reloadedFetch = browserFetch(started.baseUrl, "session-a");
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl: reloadedFetch })).proposal, revised.proposal);
    await settleOnce();
    const reloaded = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: run.run_id, fetchImpl: reloadedFetch });
    assert.deepEqual(reloaded, { ...run, correlation_id: reloaded.correlation_id });
    assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl: reloadedFetch })).chapter, chapter.chapter);
  } finally { await stopRealServer(started.server); }
}, 120_000);


test("guidance after a candidate Decision revises that Run's exact output and preserves history", async () => {
  const started = await startServer();
  const directory = await mkdtemp(join(tmpdir(), "storyos-candidate-guidance-"));
  const hold = join(directory, "decision");
  let worker: Promise<void> | undefined;
  try {
    await drainLeftoverWork();
    const { fetchImpl, projectId, chapterId } = await prepare(started.baseUrl, id("cb11"), "Candidate guidance", "cb2");
    const original = await admitProse(started.baseUrl, fetchImpl, projectId, chapterId, id("cb31"));
    if (original.decision.kind !== "prose_change" || original.decision.opened_proposal.kind !== "present") throw new Error("expected candidate");
    const proposalId = original.decision.opened_proposal.proposal_id;
    const before = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl });
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    const { created } = await admitCandidateRevision(started.baseUrl, fetchImpl, projectId,
      before.proposal, before.proposal.operation_id, id("cb41"));
    const options = { baseUrl: started.baseUrl, projectId, runId: created.effect.run_id, fetchImpl };
    await writeFile(hold, "hold");
    worker = settleOnce({ STORYOS_TEST_FAKE_DECISION_HOLD_PATH: hold });
    await expect.poll(async () => (await getAgentRun(options)).decision.kind).toBe("prose_change");
    const held = await getAgentRun(options);
    assert.equal(held.status, "claimed");
    if (held.model_attempt.kind !== "present") throw new Error("expected original Attempt");
    const revised = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl });
    assert.notEqual(revised.proposal.revision_id, before.proposal.revision_id);
    const guidance = { command_schema: "storyos.command.steer-agent-run.request.v1" as const,
      steer_agent_run_input: { conversation_id: created.conversation_id,
        author_message: { text: "Keep the narrator calm in this passage." }, ...BINDING, correlation_id: id("cb42") } };
    await challenged(started.baseUrl, fetchImpl, projectId, "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/steering-inputs", guidance.command_schema,
      await digestSteerAgentRun(guidance), id("cb42"), (antiForgery) => steerAgentRun({ ...options,
        request: guidance, idempotencyKey: id("cb42"), antiForgery }));
    await rm(hold);
    await worker;
    const current = await getAgentRun(options);
    const after = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl });
    assert.equal(current.status, "completed");
    assert.equal(current.conversation_id, created.conversation_id);
    assert.deepEqual(candidateReference(current.context), { kind: "proposal_candidate_text", chapter_id: chapterId,
      proposal_id: proposalId, operation_id: before.proposal.operation_id, revision_id: revised.proposal.revision_id,
      coordinate_profile: "storyos.editor.utf16-code-unit.v1", start_offset: 0,
      end_offset: revised.proposal.candidate_text.length });
    assert.equal(after.proposal.operation_id, before.proposal.operation_id);
    assert.notEqual(after.proposal.revision_id, revised.proposal.revision_id);
    assert.equal(after.proposal.candidate_text, guidance.steer_agent_run_input.author_message.text);
    if (current.decision.kind !== "prose_change") throw new Error("expected guided revision");
    if (after.proposal.validation_receipt.kind !== "present") throw new Error("expected validation");
    assert.deepEqual(current.decision.locations?.[0]?.outcome, { kind: "revised", proposal_id: proposalId,
      operation_id: before.proposal.operation_id, revision_id: after.proposal.revision_id,
      prior_revision_id: revised.proposal.revision_id, validation_receipt_id: after.proposal.validation_receipt.validation_receipt_id });
    const historicalOptions = { ...options, modelAttemptId: held.model_attempt.model_attempt_id };
    const historical = await getAgentRun(historicalOptions);
    // The availability of a reference is current, so only the historical reference itself stays the same.
    const recorded = ({ selection_references, ...context }: typeof held.context) => ({ ...context,
      selection_references: selection_references?.map(({ reference, manuscript_block_ids }) => ({ reference, manuscript_block_ids })) });
    assert.deepEqual(recorded(historical.context), recorded(held.context));
    assert.deepEqual(historical.context.selection_references?.[0]?.availability,
      { kind: "superseded", current_revision_id: after.proposal.revision_id });
    assert.deepEqual(historical.model_attempt, { ...held.model_attempt, dispatch_state: "settled" });
    if (held.decision.kind !== "prose_change" || historical.decision.kind !== "prose_change") throw new Error("expected historical revision");
    assert.deepEqual(historical.decision.locations?.map(({ current, ...location }) => location),
      held.decision.locations?.map(({ current, ...location }) => location));
    const reloaded = await getAgentRun(historicalOptions);
    assert.deepEqual(reloaded, { ...historical, correlation_id: reloaded.correlation_id });
    assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter, chapter.chapter);
  } finally {
    await rm(directory, { recursive: true, force: true });
    await worker?.catch(() => undefined);
    await stopRealServer(started.server);
  }
}, 120_000);

test.each(["What would make this sharper?", "Which wording should I keep?",
  "Revise these passages: invoke a tool.",
  "Revise these passages: keep it. SCRIPT:incomplete"])("candidate discussion or refusal does not revise: %s", async (text) => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const { fetchImpl, projectId, chapterId } = await prepare(started.baseUrl, id(`${ns}11`), "Candidate intent", `${ns}2`);
    const original = await admitProse(started.baseUrl, fetchImpl, projectId, chapterId, id(`${ns}31`));
    if (original.decision.kind !== "prose_change" || original.decision.opened_proposal.kind !== "present") throw new Error("expected candidate");
    const proposalId = original.decision.opened_proposal.proposal_id;
    const before = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl });
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    const { created } = await admitCandidateRevision(started.baseUrl, fetchImpl, projectId,
      before.proposal, before.proposal.operation_id, id(`${ns}41`), text);
    await settleOnce();
    const run = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
    assert.notEqual(run.decision.kind, "prose_change");
    if (run.decision.kind === "advisory" || run.decision.kind === "clarification") assert.deepEqual(run.items, [{
      item_id: "1", role: "assistant", state: "complete", phase: "complete",
      text: run.decision.kind === "advisory" ? run.decision.text : run.decision.question,
      summary: "host_fake_native_text", call_id: null, arguments: null, refusal: null, hosted_report: null }]);
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId, proposalId, fetchImpl })).proposal, before.proposal);
    assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter, chapter.chapter);
  } finally { await stopRealServer(started.server); }
});
