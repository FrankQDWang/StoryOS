// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/continue-proposal-generation-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { test } from "vitest";

import {
  applyAuthorEdit,
  completeReadyPartialProposal,
  continueProposalGeneration,
  createAgentRun,
  createEditorSession,
  digestApplyAuthorEdit,
  digestCompleteReadyPartialProposal,
  digestContinueProposalGeneration,
  digestCreateAgentRun,
  digestCreateEditorSession,
  getAgentRun,
  getChapter,
  getProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CompleteReadyPartialProposalRequest,
  ContinueProposalGenerationRequest,
  CreateAgentRunRequest,
  CreateEditorSessionRequest,
  GetProposalResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";
import {
  BINDING, PROSE, challenged, drainLeftoverWork, id, prepare, settleOnce,
  startRealServer,
} from "../support/acceptance.ts";

const FIRST = "Guard";

function freshNs(): string {
  return randomBytes(4).toString("hex");
}

function candidateDigest(text: string): string {
  return createHash("sha256").update(text).digest("hex");
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

async function generationFacts(proposalId: string): Promise<{
  generationId: string;
  streamSeq: string;
  runStatus: string;
  runId: string;
  revisionCount: string;
}> {
  const row = await queryStoryOSPostgres(
    `SELECT g.generation_id::text, g.last_applied_stream_seq::text, r.status, r.run_id::text,
            (SELECT count(*)::text FROM storyos.proposal_revisions AS revision
              WHERE revision.proposal_id = p.proposal_id)
       FROM storyos.proposals AS p
       JOIN storyos.proposal_generation_heads AS h
         ON (h.owner_user_id, h.project_id, h.proposal_id)
          = (p.owner_user_id, p.project_id, p.proposal_id)
       JOIN storyos.proposal_generations AS g
         ON (g.owner_user_id, g.project_id, g.generation_id)
          = (h.owner_user_id, h.project_id, h.generation_id)
       JOIN storyos.agent_runs AS r
         ON (r.owner_user_id, r.project_id, r.run_id)
          = (p.owner_user_id, p.project_id, COALESCE(g.run_id, p.source_run_id))
      WHERE p.proposal_id = '${proposalId}'`,
  );
  const [generationId, streamSeq, runStatus, runId, revisionCount] = row.split("|");
  if (!generationId || !streamSeq || !runStatus || !runId || !revisionCount) {
    throw new Error(`generation facts were incomplete: ${row}`);
  }
  return { generationId, streamSeq, runStatus, runId, revisionCount };
}

async function runFacts(runId: string): Promise<{
  status: string;
  predecessor: string;
  wakeupPending: string;
}> {
  const row = await queryStoryOSPostgres(
    `SELECT status, COALESCE(predecessor_run_id::text, ''), wakeup_pending
       FROM storyos.agent_runs
      WHERE run_id = '${runId}'`,
  );
  const [status, predecessor, wakeupPending] = row.split("|");
  if (!status || wakeupPending === undefined) {
    throw new Error(`run facts were incomplete: ${row}`);
  }
  return { status, predecessor: predecessor ?? "", wakeupPending };
}

async function streamEventCount(generationId: string): Promise<string> {
  const count = await queryStoryOSPostgres(
    `SELECT count(*)::text FROM storyos.proposal_stream_events
      WHERE generation_id = '${generationId}'`,
  );
  if (!count) throw new Error("stream event count was empty");
  return count.trim();
}

async function assemblyCount(runId: string): Promise<string> {
  const count = await queryStoryOSPostgres(
    `SELECT count(*)::text FROM storyos.context_assembly_manifests
      WHERE run_id = '${runId}'`,
  );
  if (!count) throw new Error("assembly count was empty");
  return count.trim();
}

test("Worker claims a Continue successor and writes the next batch into the new Generation", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = freshNs();
    const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Claim Novel", ns);
    const before = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl,
    });
    const request = streamRequest(prepared.chapterId, id(`${ns}12`));
    const created = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/agent-runs",
      request.command_schema, await digestCreateAgentRun(request), id(`${ns}31`),
      (antiForgery) => createAgentRun({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id(`${ns}31`), antiForgery, request,
      }),
    );
    if (created.effect.kind !== "admitted") throw new Error("expected admitted");
    await settleOnce();
    let opened: GetProposalResponse | undefined;
    for (let attempt = 0; attempt < 250; attempt += 1) {
      const queried = await getAgentRun({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        runId: created.effect.run_id, fetchImpl: prepared.fetchImpl,
      });
      if (queried.decision.kind === "prose_change" && queried.decision.opened_proposal.kind === "present") {
        const current = await getProposal({
          baseUrl: started.baseUrl, projectId: prepared.projectId,
          proposalId: queried.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        });
        if (current.proposal.generation === "generating" && current.proposal.candidate_text === FIRST) {
          opened = current;
          break;
        }
      }
      await new Promise((resolve) => setTimeout(resolve, 20));
    }
    if (!opened) throw new Error("streamed Proposal did not reach the first batch");
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
    await settleOnce();
    const paused = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(paused.proposal.generation, "ready_partial");
    const facts = await generationFacts(paused.proposal.proposal_id);
    if (facts.runStatus !== "completed" && facts.runStatus !== "refused" && facts.runStatus !== "cancelled") {
      throw new Error(`expected a terminal source Run, got ${facts.runStatus}`);
    }
    const priorEvents = await streamEventCount(facts.generationId);
    const completeRequest: CompleteReadyPartialProposalRequest = {
      command_schema: "storyos.command.complete-ready-partial-proposal.request.v1",
      complete_ready_partial_proposal_input: {
        proposal_revision_id: paused.proposal.revision_id,
        generation_id: facts.generationId,
        expected_candidate_digest: candidateDigest(FIRST),
        last_applied_stream_seq: facts.streamSeq,
        expected_target_revisions: [session.base_snapshot.authoritative_head_revision_id],
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id(`${ns}71`),
      },
    };
    await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-completions",
      completeRequest.command_schema, await digestCompleteReadyPartialProposal(completeRequest),
      id(`${ns}72`),
      (antiForgery) => completeReadyPartialProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        proposalId: paused.proposal.proposal_id, idempotencyKey: id(`${ns}72`),
        request: completeRequest, antiForgery,
      }),
    );
    const ready = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: paused.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(ready.proposal.generation, "ready");
    const continueRequest: ContinueProposalGenerationRequest = {
      command_schema: "storyos.command.continue-proposal-generation.request.v1",
      continue_proposal_generation_input: {
        proposal_revision_id: ready.proposal.revision_id,
        prior_generation_id: facts.generationId,
        expected_generation_state: "ready",
        expected_candidate_digest: candidateDigest(FIRST),
        selected_pending_operation_ids: [ready.proposal.operation_id],
        expected_target_revisions: [session.base_snapshot.authoritative_head_revision_id],
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id(`${ns}81`),
      },
    };
    const continued = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-continuations",
      continueRequest.command_schema, await digestContinueProposalGeneration(continueRequest),
      id(`${ns}82`),
      (antiForgery) => continueProposalGeneration({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        proposalId: ready.proposal.proposal_id, idempotencyKey: id(`${ns}82`),
        request: continueRequest, antiForgery,
      }),
    );
    assert.equal(continued.effect.kind, "started");
    if (continued.effect.kind !== "started") throw new Error("expected started");
    assert.notEqual(continued.effect.resulting_run_id, facts.runId);
    const successor = await runFacts(continued.effect.resulting_run_id);
    assert.equal(successor.predecessor, facts.runId);
    assert.equal(await assemblyCount(continued.effect.resulting_run_id), "1");
    const prior = await runFacts(facts.runId);
    assert.equal(prior.status, facts.runStatus);
    await drainLeftoverWork();
    const streamed = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: ready.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(streamed.proposal.candidate_text, PROSE);
    assert.notEqual(streamed.proposal.revision_id, ready.proposal.revision_id);
    const after = await generationFacts(ready.proposal.proposal_id);
    assert.equal(after.generationId, continued.effect.new_generation_id);
    assert.notEqual(after.streamSeq, "0");
    assert.notEqual(await streamEventCount(after.generationId), "0");
    assert.equal(await streamEventCount(facts.generationId), priorEvents);
    const settledSuccessor = await runFacts(continued.effect.resulting_run_id);
    assert.equal(settledSuccessor.status, "completed");
    assert.equal(settledSuccessor.predecessor, facts.runId);
    const settledPrior = await runFacts(facts.runId);
    assert.equal(settledPrior.status, facts.runStatus);
    const secondRequest: ContinueProposalGenerationRequest = {
      command_schema: "storyos.command.continue-proposal-generation.request.v1",
      continue_proposal_generation_input: {
        proposal_revision_id: streamed.proposal.revision_id,
        prior_generation_id: after.generationId,
        expected_generation_state: "ready",
        expected_candidate_digest: candidateDigest(PROSE),
        selected_pending_operation_ids: [streamed.proposal.operation_id],
        expected_target_revisions: [session.base_snapshot.authoritative_head_revision_id],
        editor_session_id: session.editor_session.editor_session_id,
        ...BINDING,
        correlation_id: id(`${ns}91`),
      },
    };
    const second = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-continuations",
      secondRequest.command_schema, await digestContinueProposalGeneration(secondRequest),
      id(`${ns}92`),
      (antiForgery) => continueProposalGeneration({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        proposalId: ready.proposal.proposal_id, idempotencyKey: id(`${ns}92`),
        request: secondRequest, antiForgery,
      }),
    );
    assert.equal(second.effect.kind, "started");
    if (second.effect.kind !== "started") throw new Error("expected started");
    assert.equal(second.effect.prior_run_id, continued.effect.resulting_run_id);
    assert.notEqual(second.effect.prior_run_id, facts.runId);
    const chapter = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(chapter.chapter, before.chapter);
    await drainLeftoverWork();
  } finally {
    await stopRealServer(started.server);
  }
});
