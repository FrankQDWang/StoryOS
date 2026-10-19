// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { digestExportProjectArchive, digestUndoLatestAuthorAction, exportProjectArchive, getChapter,
  getExportOperation, getProposal, undoLatestAuthorAction,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { UndoLatestAuthorActionRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { BINDING, REVISED, accept, acceptRequest, advanceChapterHead, challenged, drainLeftoverWork, id,
  openedBlockProposal, reject, seedAcceptanceCondition, settleOnce, startRealServer } from "../support/acceptance.ts";

const AUTHOR_TEXT = "The author rewrote this paragraph after the Proposal opened.";

test("Acceptance on an earlier base replaces the Block text, survives a restart, and Author Undo restores the author text", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedBlockProposal(started, "e5a1");
    const { projectId, chapterId } = opened.prepared;
    const history = await opened.history();
    const block = opened.base.current_revision.blocks.find((candidate) =>
      candidate.manuscript_block_id === opened.revised.manuscript_block_id);
    if (!block) throw new Error("expected the target Block on H0");
    const h1 = id("e5a10051");
    await advanceChapterHead(projectId, chapterId, opened.base.current_revision.revision_id, h1,
      [{ manuscript_block_id: block.manuscript_block_id, text: AUTHOR_TEXT }]);
    const authored = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl: opened.prepared.fetchImpl });
    assert.equal(authored.chapter.current_revision.body, AUTHOR_TEXT);

    const { accepted, replay } = await accept(started, opened, acceptRequest(opened, h1, "e5a10061"), "e5a10062");
    if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
    assert.equal(accepted.effect.target_resolution, "block_replacement");
    assert.deepEqual(accepted.receipt.prior_authoritative_revision_ids, [h1]);
    assert.deepEqual(accepted.effect.authoritative_revision.blocks,
      [{ ...block, text: REVISED }]);
    assert.deepEqual(await replay(), accepted);
    assert.deepEqual(JSON.parse(await queryPostgres(`SELECT jsonb_build_object(
      'applied', applied_authoritative_revision_id, 'resolution', target_resolution)::text
      FROM storyos.acceptance_receipts WHERE acceptance_receipt_id = '${accepted.receipt.receipt_id}'::uuid`)),
    { applied: h1, resolution: "block_replacement" });

    await stopRealServer(started.server);
    started = await startRealServer();
    const fetchImpl = browserFetch(started.baseUrl, "session-a");
    const reloaded = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    assert.equal(reloaded.chapter.current_revision.body, REVISED);
    const inspected = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId: opened.revised.proposal_id, fetchImpl });
    assert.equal(inspected.proposal.operation_resolution, "applied");
    assert.equal(await opened.history(), history);

    const exportRequest = { command_schema: "storyos.command.export-project-archive.request.v1" as const,
      export_project_archive_input: { ...BINDING, correlation_id: id("e5a10071"),
        archive_profile: "storyos.project-export.v1",
        archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
    const archive = await challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/exports",
      exportRequest.command_schema, await digestExportProjectArchive(exportRequest), id("e5a10072"),
      (antiForgery) => exportProjectArchive({ baseUrl: started.baseUrl, projectId, fetchImpl,
        request: exportRequest, idempotencyKey: id("e5a10072"), antiForgery }));
    if (archive.effect.kind !== "admitted") throw new Error("expected admitted archive");
    await settleOnce();
    assert.equal((await getExportOperation({ baseUrl: started.baseUrl, projectId, fetchImpl,
      exportId: archive.effect.export_id })).status, "ready");
    const download = await fetchImpl(`${started.baseUrl}/api/v1/projects/${projectId}/exports/${archive.effect.export_id}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    assert.equal(download.status, 200);
    const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
    const acceptances = JSON.parse(new TextDecoder().decode(files.get("canonical/acceptance_receipts.json")));
    assert.deepEqual(acceptances.map((row: Record<string, unknown>) => [row.acceptance_receipt_id,
      row.applied_authoritative_revision_id, row.target_resolution]),
    [[accepted.receipt.receipt_id, h1, "block_replacement"]]);

    const undoRequest: UndoLatestAuthorActionRequest = {
      command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: {
        expected_author_undo_frontier_sequence: accepted.effect.author_action_sequence,
        expected_authoritative_revision_id: accepted.effect.authoritative_revision.revision_id,
        editor_session_id: opened.session.editor_session.editor_session_id, ...BINDING, correlation_id: id("e5a10081"),
      },
    };
    const undone = await challenged(started.baseUrl, fetchImpl, projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", undoRequest.command_schema,
      await digestUndoLatestAuthorAction(undoRequest), id("e5a10082"), (antiForgery) => undoLatestAuthorAction({
        baseUrl: started.baseUrl, projectId, fetchImpl, idempotencyKey: id("e5a10082"), antiForgery, request: undoRequest }));
    assert.equal(undone.effect.kind, "compensated");
    const restored = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    assert.equal(restored.chapter.current_revision.body, AUTHOR_TEXT);
  } finally {
    await stopRealServer(started.server);
  }
});

test("an author paragraph added after the Proposal opened does not stop Acceptance", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedBlockProposal(started, "e5a2");
    const block = opened.base.current_revision.blocks[0]!;
    const h1 = id("e5a20051");
    await advanceChapterHead(opened.prepared.projectId, opened.prepared.chapterId,
      opened.base.current_revision.revision_id, h1,
      [block, { manuscript_block_id: id("e5a20052"), text: AUTHOR_TEXT }]);
    const { accepted } = await accept(started, opened, acceptRequest(opened, h1, "e5a20061"), "e5a20062");
    if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
    assert.equal(accepted.effect.target_resolution, "block_replacement");
    assert.equal(accepted.effect.authoritative_revision.body, `${REVISED}\n${AUTHOR_TEXT}`);
  } finally {
    await stopRealServer(started.server);
  }
});

test("Acceptance refuses a Proposal whose target Block was deleted and changes nothing, then Reject applies", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedBlockProposal(started, "e5a3");
    const { projectId, chapterId, fetchImpl } = opened.prepared;
    const h1 = id("e5a30051");
    await advanceChapterHead(projectId, chapterId, opened.base.current_revision.revision_id, h1,
      [{ manuscript_block_id: id("e5a30052"), text: AUTHOR_TEXT }]);
    const chapter = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    const proposal = await getProposal({ baseUrl: started.baseUrl, projectId, proposalId: opened.revised.proposal_id, fetchImpl });
    const { accepted } = await accept(started, opened, acceptRequest(opened, h1, "e5a30061"), "e5a30062");
    assert.deepEqual(accepted.effect, { kind: "refused", reason: "missing_target" });
    assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter, chapter.chapter);
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId, proposalId: opened.revised.proposal_id,
      fetchImpl })).proposal, proposal.proposal);
    const rejected = await reject(started, opened, h1, "e5a3007");
    assert.equal(rejected.effect.kind, "resolved");
  } finally {
    await stopRealServer(started.server);
  }
});

test.each(["proposal_conflict", "proposal_recovery_conflict"] as const)(
  "a Proposal Revision with a %s condition on an earlier base", async (kind) => {
    const started = await startRealServer();
    try {
      await drainLeftoverWork();
      const ns = kind === "proposal_conflict" ? "e5a4" : "e5a5";
      const opened = await openedBlockProposal(started, ns);
      const { projectId, chapterId } = opened.prepared;
      const block = opened.base.current_revision.blocks[0]!;
      const h1 = id(`${ns}0051`);
      await advanceChapterHead(projectId, chapterId, opened.base.current_revision.revision_id, h1,
        [{ ...block, text: AUTHOR_TEXT }]);
      await seedAcceptanceCondition(started.baseUrl, opened.prepared.fetchImpl, projectId, opened.revised,
        opened.session.editor_session.editor_session_id, h1, kind, `${ns}0060`);
      const { accepted } = await accept(started, opened, acceptRequest(opened, h1, `${ns}0071`), `${ns}0072`);
      if (kind === "proposal_conflict") {
        if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
        assert.equal(accepted.effect.target_resolution, "block_replacement");
        assert.equal(accepted.effect.authoritative_revision.body, REVISED);
        return;
      }
      assert.deepEqual(accepted.effect, { kind: "refused", reason: "not_eligible" });
      const rejected = await reject(started, opened, h1, `${ns}008`);
      assert.equal(rejected.effect.kind, "resolved");
      assert.equal((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId,
        fetchImpl: opened.prepared.fetchImpl })).chapter.current_revision.body, AUTHOR_TEXT);
    } finally {
      await stopRealServer(started.server);
    }
  });
