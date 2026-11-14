// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/open-block-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { test } from "vitest";

import {
  applyAuthorEdit,
  createEditorSession,
  deleteChapter,
  digestApplyAuthorEdit,
  digestCreateEditorSession,
  digestDeleteChapter,
  getAgentRun,
  getChapter,
  getProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateEditorSessionRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres } from "../support/node-integration.ts";
import { BINDING, admitRun, challenged, drainLeftoverWork, id, inspectFirstRunStep, prepare, runWorker, serverForFile,
  settleOnce } from "../support/http-harness.ts";

const started = serverForFile();

async function withProject(
  title: string,
  exercise: (project: Awaited<ReturnType<typeof prepare>> & { baseUrl: string; ns: string }) => Promise<void>,
  server = started,
) {
  await drainLeftoverWork();
  const ns = randomBytes(3).toString("hex");
  const prepared = await prepare(server.baseUrl, id(`${ns}01`), title, ns);
  await exercise({ ...prepared, baseUrl: server.baseUrl, ns });
}

async function admitQueued(
  baseUrl: string, fetchImpl: typeof fetch, projectId: string,
  chapterId: string, key: string, message = "Revise this passage: keep the voice.",
): Promise<string> {
  return (await admitRun(baseUrl, { projectId, fetchImpl, chapterId }, key, message)).effect.run_id;
}

// A refused `propose_edit` call returns its reason to the model, and the final answer is advisory (ADR 0091 section 6).
test.each([
  ["Revise this passage: keep the voice.", "advisory", started],
  ["Revise these passages: keep the voice.", "advisory", started],
] as const)("an admitted H0 target cannot open a valid H1 Proposal: %s", async (message, kind, server) => {
  const initialText = "red fox";
  await withProject("Admitted Target Novel", async ({ baseUrl, ns, fetchImpl, projectId, chapterId }) => {
    let editorSession: Awaited<ReturnType<typeof createEditorSession>> | undefined;
    let latestRevisionId: string | undefined;
    let intentSequence = 0;
    async function authorEdit(text: string, position: number, suffix: string) {
      const sessionRequest: CreateEditorSessionRequest = {
        command_schema: "storyos.command.create-editor-session.request.v1",
        ...BINDING, correlation_id: id(`${ns}${suffix}1`),
      };
      editorSession ??= await challenged(
        baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/editor-sessions",
        sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest),
        id(`${ns}${suffix}2`),
        (antiForgery) => createEditorSession({
          baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}${suffix}2`),
          antiForgery, request: sessionRequest,
        }),
      );
      const session = editorSession;
      if (session.writer.kind !== "current_writer") throw new Error("expected current writer");
      const request: ApplyAuthorEditRequest = {
        command_schema: "storyos.command.apply-author-edit.request.v1",
        ...BINDING, correlation_id: id(`${ns}${suffix}3`),
        editor_session_id: session.editor_session.editor_session_id,
        writer_generation: session.writer.writer_generation,
        chapter_id: session.base_snapshot.chapter_id,
        expected_authoritative_revision_id: latestRevisionId ?? session.base_snapshot.authoritative_head_revision_id,
        expected_proposal_head_revision_ids: [],
        target_refs: session.base_snapshot.target_refs,
        observed_ownership_partition: "authoritative",
        editor_contract_revision: "storyos.editor-contract.release-1.v3",
        undo_group_id: id(`${ns}${suffix}4`),
        completed_intent_record_id: id(`${ns}${suffix}5`),
        local_intent_sequence: String(++intentSequence),
        author_edit_units: [{
          normalized_primitives: [{ kind: "replace_selection", from: position, to: position, text }],
          selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: position, to: position },
        }],
      };
      const result = await challenged(
        baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/manuscript/author-edits",
        request.command_schema, await digestApplyAuthorEdit(request), id(`${ns}${suffix}6`),
        (antiForgery) => applyAuthorEdit({
          baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}${suffix}6`), antiForgery, request,
        }),
      );
      assert.equal(result.effect.kind, "authoritative_applied");
      if (result.effect.kind === "authoritative_applied") latestRevisionId = result.effect.authoritative_revision.revision_id;
    }

    await authorEdit(initialText, 0, "a");
    const h0 = await getChapter({ baseUrl: baseUrl, projectId, chapterId, fetchImpl });
    const runId = await admitQueued(baseUrl, fetchImpl, projectId, chapterId, id(`${ns}b2`), message);
    await runWorker(["--claim-only"], { extraEnv: { STORYOS_EXPORT_LEASE_TTL_SECS: "0" } });
    await authorEdit("!", initialText.length, "c");
    const h1 = await getChapter({ baseUrl: baseUrl, projectId, chapterId, fetchImpl });
    assert.notEqual(h1.chapter.current_revision.revision_id, h0.chapter.current_revision.revision_id);

    await settleOnce();
    const run = await getAgentRun({ baseUrl, projectId, runId, fetchImpl });
    assert.equal(run.status, "completed");
    assert.equal(run.decision.kind, kind);
    // The default read shows the closing RunStep. The first RunStep sent the admitted H0 text.
    const { context } = await inspectFirstRunStep(baseUrl, { projectId, fetchImpl }, runId);
    assert.equal(context.selected.find((source) => source.source_class === "working_target")?.content, initialText);
    assert.equal(context.selected.find((source) => source.source_class === "working_target")?.source_version, h0.chapter.current_revision.revision_id);
    assert.deepEqual(context.current_availability.working_target, {
      kind: "superseded", current_revision_id: h1.chapter.current_revision.revision_id,
    });
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.proposals
      WHERE project_id = '${projectId}'::uuid AND source_run_id = '${run.run_id}'::uuid;`), "0");
  }, server);
});

test("a removed admitted Chapter cannot open a Proposal from retained historical blocks", async () => {
  await withProject("Removed Target Novel", async ({ baseUrl, ns, fetchImpl, projectId, chapterId }) => {
    const runId = await admitQueued(baseUrl, fetchImpl, projectId, chapterId, id(`${ns}12`));
    const removal = {
      command_schema: "storyos.command.delete-chapter.request.v1" as const,
      delete_chapter_input: { expected_tree_revision: "3", ...BINDING, correlation_id: id(`${ns}21`) },
    };
    const deleted = await challenged(
      baseUrl, fetchImpl, projectId, "DELETE",
      "/api/v1/projects/{project_id}/chapters/{chapter_id}",
      removal.command_schema, await digestDeleteChapter(removal), id(`${ns}22`),
      (antiForgery) => deleteChapter({
        baseUrl: baseUrl, projectId, chapterId, fetchImpl,
        idempotencyKey: id(`${ns}22`), antiForgery, request: removal,
      }),
    );
    assert.equal(deleted.effect.kind, "authoritative_applied");
    await settleOnce();
    const run = await getAgentRun({ baseUrl, projectId, runId, fetchImpl });
    // The Host refuses the `propose_edit` call. The closing RunStep assembles its context again, and the removed
    // Chapter blocks it (ADR 0085, ADR 0091).
    assert.deepEqual([run.status, run.decision], ["refused", { kind: "execution_refused", capability: "blocked_context" }]);
    assert.equal(await queryPostgres(`SELECT string_agg(call.function_name || ':' || call.state, ' ' ORDER BY call.call_position)
      FROM storyos.tool_calls AS call JOIN storyos.model_attempts AS attempt USING (owner_user_id, project_id, model_attempt_id)
      WHERE attempt.run_id = '${runId}'::uuid;`), "propose_edit:refused finish:settled");
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.proposals
      WHERE project_id = '${projectId}'::uuid AND source_run_id = '${runId}'::uuid;`), "0");
  });
});

