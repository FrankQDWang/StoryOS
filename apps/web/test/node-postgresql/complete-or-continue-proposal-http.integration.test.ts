// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/stream-proposal-generation-http.integration.test.ts"]}
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
  revisionCount: string;
}> {
  const row = await queryStoryOSPostgres(
    `SELECT g.generation_id::text, g.last_applied_stream_seq::text, r.status,
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
          = (p.owner_user_id, p.project_id, p.source_run_id)
      WHERE p.proposal_id = '${proposalId}'`,
  );
  const [generationId, streamSeq, runStatus, revisionCount] = row.split("|");
  return { generationId, streamSeq, runStatus, revisionCount };
}

test("complete keeps the partial candidate ready and continue opens a fresh Generation", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = freshNs();
    const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Complete Novel", ns);
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
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
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
    assert.equal(paused.proposal.candidate_text, FIRST);
    const facts = await generationFacts(paused.proposal.proposal_id);
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
    const completeOptions = {
      baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
      proposalId: paused.proposal.proposal_id, idempotencyKey: id(`${ns}72`), request: completeRequest,
      antiForgery: "",
    };
    const completed = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-completions",
      completeRequest.command_schema, await digestCompleteReadyPartialProposal(completeRequest),
      completeOptions.idempotencyKey,
      (antiForgery) => {
        completeOptions.antiForgery = antiForgery;
        return completeReadyPartialProposal(completeOptions);
      },
    );
    assert.equal(completed.effect.kind, "completed");
    if (completed.effect.kind !== "completed") throw new Error("expected completed");
    assert.equal(completed.effect.generation_id, facts.generationId);
    assert.equal(completed.effect.prior_generation_state, "ready_partial");
    assert.equal(completed.effect.resulting_generation_state, "ready");
    assert.equal(completed.effect.preserved_validation, paused.proposal.validation);
    assert.equal(completed.effect.preserved_closure, paused.proposal.closure);
    assert.equal(completed.effect.preserved_operation_resolution, paused.proposal.operation_resolution);
    assert.deepEqual(completed.receipt.authoritative_commit_ids, []);
    assert.deepEqual(await completeReadyPartialProposal(completeOptions), completed);
    const ready = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: paused.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(ready.proposal.generation, "ready");
    assert.equal(ready.proposal.revision_id, paused.proposal.revision_id);
    assert.equal(ready.proposal.candidate_text, FIRST);
    assert.equal(ready.proposal.operation_resolution, "pending");
    assert.notEqual(ready.proposal.candidate_text, PROSE);
    const afterComplete = await generationFacts(paused.proposal.proposal_id);
    assert.equal(afterComplete.generationId, facts.generationId);
    assert.equal(afterComplete.revisionCount, facts.revisionCount);
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
    const continueOptions = {
      baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
      proposalId: ready.proposal.proposal_id, idempotencyKey: id(`${ns}82`), request: continueRequest,
      antiForgery: "",
    };
    const continued = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-continuations",
      continueRequest.command_schema, await digestContinueProposalGeneration(continueRequest),
      continueOptions.idempotencyKey,
      (antiForgery) => {
        continueOptions.antiForgery = antiForgery;
        return continueProposalGeneration(continueOptions);
      },
    );
    assert.equal(continued.effect.kind, "started");
    if (continued.effect.kind !== "started") throw new Error("expected started");
    assert.equal(continued.effect.prior_generation_id, facts.generationId);
    assert.notEqual(continued.effect.new_generation_id, facts.generationId);
    assert.equal(continued.effect.resulting_generation_state, "generating");
    assert.equal(continued.effect.prior_run_id, created.effect.run_id);
    assert.deepEqual(continued.receipt.authoritative_commit_ids, []);
    assert.deepEqual(await continueProposalGeneration(continueOptions), continued);
    const generating = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: ready.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(generating.proposal.generation, "generating");
    assert.equal(generating.proposal.revision_id, ready.proposal.revision_id);
    assert.equal(generating.proposal.candidate_text, FIRST);
    assert.equal(generating.proposal.operation_resolution, "pending");
    assert.equal(generating.proposal.validation, ready.proposal.validation);
    assert.equal(generating.proposal.closure, ready.proposal.closure);
    const head = await generationFacts(ready.proposal.proposal_id);
    assert.equal(head.generationId, continued.effect.new_generation_id);
    assert.equal(head.streamSeq, "0");
    assert.equal(head.revisionCount, facts.revisionCount);
    const copiedEvents = await queryStoryOSPostgres(
      `SELECT count(*)::text FROM storyos.proposal_stream_events
        WHERE generation_id = '${continued.effect.new_generation_id}'`,
    );
    assert.equal(copiedEvents, "0");
    const terminal = facts.runStatus === "completed" || facts.runStatus === "refused" || facts.runStatus === "cancelled";
    if (terminal) {
      assert.notEqual(continued.effect.resulting_run_id, continued.effect.prior_run_id);
      const successor = await queryStoryOSPostgres(
        `SELECT status || '|' || predecessor_run_id::text || '|' || wakeup_pending::text
           FROM storyos.agent_runs WHERE run_id = '${continued.effect.resulting_run_id}'`,
      );
      assert.equal(successor, `queued|${continued.effect.prior_run_id}|true`);
      const assembly = await queryStoryOSPostgres(
        `SELECT count(*)::text FROM storyos.context_assembly_manifests
          WHERE run_id = '${continued.effect.resulting_run_id}'`,
      );
      assert.equal(assembly, "1");
      const priorEvents = await queryStoryOSPostgres(
        `SELECT count(*)::text FROM storyos.proposal_stream_events
          WHERE generation_id = '${facts.generationId}'`,
      );
      await settleOnce();
      const continuedEvents = await queryStoryOSPostgres(
        `SELECT count(*)::text || '|' || COALESCE(max(stream_seq)::text, '0')
           FROM storyos.proposal_stream_events
          WHERE generation_id = '${continued.effect.new_generation_id}'`,
      );
      assert.equal(continuedEvents, "1|1");
      const priorEventsAfter = await queryStoryOSPostgres(
        `SELECT count(*)::text FROM storyos.proposal_stream_events
          WHERE generation_id = '${facts.generationId}'`,
      );
      assert.equal(priorEventsAfter, priorEvents);
      const stillOneProposal = await queryStoryOSPostgres(
        `SELECT count(*)::text FROM storyos.proposals
          WHERE proposal_id = '${ready.proposal.proposal_id}'`,
      );
      assert.equal(stillOneProposal, "1");
      const advanced = await generationFacts(ready.proposal.proposal_id);
      assert.equal(advanced.generationId, continued.effect.new_generation_id);
      assert.equal(advanced.streamSeq, "1");
      let finished = generating;
      for (let attempt = 0; attempt < 6 && finished.proposal.generation !== "ready"; attempt += 1) {
        await settleOnce();
        finished = await getProposal({
          baseUrl: started.baseUrl, projectId: prepared.projectId,
          proposalId: ready.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        });
      }
      assert.equal(finished.proposal.generation, "ready");
      const againRequest: ContinueProposalGenerationRequest = {
        command_schema: "storyos.command.continue-proposal-generation.request.v1",
        continue_proposal_generation_input: {
          proposal_revision_id: finished.proposal.revision_id,
          prior_generation_id: continued.effect.new_generation_id,
          expected_generation_state: "ready",
          expected_candidate_digest: candidateDigest(finished.proposal.candidate_text),
          selected_pending_operation_ids: [finished.proposal.operation_id],
          expected_target_revisions: [session.base_snapshot.authoritative_head_revision_id],
          editor_session_id: session.editor_session.editor_session_id,
          ...BINDING,
          correlation_id: id(`${ns}83`),
        },
      };
      const againOptions = {
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
        proposalId: ready.proposal.proposal_id, idempotencyKey: id(`${ns}84`), request: againRequest,
        antiForgery: "",
      };
      const again = await challenged(
        started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-continuations",
        againRequest.command_schema, await digestContinueProposalGeneration(againRequest),
        againOptions.idempotencyKey,
        (antiForgery) => {
          againOptions.antiForgery = antiForgery;
          return continueProposalGeneration(againOptions);
        },
      );
      assert.equal(again.effect.kind, "started");
      if (again.effect.kind !== "started") throw new Error("expected started");
      assert.equal(again.effect.prior_run_id, continued.effect.resulting_run_id);
      assert.notEqual(again.effect.resulting_run_id, again.effect.prior_run_id);
      const linked = await queryStoryOSPostgres(
        `SELECT predecessor_run_id::text FROM storyos.agent_runs
          WHERE run_id = '${again.effect.resulting_run_id}'`,
      );
      assert.equal(linked, continued.effect.resulting_run_id);
    } else {
      assert.equal(continued.effect.resulting_run_id, continued.effect.prior_run_id);
    }
    const after = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    await drainLeftoverWork();
  } finally {
    await stopRealServer(started.server);
  }
});
