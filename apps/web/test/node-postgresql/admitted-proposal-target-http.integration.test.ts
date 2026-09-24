// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/open-block-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  applyAuthorEdit, createAgentRun, createEditorSession, deleteChapter,
  digestApplyAuthorEdit, digestCreateAgentRun, digestCreateEditorSession,
  digestDeleteChapter, getAgentRun, getChapter, getProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest, CreateAgentRunRequest, CreateEditorSessionRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, runStoryOSWorker,
  stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { BINDING, PROSE, challenged, drainLeftoverWork, id, prepare, settleOnce,
  startRealServer } from "../support/acceptance.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));

async function withProject(
  title: string,
  exercise: (project: Awaited<ReturnType<typeof prepare>> & { baseUrl: string; ns: string }) => Promise<void>,
) {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const prepared = await prepare(started.baseUrl, id(`${ns}01`), title, ns);
    await exercise({ ...prepared, baseUrl: started.baseUrl, ns });
  } finally { await stopRealServer(started.server); }
}

async function admitQueued(
  baseUrl: string, fetchImpl: typeof fetch, projectId: string,
  chapterId: string, key: string, message = "Revise this passage: keep the voice.",
): Promise<string> {
  const request: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" }, author_message: { text: message },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" }, cause: { kind: "author_request" }, ...BINDING,
      correlation_id: id(key.slice(-4)),
    },
  };
  const admitted = await challenged(
    baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema, await digestCreateAgentRun(request), key,
    (antiForgery) => createAgentRun({ baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request }),
  );
  if (admitted.effect.kind !== "admitted") throw new Error("expected admitted Run");
  return admitted.effect.run_id;
}

test.each([
  ["Revise this passage: keep the voice.", "red fox"],
  ["Revise this phrase: keep the voice.", PROSE],
  ["Stream this passage: keep the voice.", "red fox"],
  ["Revise these passages: keep the voice.", "red fox"],
])("an admitted H0 target cannot open a valid H1 Proposal: %s", async (message, initialText) => {
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
        editor_contract_revision: "storyos.editor-contract.release-1.v2",
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
    await runStoryOSWorker({ repositoryRoot,
      workerBinary: join(repositoryRoot, "target", "release-package", "storyos-worker"),
      args: ["--claim-only"], extraEnv: { STORYOS_EXPORT_LEASE_TTL_SECS: "0" },
    });
    await authorEdit("!", initialText.length, "c");
    const h1 = await getChapter({ baseUrl: baseUrl, projectId, chapterId, fetchImpl });
    assert.notEqual(h1.chapter.current_revision.revision_id, h0.chapter.current_revision.revision_id);

    await settleOnce();
    const run = await getAgentRun({ baseUrl, projectId, runId, fetchImpl });
    assert.equal(run.status, "completed");
    assert.equal(run.decision.kind, "prose_change");
    if (run.decision.kind !== "prose_change") throw new Error("expected prose decision");
    assert.deepEqual(run.decision.opened_proposal, { kind: "absent" });
    assert.equal(run.context.selected.find((source) => source.source_class === "working_target")?.content, initialText);
    assert.equal(run.context.selected.find((source) => source.source_class === "working_target")?.source_version, h0.chapter.current_revision.revision_id);
    assert.deepEqual(run.context.current_availability.working_target, {
      kind: "superseded", current_revision_id: h1.chapter.current_revision.revision_id,
    });
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.proposals
      WHERE project_id = '${projectId}'::uuid AND source_run_id = '${run.run_id}'::uuid;`), "0");
  });
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
    assert.equal(run.decision.kind, "prose_change");
    if (run.decision.kind !== "prose_change") throw new Error("expected prose decision");
    assert.deepEqual(run.decision.opened_proposal, { kind: "absent" });
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
    assert.equal(first.decision.kind, "prose_change");
    assert.equal(second.decision.kind, "prose_change");
    if (first.decision.kind !== "prose_change" || second.decision.kind !== "prose_change") {
      throw new Error("expected prose decisions");
    }
    assert.equal(first.decision.opened_proposal.kind, "present");
    assert.deepEqual(second.decision.opened_proposal, { kind: "absent" });
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
    await runStoryOSWorker({
      repositoryRoot,
      workerBinary: join(repositoryRoot, "target", "release-package", "storyos-worker"),
      args: ["--claim-only"], extraEnv: { STORYOS_EXPORT_LEASE_TTL_SECS: "0" },
    });
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