test("a later reservation cannot become an admitted Run's Proposal", async () => {
  await withProject("Reserved Target Novel", async ({ baseUrl, ns, fetchImpl, projectId, chapterId }) => {
    const firstId = await admitQueued(baseUrl, fetchImpl, projectId, chapterId, id(`${ns}11`));
    const secondId = await admitQueued(baseUrl, fetchImpl, projectId, chapterId, id(`${ns}21`));
    await settleOnce();
    await settleOnce();
    const first = await getAgentRun({ baseUrl: baseUrl, projectId, runId: firstId, fetchImpl });
    const second = await getAgentRun({ baseUrl: baseUrl, projectId, runId: secondId, fetchImpl });
    if (first.decision.kind !== "prose_change") throw new Error("expected prose decision");
    assert.equal(first.decision.opened_proposal.kind, "present");
    // The reservation refuses the `propose_edit` call of the second Run, and its final answer is advisory (ADR 0091).
    assert.equal(second.decision.kind, "advisory");
  });
});

test("a historical single-Block Run reclaims and opens one H0 Proposal", async () => {
  await withProject("Historical Target Novel", async ({ baseUrl, ns, fetchImpl, projectId, chapterId }) => {
    const before = await getChapter({ baseUrl: baseUrl, projectId, chapterId, fetchImpl });
    const runId = await admitQueued(baseUrl, fetchImpl, projectId, chapterId, id(`${ns}12`));
    await queryPostgres(`
      UPDATE storyos.operation_requirements
         SET payload = payload #- '{operation_requirement,proposal_target_block_ids}'
       WHERE project_id = '${projectId}'::uuid
         AND run_id = '${runId}'::uuid;
    `);
    await runWorker(["--claim-only"], { extraEnv: { STORYOS_EXPORT_LEASE_TTL_SECS: "0" } });
    await settleOnce();
    await settleOnce();
    const run = await getAgentRun({ baseUrl, projectId, runId, fetchImpl });
    assert.equal(run.decision.kind, "prose_change");
    if (run.decision.kind !== "prose_change") throw new Error("expected prose decision");
    assert.equal(run.decision.opened_proposal.kind, "present");
    if (run.decision.opened_proposal.kind !== "present") throw new Error("expected Proposal");
    const proposal = await getProposal({ baseUrl, projectId,
      proposalId: run.decision.opened_proposal.proposal_id, fetchImpl });
    assert.equal(proposal.proposal.base_authoritative_revision_id, before.chapter.current_revision.revision_id);
    assert.equal(proposal.proposal.validation_receipt.kind, "present");
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.proposals
      WHERE project_id = '${projectId}'::uuid AND source_run_id = '${runId}'::uuid;`), "1");
  });
});
