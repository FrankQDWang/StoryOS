// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { createEditorSession, digestCreateEditorSession, digestExportProjectArchive, digestUndoLatestAuthorAction,
  exportProjectArchive, getChapter, getExportOperation, getProposal, undoLatestAuthorAction,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateEditorSessionRequest, UndoLatestAuthorActionRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { BINDING, REVISED, accept, acceptRequest, admitProse, advanceChapterHead, challenged, drainLeftoverWork, id,
  openedBlockProposal, prepare, reject, seedAcceptanceCondition, sendAuthorEdit, settleOnce, startRealServer,
} from "../support/acceptance.ts";
import type { Started } from "../support/acceptance.ts";

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

test.each(["acknowledged", "authoritative"] as const)(
  "an author edit of the reserved target Block with %s ownership leaves the Chapter Head and the Proposal unchanged",
  async (ownership) => {
    const started = await startRealServer();
    try {
      await drainLeftoverWork();
      const ns = ownership === "acknowledged" ? "f5a1" : "f5a2";
      const opened = await openedBlockProposal(started, ns);
      const { projectId, chapterId, fetchImpl } = opened.prepared;
      const chapter = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
      const edited = await sendAuthorEdit(started, opened, { kind: "replace_block_selection",
        manuscript_block_id: opened.revised.manuscript_block_id, from: 0, to: 0, text: "Edit " },
      ownership === "acknowledged" ? { kind: "acknowledged", proposal: opened.revised } : { kind: "authoritative" },
      "2", `${ns}006`);
      assert.deepEqual(edited.effect, { kind: "conflicted",
        reason: ownership === "acknowledged" ? "ownership_changed" : "proposal_head_present",
        current_authoritative_revision_id: chapter.chapter.current_revision.revision_id });
      assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter,
        chapter.chapter);
      assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId, fetchImpl,
        proposalId: opened.revised.proposal_id })).proposal, opened.revised);
    } finally {
      await stopRealServer(started.server);
    }
  });

/** A Block Proposal on the first of two Blocks that real author edits wrote before the Proposal opened. */
async function openedOnTwoBlocks(started: Started, ns: string) {
  const prepared = await prepare(started.baseUrl, id(`${ns}0011`), "Two Block Novel", `${ns}02`);
  const { projectId, chapterId, fetchImpl } = prepared;
  const sessionRequest: CreateEditorSessionRequest = { command_schema: "storyos.command.create-editor-session.request.v1",
    ...BINDING, correlation_id: id(`${ns}0031`) };
  const session = await challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/editor-sessions",
    sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest), id(`${ns}0032`),
    (antiForgery) => createEditorSession({ baseUrl: started.baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}0032`),
      antiForgery, request: sessionRequest }));
  const context = { prepared, session };
  const written = await sendAuthorEdit(started, context, { kind: "replace_selection", from: 0, to: 0, text: "Hello World" },
    { kind: "authoritative" }, "1", `${ns}004`);
  if (written.effect.kind !== "authoritative_applied") throw new Error("expected written prose");
  const split = await sendAuthorEdit(started, context, { kind: "split_block", offset: 6,
    manuscript_block_id: written.effect.authoritative_revision.blocks[0]!.manuscript_block_id,
    new_manuscript_block_id: id(`${ns}0051`) }, { kind: "authoritative" }, "2", `${ns}005`);
  if (split.effect.kind !== "authoritative_applied") throw new Error("expected two Blocks");
  const queried = await admitProse(started.baseUrl, fetchImpl, projectId, chapterId, id(`${ns}0061`));
  if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened prose");
  }
  const revised = (await getProposal({ baseUrl: started.baseUrl, projectId, fetchImpl,
    proposalId: queried.decision.opened_proposal.proposal_id })).proposal;
  if (revised.validation_receipt.kind !== "present") throw new Error("expected validation");
  const base = (await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter;
  return { prepared, session, revised, base, history: async () => "",
    validationReceiptId: revised.validation_receipt.validation_receipt_id };
}

test("an author edit of another Block moves the head, Acceptance applies on it, and Author Undo restores it", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const opened = await openedOnTwoBlocks(started, "f5b1");
    const { projectId, chapterId, fetchImpl } = opened.prepared;
    const [target, other] = opened.base.current_revision.blocks;
    if (!target || !other) throw new Error("expected two Blocks");
    assert.equal(opened.revised.manuscript_block_id, target.manuscript_block_id);
    assert.equal(opened.revised.base_authoritative_revision_id, opened.base.current_revision.revision_id);
    const edited = await sendAuthorEdit(started, opened, { kind: "replace_block_selection",
      manuscript_block_id: other.manuscript_block_id, from: other.text.length, to: other.text.length, text: " again" },
    { kind: "authoritative" }, "3", "f5b1007");
    if (edited.effect.kind !== "authoritative_applied") throw new Error(`expected applied, got ${JSON.stringify(edited.effect)}`);
    const h1 = edited.effect.authoritative_revision;

    const { accepted } = await accept(started, opened, acceptRequest(opened, h1.revision_id, "f5b10081"), "f5b10082");
    if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
    assert.equal(accepted.effect.target_resolution, "block_replacement");
    assert.deepEqual(accepted.receipt.prior_authoritative_revision_ids, [h1.revision_id]);
    assert.deepEqual(accepted.effect.authoritative_revision.blocks,
      [{ ...target, text: opened.revised.candidate_text }, { ...other, text: `${other.text} again` }]);
    assert.deepEqual(JSON.parse(await queryPostgres(`SELECT jsonb_build_object(
      'applied', applied_authoritative_revision_id, 'resolution', target_resolution)::text
      FROM storyos.acceptance_receipts WHERE acceptance_receipt_id = '${accepted.receipt.receipt_id}'::uuid`)),
    { applied: h1.revision_id, resolution: "block_replacement" });

    const undoRequest: UndoLatestAuthorActionRequest = {
      command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: {
        expected_author_undo_frontier_sequence: accepted.effect.author_action_sequence,
        expected_authoritative_revision_id: accepted.effect.authoritative_revision.revision_id,
        editor_session_id: opened.session.editor_session.editor_session_id, ...BINDING, correlation_id: id("f5b10091"),
      },
    };
    const undone = await challenged(started.baseUrl, fetchImpl, projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", undoRequest.command_schema,
      await digestUndoLatestAuthorAction(undoRequest), id("f5b10092"), (antiForgery) => undoLatestAuthorAction({
        baseUrl: started.baseUrl, projectId, fetchImpl, idempotencyKey: id("f5b10092"), antiForgery, request: undoRequest }));
    assert.equal(undone.effect.kind, "compensated");
    assert.equal((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl }))
      .chapter.current_revision.body, h1.body);
  } finally {
    await stopRealServer(started.server);
  }
});
