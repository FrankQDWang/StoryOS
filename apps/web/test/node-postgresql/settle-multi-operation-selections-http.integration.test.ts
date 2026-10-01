// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { createHash, randomBytes } from "node:crypto";
import { test, vi } from "vitest";
import { execFile } from "node:child_process";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  acceptProposal, applyAuthorEdit, cancelAgentRun, createAgentRun, createChapter, createEditorSession,
  digestAcceptProposal, digestApplyAuthorEdit, digestCancelAgentRun, digestCreateAgentRun, digestCreateChapter,
  digestCreateEditorSession, digestExportProjectArchive, digestReplanProposal,
  exportProjectArchive, getAgentRun, getChapter, getExportOperation, getManuscriptTree, getEditorSession, getProposal, pauseAgentRun, digestPauseAgentRun, deleteChapter, digestDeleteChapter, rejectProposalOperations, digestRejectProposalOperations, replanProposal, setCurrentChapter, digestSetCurrentChapter,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  AcceptProposalRequest, ApplyAuthorEditRequest, CancelAgentRunRequest, CreateAgentRunRequest,
  CreateEditorSessionRequest, GetProposalResponse, ReplanProposalRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, sessionFetch as browserFetch, requireStoryOSProtocolError,
  stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { canonicalDraftValue } from "../../src/refused-edit-discard.ts";
import { zipStoreFiles } from "../support/archive.ts";
import { BINDING, PROSE, USER_A, challenged, drainLeftoverWork, id, prepare, settleOnce,
  startRealServer } from "../support/acceptance.ts";

const SECOND_PROSE = "Keep the second block voice in this passage.";

async function seedTwoBlocks(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  ns: string,
  singleText?: string,
  currentSession?: Awaited<ReturnType<typeof createEditorSession>>,
) {
  const sessionRequest: CreateEditorSessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}1`),
  };
  const session = currentSession ?? await challenged(
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
  const insertRequest: ApplyAuthorEditRequest = {
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
    local_intent_sequence: currentSession ? "3" : "1",
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from: 0, to: 0, text: singleText ?? "Hello World" }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 0,
      },
    }],
  };
  const inserted = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/manuscript/author-edits",
    insertRequest.command_schema, await digestApplyAuthorEdit(insertRequest), id(`${ns}6`),
    (antiForgery) => applyAuthorEdit({
      baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}6`), antiForgery,
      request: insertRequest,
    }),
  );
  if (inserted.effect.kind !== "authoritative_applied") {
    throw new Error("expected inserted passage");
  }
  if (singleText !== undefined) return { session, revisionId: inserted.effect.authoritative_revision.revision_id,
    secondBlockId: inserted.effect.authoritative_revision.blocks[0]!.manuscript_block_id };
  const firstBlock = inserted.effect.authoritative_revision.blocks[0];
  if (!firstBlock) throw new Error("expected first Block");
  const splitRequest: ApplyAuthorEditRequest = {
    ...insertRequest,
    correlation_id: id(`${ns}7`),
    expected_authoritative_revision_id: inserted.effect.authoritative_revision.revision_id,
    undo_group_id: id(`${ns}8`),
    completed_intent_record_id: id(`${ns}9`),
    local_intent_sequence: "2",
    author_edit_units: [{
      normalized_primitives: [{
        kind: "split_block",
        manuscript_block_id: firstBlock.manuscript_block_id,
        offset: 6,
        new_manuscript_block_id: id(`${ns}b2`),
      }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 6, to: 6,
      },
    }],
  };
  const split = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/manuscript/author-edits",
    splitRequest.command_schema, await digestApplyAuthorEdit(splitRequest), id(`${ns}a`),
    (antiForgery) => applyAuthorEdit({
      baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}a`), antiForgery,
      request: splitRequest,
    }),
  );
  if (split.effect.kind !== "authoritative_applied") {
    throw new Error("expected split Blocks");
  }
  assert.equal(split.effect.authoritative_revision.blocks.length, 2);
  return {
    session,
    revisionId: split.effect.authoritative_revision.revision_id,
    secondBlockId: split.effect.authoritative_revision.blocks[1]!.manuscript_block_id,
  };
}

async function admitPassages(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  text: string,
  key: string,
  beforeSettle?: () => Promise<void>,
  settle = true,
  target?: CreateAgentRunRequest["create_agent_run_input"]["working_target"],
) {
  const request: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" },
      author_message: { text },
      working_target: target ?? { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: id(key.slice(-4)),
    },
  };
  const created = await challenged(
    baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema, await digestCreateAgentRun(request), key,
    (antiForgery) => createAgentRun({
      baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request,
    }),
  );
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  await beforeSettle?.();
  if (settle) await settleOnce();
  return getAgentRun({ baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
}

async function collectionSetup(baseUrl: string, ns: string, secondText = "A lantern crossed the river.") {
  const { fetchImpl, projectId, chapterId } = await prepare(baseUrl, id(`${ns}11`), "Chapter collection", `${ns}2`);
  const first = await seedTwoBlocks(baseUrl, fetchImpl, projectId, `${ns}3`);
  const tree = await getManuscriptTree({ baseUrl: baseUrl, projectId, fetchImpl });
  const volumeId = tree.volumes[0]!.volume_id;
  const request = { command_schema: "storyos.command.create-chapter.request.v1",
    create_chapter_input: { title: "Second target", expected_tree_revision: tree.tree_revision,
    ...BINDING, correlation_id: id(`${ns}41`) } };
  const created = await challenged(baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters", request.command_schema,
    await digestCreateChapter(request), id(`${ns}42`), (antiForgery) => createChapter({
    baseUrl: baseUrl, projectId, volumeId, fetchImpl, request, antiForgery, idempotencyKey: id(`${ns}42`) }));
  if (created.effect.kind !== "authoritative_applied") throw new Error("expected second Chapter");
  const switchChapter = async (from: string, to: string, sessionId: string, key: string) => {
    const target = await getChapter({ baseUrl: baseUrl, projectId, chapterId: to, fetchImpl });
    const request = { command_schema: "storyos.command.set-current-chapter.request.v1",
    set_current_chapter_input: { chapter_id: to, expected_current_chapter_id: from,
      expected_target_revision_id: target.chapter.current_revision.revision_id, editor_session_id: sessionId,
      ...BINDING, correlation_id: key } };
    await challenged(baseUrl, fetchImpl, projectId, "PUT",
    "/api/v1/projects/{project_id}/current-chapter", request.command_schema,
    await digestSetCurrentChapter(request), key, (antiForgery) => setCurrentChapter({
      baseUrl: baseUrl, projectId, fetchImpl, request, antiForgery, idempotencyKey: key }));
  };
  await switchChapter(chapterId, created.effect.chapter_id, first.session.editor_session.editor_session_id, id(`${ns}44`));
  const current = await getEditorSession({ baseUrl: baseUrl, projectId, fetchImpl,
    editorSessionId: first.session.editor_session.editor_session_id });
  const second = await seedTwoBlocks(baseUrl, fetchImpl, projectId, `${ns}5`, secondText, current);
  await switchChapter(created.effect.chapter_id, chapterId, second.session.editor_session.editor_session_id, id(`${ns}45`));
  const chapters = await Promise.all([chapterId, created.effect.chapter_id].map((chapterId) =>
    getChapter({ baseUrl: baseUrl, projectId, chapterId, fetchImpl })));
  const target = { kind: "passage_collection", source_chapter_id: chapterId,
    targets: chapters.map(({ chapter }) => ({ chapter_id: chapter.chapter_id,
    base_authoritative_revision_id: chapter.current_revision.revision_id,
    manuscript_block_ids: chapter.current_revision.blocks.map((block) => block.manuscript_block_id) }))
  } satisfies CreateAgentRunRequest["create_agent_run_input"]["working_target"];
  return { fetchImpl, projectId, chapterId, chapters, target,
    editorSessionId: first.session.editor_session.editor_session_id };
}

test.each(["", " SCRIPT:reverse_locations"])("one collection request produces three exact locations across two Chapters%s", async (script) => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const { fetchImpl, projectId, chapterId, chapters, target } = await collectionSetup(started.baseUrl, ns);
    const run = await admitPassages(started.baseUrl, fetchImpl, projectId, chapterId,
      `Revise these passages: keep the voice.${script}`, id(`${ns}43`), undefined, true, target);
    if (run.decision.kind !== "prose_change") throw new Error("expected collection Decision");
    assert.equal(run.decision.locations?.length, 3);
    assert.deepEqual(run.decision.locations!.map(({ chapter_id, manuscript_block_id, base_authoritative_revision_id }) =>
      ({ chapter_id, manuscript_block_id, base_authoritative_revision_id })), chapters.flatMap(({ chapter }) =>
      chapter.current_revision.blocks.map((block) => ({ chapter_id: chapter.chapter_id,
        manuscript_block_id: block.manuscript_block_id, base_authoritative_revision_id: chapter.current_revision.revision_id }))));
    assert.deepEqual(run.context.passage_targets, target.targets);
    const sent = run.evidence.find((evidence) => evidence.kind === "sent_content");
    if (sent?.kind !== "sent_content") throw new Error("expected actual sent content");
    const wire = JSON.parse(sent.content);
    assert.deepEqual(wire.targets, target.targets);
    assert.equal(wire.source_chapter_id, chapterId);
    assert.deepEqual(wire.selected, run.context.selected.map((source) => ({ ...source, token_count: Number(source.token_count) })));
    assert.equal(wire.selected.some((source: { content: string }) => source.content === "A lantern crossed the river."), true);
    const retained = JSON.parse(await queryPostgres(`SELECT payload->'wire' FROM storyos.model_attempts WHERE run_id='${run.run_id}'::uuid;`));
    assert.equal(retained.serialized_payload, sent.content);
    assert.equal(retained.digest, `sha256:${createHash("sha256").update(sent.content).digest("hex")}`);
    const proposals = await Promise.all([...new Set(run.decision.locations!.map((location) => {
      if (location.outcome.kind !== "opened") throw new Error("expected eligible location");
      return location.outcome.proposal_id;
    }))].map(async (proposalId) => (await getProposal({ baseUrl: started.baseUrl, projectId, fetchImpl, proposalId })).proposal));
    for (const [index, location] of run.decision.locations!.entries()) {
      assert.equal(location.candidate_text, index === 0 ? PROSE : SECOND_PROSE);
      assert.equal(location.explanation, index === 0 ? "Preserve the narrator voice in the first passage." : "Keep the second passage consistent with the narrator voice.");
      if (location.outcome.kind !== "opened") throw new Error("expected opened location");
      const outcome = location.outcome;
      assert.equal(proposals.find((proposal) => proposal.proposal_id === outcome.proposal_id)?.operations
        .find((operation) => operation.operation_id === outcome.operation_id)?.candidate_text, location.candidate_text);
    }
    await stopRealServer(started.server);
    started = await startRealServer();
    const reloadedFetch = browserFetch(started.baseUrl, "session-a");
    const reloaded = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: run.run_id, fetchImpl: reloadedFetch });
    assert.deepEqual(reloaded, { ...run, correlation_id: reloaded.correlation_id });
    for (const proposal of proposals) assert.deepEqual((await getProposal({ baseUrl: started.baseUrl,
      projectId, proposalId: proposal.proposal_id, fetchImpl: reloadedFetch })).proposal, proposal);
    for (const before of chapters) assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId,
      chapterId: before.chapter.chapter_id, fetchImpl })).chapter, before.chapter);
  } finally { await stopRealServer(started.server); }
});

test.each(["deleted", "foreign", "stale", "missing_block", "over_budget", "reservation"])("collection refuses %s before destination dispatch", async (boundary) => {
  const started = await startRealServer();
  try {
    const ns = randomBytes(3).toString("hex");
    const setup = await collectionSetup(started.baseUrl, ns, boundary === "over_budget" ? "a".repeat(10_000) : undefined);
    if (boundary === "deleted") {
    const tree = await getManuscriptTree({ baseUrl: started.baseUrl, ...setup });
    const request = { command_schema: "storyos.command.delete-chapter.request.v1",
      delete_chapter_input: { expected_tree_revision: tree.tree_revision, ...BINDING, correlation_id: id(`${ns}61`) } };
    await challenged(started.baseUrl, setup.fetchImpl, setup.projectId, "DELETE",
      "/api/v1/projects/{project_id}/chapters/{chapter_id}", request.command_schema,
      await digestDeleteChapter(request), id(`${ns}62`), (antiForgery) => deleteChapter({
        baseUrl: started.baseUrl, projectId: setup.projectId, chapterId: setup.target.targets[1]!.chapter_id,
        fetchImpl: setup.fetchImpl, request, antiForgery, idempotencyKey: id(`${ns}62`) }));
    } else if (boundary === "foreign") {
      const foreign = await prepare(started.baseUrl, id(`${ns}71`), "Foreign target", `${ns}72`);
      await seedTwoBlocks(started.baseUrl, foreign.fetchImpl, foreign.projectId, `${ns}74`, "Private foreign prose.");
      const chapter = await getChapter({ baseUrl: started.baseUrl, ...foreign });
      setup.target.targets[1] = { chapter_id: foreign.chapterId,
        base_authoritative_revision_id: chapter.chapter.current_revision.revision_id,
        manuscript_block_ids: chapter.chapter.current_revision.blocks.map((block) => block.manuscript_block_id) };
    } else if (boundary === "stale") {
      setup.target.targets[1]!.base_authoritative_revision_id = setup.target.targets[0]!.base_authoritative_revision_id;
    } else if (boundary === "missing_block") {
      setup.target.targets[1]!.manuscript_block_ids = [id(`${ns}73`)];
    }
    if (boundary === "reservation") await admitPassages(started.baseUrl, setup.fetchImpl, setup.projectId, setup.chapterId,
      "Revise these passages: keep the voice.", id(`${ns}64`), undefined, true, { ...setup.target, targets: [setup.target.targets[1]!] });
    const run = await admitPassages(started.baseUrl, setup.fetchImpl, setup.projectId, setup.chapterId,
      "Revise these passages: keep the voice.", id(`${ns}63`), undefined, true, setup.target);
    if (boundary === "reservation") {
      if (run.decision.kind !== "prose_change") throw new Error("expected truthful location outcomes");
      assert.deepEqual(run.decision.locations?.map((location) => location.outcome.kind === "opened"
        ? { kind: "opened" } : location.outcome), [{ kind: "opened" }, { kind: "opened" }, { kind: "refused", reason: "conflicting_reservation" }]);
    } else {
      assert.equal(run.context.sufficiency.kind, "blocked");
      assert.deepEqual(run.model_attempt, { kind: "absent" });
      assert.deepEqual(run.evidence, []);
      if (boundary === "over_budget") assert.equal(run.context.selected.some((source) => source.source_class === "working_target"), false);
      else assert.equal(run.context.selected.some((source) => source.content === "A lantern crossed the river."), false);
      assert.equal(run.context.selected.some((source) => source.content === "Private foreign prose."), false);
    }
    assert.deepEqual(run.context.passage_targets, setup.target.targets);
    await settleOnce();
    const repeated = await getAgentRun({ baseUrl: started.baseUrl, ...setup, runId: run.run_id });
    assert.deepEqual(repeated, { ...run, correlation_id: repeated.correlation_id });
  } finally { await stopRealServer(started.server); }
});

test("one request produces explained exact locations and keeps them after restart", async () => {
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const { fetchImpl, projectId, chapterId } = await prepare(started.baseUrl, id(`${ns}11`), "Explained locations", `${ns}2`);
    await seedTwoBlocks(started.baseUrl, fetchImpl, projectId, `${ns}3`);
    const before = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    const run = await admitPassages(started.baseUrl, fetchImpl, projectId, chapterId,
      "Revise these passages: keep the voice.", id(`${ns}41`));
    assert.equal(run.decision.kind, "prose_change");
    if (run.decision.kind !== "prose_change") throw new Error("expected prose change");
    const locations = run.decision.locations;
    assert.equal(locations?.length, 2);
    assert.deepEqual(locations!.map(({ chapter_id, manuscript_block_id, base_authoritative_revision_id,
      candidate_text, explanation }) => ({ chapter_id, manuscript_block_id, base_authoritative_revision_id, candidate_text, explanation })),
      before.chapter.current_revision.blocks.map((block, index) => ({ chapter_id: chapterId,
        manuscript_block_id: block.manuscript_block_id,
        base_authoritative_revision_id: before.chapter.current_revision.revision_id,
        candidate_text: index === 0 ? PROSE : SECOND_PROSE,
        explanation: index === 0 ? "Preserve the narrator voice in the first passage." : "Keep the second passage consistent with the narrator voice.",
      })));
    const opened = await getProposal({ baseUrl: started.baseUrl, projectId, fetchImpl,
      proposalId: openedProposal(run) });
    assert.deepEqual(opened.proposal.operations.map((operation) =>
      operation.candidate_text), [PROSE, SECOND_PROSE]);
    assert.equal(opened.proposal.validation_receipt.kind, "present");
    const after = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    assert.deepEqual(after.chapter, before.chapter);
    await stopRealServer(started.server);
    started = await startRealServer();
    const reloadedFetch = browserFetch(started.baseUrl, "session-a");
    const reloaded = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: run.run_id, fetchImpl: reloadedFetch });
    assert.deepEqual(reloaded.decision, run.decision);
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: reloadedFetch })).proposal, opened.proposal);
    await settleOnce();
    assert.deepEqual((await getAgentRun({ baseUrl: started.baseUrl, projectId,
      runId: run.run_id, fetchImpl: reloadedFetch })).decision, run.decision);
  } finally { await stopRealServer(started.server); }
});

test.each(["stream", "decision", "cancelled", "collection_stream", "collection_decision", "collection_cancelled", "collection_paused", "collection_rejected"])("typed result recovers the %s boundary without revival", async (scenario) => {
  const phase = scenario === "collection_rejected" ? "decision" : scenario.replace("collection_", "");
  const terminal = phase === "cancelled" || phase === "paused";
  let started = await startRealServer();
  const hold = join(tmpdir(), `storyos-377-${randomBytes(6).toString("hex")}.hold`);
  let child: ReturnType<typeof execFile> | undefined;
  let exited: Promise<void> | undefined;
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const collection = scenario.startsWith("collection_") ? await collectionSetup(started.baseUrl, ns) : undefined;
    const { fetchImpl, projectId, chapterId } = collection ?? await prepare(started.baseUrl, id(`${ns}11`), "Recovered locations", `${ns}2`);
    if (!collection) await seedTwoBlocks(started.baseUrl, fetchImpl, projectId, `${ns}3`);
    const before = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
    const admitted = await admitPassages(started.baseUrl, fetchImpl, projectId, chapterId,
      collection ? "Stream this passage:" : "Revise these passages: keep the voice.", id(`${ns}41`), undefined, false, collection?.target);
    const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
    writeFileSync(hold, "hold");
    child = execFile(join(repositoryRoot, "target/release-package/storyos-worker"), ["--once"], {
      cwd: repositoryRoot, env: { ...process.env, STORYOS_DATABASE_URL: process.env.STORYOS_TEST_DATABASE_URL,
        [phase === "decision" ? "STORYOS_TEST_FAKE_DECISION_HOLD_PATH" : "STORYOS_TEST_FAKE_STREAM_HOLD_PATH"]: hold },
      timeout: 60_000, killSignal: "SIGKILL",
    });
    exited = new Promise<void>((resolve) => child!.once("exit", () => resolve()));
    let held = admitted;
    await vi.waitUntil(async () => {
      held = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: admitted.run_id, fetchImpl });
      return phase === "decision" ? held.decision.kind === "prose_change" : held.items.length > 0;
    }, { timeout: 10_000, interval: 10 });
    const frozen = await queryPostgres(`SELECT payload->'decision' FROM storyos.model_attempts WHERE run_id='${admitted.run_id}'::uuid;`);
    const raw = held.items;
    const readProposals = async () => Promise.all((JSON.parse(await queryPostgres(
      `SELECT coalesce(jsonb_agg(proposal_id::text ORDER BY proposal_id), '[]'::jsonb)::text FROM storyos.proposals WHERE source_run_id='${admitted.run_id}'::uuid;`
    )) as string[]).map(async (proposalId) => (await getProposal({ baseUrl: started.baseUrl,
      projectId, fetchImpl, proposalId })).proposal));
    let priorProposals: Awaited<ReturnType<typeof readProposals>> | undefined;
    if (scenario === "collection_rejected" && collection) {
      const opened = await getProposal({ baseUrl: started.baseUrl, projectId, fetchImpl, proposalId: openedProposal(held) });
      const request = { command_schema: "storyos.command.reject-proposal-operations.request.v1",
        reject_proposal_operations_input: { proposal_revision_id: opened.proposal.revision_id,
          selected_pending_operation_ids: opened.proposal.operations.map((operation) => operation.operation_id),
          expected_target_revisions: [before.chapter.current_revision.revision_id],
          rejection_reason: { kind: "author_declined" as const, note: { kind: "omitted" as const } },
          editor_session_id: collection.editorSessionId, ...BINDING, correlation_id: id(`${ns}55`) } };
      const rejected = await challenged(started.baseUrl, fetchImpl, projectId, "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections", request.command_schema,
        await digestRejectProposalOperations(request), id(`${ns}56`), (antiForgery) => rejectProposalOperations({
          baseUrl: started.baseUrl, projectId, proposalId: opened.proposal.proposal_id, fetchImpl,
          request, antiForgery, idempotencyKey: id(`${ns}56`) }));
      assert.equal(rejected.effect.kind, "resolved");
      held = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: admitted.run_id, fetchImpl });
      priorProposals = await readProposals();
    }
    if (phase === "paused") {
      const request = { command_schema: "storyos.command.pause-agent-run.request.v1",
        pause_agent_run_input: { ...BINDING, correlation_id: id(`${ns}51`) } };
      await challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause",
        request.command_schema, await digestPauseAgentRun(request), id(`${ns}52`), (antiForgery) => pauseAgentRun({
          baseUrl: started.baseUrl, projectId, runId: admitted.run_id, fetchImpl, request, antiForgery, idempotencyKey: id(`${ns}52`) }));
    } else if (phase === "cancelled") {
      const request: CancelAgentRunRequest = { command_schema: "storyos.command.cancel-agent-run.request.v1",
        cancel_agent_run_input: { ...BINDING, correlation_id: id(`${ns}51`) } };
      await challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
        request.command_schema, await digestCancelAgentRun(request), id(`${ns}52`), (antiForgery) => cancelAgentRun({
          baseUrl: started.baseUrl, projectId, runId: admitted.run_id, fetchImpl, request, antiForgery, idempotencyKey: id(`${ns}52`) }));
    } else {
      child.kill("SIGKILL");
      await exited;
      child = undefined;
      await queryPostgres(`UPDATE storyos.agent_runs SET lease_expires_at=clock_timestamp()-interval '1 second' WHERE run_id='${admitted.run_id}'::uuid;`);
      if (phase === "decision") {
        const lossy: typeof fetch = async (input, init) => {
          const response = await fetchImpl(input, init);
          assert.equal(response.status, 200);
          await response.arrayBuffer();
          throw new Error("Controlled result response loss");
        };
        await assert.rejects(getAgentRun({ baseUrl: started.baseUrl, projectId, runId: admitted.run_id, fetchImpl: lossy }), /Controlled result response loss/);
      }
      const address = new URL(started.baseUrl);
      await stopRealServer(started.server);
      started = await startRealServer(`${address.hostname}:${address.port}`);
    }
    unlinkSync(hold);
    if (child) { await exited; child = undefined; }
    await settleOnce();
    const after = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: admitted.run_id, fetchImpl });
    assert.deepEqual(after.items, raw);
    if (priorProposals) assert.deepEqual(await readProposals(), priorProposals);
    assert.deepEqual(after.model_attempt, terminal ? held.model_attempt : { ...held.model_attempt, dispatch_state: "settled" });
    if (terminal) {
      assert.equal(after.status, phase);
      assert.deepEqual(after.decision, { kind: "absent" });
    } else {
      if (after.decision.kind !== "prose_change") throw new Error("expected typed decision");
      assert.equal(after.decision.locations?.length, collection ? 3 : 2);
      if (phase === "decision") {
        assert.equal(await queryPostgres(`SELECT payload->'decision' FROM storyos.model_attempts WHERE run_id='${admitted.run_id}'::uuid;`), frozen);
        assert.deepEqual(after.decision.locations, held.decision.kind === "prose_change" ? held.decision.locations : undefined);
      }
    }
    const count = terminal ? "0" : collection ? "2" : "1";
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.proposals WHERE source_run_id='${admitted.run_id}'::uuid;`), count);
    assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter, before.chapter);
    for (const original of collection?.chapters ?? []) assert.deepEqual((await getChapter({ baseUrl: started.baseUrl,
      projectId, chapterId: original.chapter.chapter_id, fetchImpl })).chapter, original.chapter);
    await settleOnce();
    const repeated = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: admitted.run_id, fetchImpl });
    assert.deepEqual(repeated, { ...after, correlation_id: repeated.correlation_id });
  } finally {
    if (existsSync(hold)) unlinkSync(hold);
    if (child) { child.kill("SIGKILL"); await exited; }
    await stopRealServer(started.server);
  }
});

test.each(["undeclared_location", "stale_location_base"])("scalar requests do not enter the typed script %s", async (script) => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const { fetchImpl, projectId, chapterId } = await prepare(started.baseUrl, id(`${ns}11`), "Scalar compatibility", `${ns}2`);
    const run = await admitPassages(started.baseUrl, fetchImpl, projectId, chapterId,
      `Revise this passage SCRIPT:${script}`, id(`${ns}41`));
    assert.equal(run.decision.kind, "advisory");
    const normal = await admitPassages(started.baseUrl, fetchImpl, projectId, chapterId,
      `Revise this passage: SCRIPT:${script}`, id(`${ns}51`));
    if (normal.decision.kind !== "prose_change") throw new Error("expected legacy prose");
    assert.equal(normal.decision.locations, undefined);
    assert.equal((await getProposal({ baseUrl: started.baseUrl, projectId,
      proposalId: openedProposal(normal), fetchImpl })).proposal.candidate_text, PROSE);
  } finally { await stopRealServer(started.server); }
});

test.each(["malformed_locations", "undeclared_location", "stale_location_base", "oversized_explanation", "incomplete", "unselected"])(
  "unusable typed output %s retains evidence without candidates", async (script) => {
    const started = await startRealServer();
    try {
      await drainLeftoverWork();
      const ns = randomBytes(3).toString("hex");
      const { fetchImpl, projectId, chapterId } = await prepare(started.baseUrl, id(`${ns}11`), "Rejected list", `${ns}2`);
      await seedTwoBlocks(started.baseUrl, fetchImpl, projectId, `${ns}3`);
      const before = await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl });
      const run = await admitPassages(started.baseUrl, fetchImpl, projectId, chapterId,
        `Revise these passages SCRIPT:${script}`, id(`${ns}41`));
      assert.deepEqual(run.decision, { kind: "absent" });
      const output = JSON.parse(run.items[0]!.text!);
      assert.equal(output.length, 2);
      assert.equal(output[1].candidate_text, SECOND_PROSE);
      assert.equal(run.items[0]!.state, script === "incomplete" ? "incomplete" : "complete");
      assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.proposals WHERE source_run_id = '${run.run_id}'::uuid;`), "0");
      assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter, before.chapter);
      const reloaded = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: run.run_id, fetchImpl });
      assert.deepEqual(reloaded, { ...run, correlation_id: reloaded.correlation_id });
    } finally { await stopRealServer(started.server); }
  });

test.each(["", " SCRIPT:reverse_locations"])("a later reservation preserves the permitted location%s", async (script) => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Reserved Multi Target Novel", `${ns}2`);
    const seeded = await seedTwoBlocks(started.baseUrl, prepared.fetchImpl, prepared.projectId, `${ns}3`);
    const queried = await admitPassages(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
      `Revise these passages: keep the voice.${script}`, id(`${ns}41`), async () => {
        const other = await admitPassages(
          started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
          "Revise this passage: keep the voice.", id(`${ns}51`), undefined, false,
        );
        const proposalId = id(`${ns}61`);
        await queryPostgres(`
          INSERT INTO storyos.proposals
            (owner_user_id, project_id, proposal_id, kind, chapter_id,
             manuscript_block_id, source_run_id, source_decision_id)
          VALUES ('${USER_A}'::uuid, '${prepared.projectId}'::uuid, '${proposalId}'::uuid,
            'block_edit', '${prepared.chapterId}'::uuid, '${seeded.secondBlockId}'::uuid,
            '${other.run_id}'::uuid, '${id(`${ns}62`)}'::uuid);
          INSERT INTO storyos.proposal_operations
            (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
             resolution, reservation_state, candidate_text)
          VALUES ('${USER_A}'::uuid, '${prepared.projectId}'::uuid, '${proposalId}'::uuid,
            '${id(`${ns}63`)}'::uuid, '${seeded.secondBlockId}'::uuid, 'pending', 'unresolved', 'reserved');
        `);
      },
    );
    assert.equal(queried.decision.kind, "prose_change");
    if (queried.decision.kind !== "prose_change") throw new Error("expected prose decision");
    const opened = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: openedProposal(queried), fetchImpl: prepared.fetchImpl });
    assert.equal(opened.proposal.operations.length, 1);
    assert.deepEqual(queried.decision.locations?.map((location) => ({
      block: location.manuscript_block_id, candidate: location.candidate_text, outcome: location.outcome,
    })), [{ block: opened.proposal.operations[0]!.manuscript_block_id, candidate: PROSE,
      outcome: { kind: "opened", proposal_id: opened.proposal.proposal_id,
        operation_id: opened.proposal.operations[0]!.operation_id, revision_id: opened.proposal.revision_id,
        validation_receipt_id: opened.proposal.validation_receipt.kind === "present"
          ? opened.proposal.validation_receipt.validation_receipt_id : "missing" } },
      { block: seeded.secondBlockId, candidate: SECOND_PROSE,
        outcome: { kind: "refused", reason: "conflicting_reservation" } }]);
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.proposals
      WHERE source_run_id = '${queried.run_id}'::uuid;`), "1");
    const chapter = await getChapter({ baseUrl: started.baseUrl,
      projectId: prepared.projectId, chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    assert.equal(chapter.chapter.current_revision.revision_id, seeded.revisionId);
    await settleOnce();
  } finally {
    await stopRealServer(started.server);
  }
});

function openedProposal(queried: Awaited<ReturnType<typeof getAgentRun>>): string {
  if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened prose");
  }
  return queried.decision.opened_proposal.proposal_id;
}

function acceptRequest(
  opened: GetProposalResponse,
  selected: string[],
  expectedRevisionId: string,
  editorSessionId: string,
  correlationId: string,
): AcceptProposalRequest {
  if (opened.proposal.validation_receipt.kind !== "present") {
    throw new Error("expected validation receipt");
  }
  return {
    command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: {
      proposal_revision_id: opened.proposal.revision_id,
      validation_receipt_id: opened.proposal.validation_receipt.validation_receipt_id,
      selected_operation_ids: selected,
      expected_authoritative_revision_id: expectedRevisionId,
      editor_session_id: editorSessionId,
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

test("acceptProposal applies a reversed multi-operation set without using array order as authority", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f3820111"), "Multi Operation Novel", "f3822");
    const seeded = await seedTwoBlocks(started.baseUrl, prepared.fetchImpl, prepared.projectId, "f3823");
    const queried = await admitPassages(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
      "Revise these passages: keep the voice.", id("f3820131"),
    );
    const opened = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: openedProposal(queried), fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.proposal.operations.length, 2);
    assert.deepEqual(opened.proposal.operations.map((operation) => operation.resolution), [
      "pending", "pending",
    ]);
    const selected = [...opened.proposal.operations.map((operation) => operation.operation_id)].reverse();
    const request = acceptRequest(
      opened, selected, seeded.revisionId,
      seeded.session.editor_session.editor_session_id, id("f3820151"),
    );
    const accepted = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      request.command_schema, await digestAcceptProposal(request), id("f3820152"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f3820152"), antiForgery, request,
      }),
    );
    assert.equal(accepted.effect.kind, "applied");
    assert.deepEqual(accepted.receipt.selected_operation_ids, selected);
    if (accepted.effect.kind !== "applied") throw new Error("expected applied");
    assert.equal(accepted.effect.authoritative_revision.body, `${PROSE}\n${SECOND_PROSE}`);
    const inspected = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(inspected.proposal.operations.map((operation) => operation.resolution), [
      "applied", "applied",
    ]);
    const after = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl,
    });
    assert.equal(after.chapter.current_revision.body, `${PROSE}\n${SECOND_PROSE}`);
  } finally {
    await stopRealServer(started.server);
  }
});

for (const history of ["retained", "legacy_overwritten", "collection"]) {
test(`partial Acceptance preserves ${history} evidence and conflicts the remaining Operation`, async () => {
  const ns = randomBytes(3).toString("hex");
  let started = await startRealServer();
  try {
    await drainLeftoverWork();
    const collection = history === "collection" ? await collectionSetup(started.baseUrl, ns) : undefined;
    const prepared = collection ?? await prepare(started.baseUrl, id(`${ns}0311`), "Subset Operation Novel", `${ns}6`);
    const seeded = collection ? { revisionId: collection.target.targets[0]!.base_authoritative_revision_id,
      session: await getEditorSession({ baseUrl: started.baseUrl, ...prepared, editorSessionId: collection.editorSessionId }) }
      : await seedTwoBlocks(started.baseUrl, prepared.fetchImpl, prepared.projectId, `${ns}7`);
    const queried = await admitPassages(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
      "Revise these passages: keep the voice.", id(`${ns}0331`), undefined, true, collection?.target,
    );
    const opened = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: openedProposal(queried), fetchImpl: prepared.fetchImpl,
    });
    const firstOperation = opened.proposal.operations[0];
    const secondOperation = opened.proposal.operations[1];
    if (!firstOperation || !secondOperation) throw new Error("expected two Operations");
    const retainedHistory = async () => JSON.parse(await queryPostgres(`SELECT jsonb_build_object(
      'revision', (SELECT to_jsonb(record) FROM storyos.proposal_revisions AS record
        WHERE project_id='${prepared.projectId}'::uuid AND revision_id='${opened.proposal.revision_id}'::uuid),
      'validation', (SELECT to_jsonb(record) FROM storyos.validation_receipts AS record
        WHERE project_id='${prepared.projectId}'::uuid AND proposal_revision_id='${opened.proposal.revision_id}'::uuid))::text`));
    const originalHistory = await retainedHistory();
    const beforeAcceptance = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
    const firstRequest = acceptRequest(
      opened, [firstOperation.operation_id], seeded.revisionId,
      seeded.session.editor_session.editor_session_id, id(`${ns}0351`),
    );
    let durableResponse: Awaited<ReturnType<typeof acceptProposal>> | undefined;
    const lossyFetch: typeof fetch = async (input, init) => {
      const response = await prepared.fetchImpl(input, init);
      if (String(input).endsWith("/acceptances") && response.status === 200) {
        durableResponse = await response.clone().json();
        if (started.server.exitCode === null && started.server.signalCode === null) await stopRealServer(started.server);
        throw new Error("Controlled acknowledgement loss");
      }
      return response;
    };
    let nonce = "";
    await assert.rejects(challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      firstRequest.command_schema, await digestAcceptProposal(firstRequest), id(`${ns}0352`),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: lossyFetch,
        idempotencyKey: id(`${ns}0352`), antiForgery: (nonce = antiForgery), request: firstRequest,
      }),
    ), /Controlled acknowledgement loss/);
    const address = new URL(started.baseUrl);
    started = await startRealServer(`${address.hostname}:${address.port}`);
    const fetchImpl = browserFetch(started.baseUrl, "session-a");
    const firstAccepted = await acceptProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl,
      idempotencyKey: id(`${ns}0352`), antiForgery: nonce, request: firstRequest,
    });
    assert.deepEqual(firstAccepted, durableResponse);
    assert.deepEqual(await retainedHistory(), originalHistory);
    assert.equal(firstAccepted.effect.kind, "applied");
    const afterFirst = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(afterFirst.proposal, { ...opened.proposal,
      operation_resolution: "applied", reservation_state: "resolved",
      operations: opened.proposal.operations.map((operation) => operation.operation_id === firstOperation.operation_id
        ? { ...operation, resolution: "applied", reservation_state: "resolved" } : operation),
    });
    if (firstAccepted.effect.kind !== "applied") throw new Error("expected applied");
    assert.deepEqual(firstAccepted.effect.authoritative_revision, {
      ...beforeAcceptance.chapter.current_revision, revision_id: firstAccepted.effect.authoritative_revision.revision_id,
      body: `${PROSE}\nWorld`,
      blocks: beforeAcceptance.chapter.current_revision.blocks.map((block) =>
        block.manuscript_block_id === firstOperation.manuscript_block_id ? { ...block, text: PROSE } : block),
    });
    if (history === "legacy_overwritten") {
      await queryPostgres(`UPDATE storyos.proposal_revisions SET base_authoritative_revision_id=
        '${firstAccepted.effect.authoritative_revision.revision_id}'::uuid
        WHERE project_id='${prepared.projectId}'::uuid AND revision_id='${opened.proposal.revision_id}'::uuid;
        UPDATE storyos.validation_receipts SET base_authoritative_revision_id=
        '${firstAccepted.effect.authoritative_revision.revision_id}'::uuid
        WHERE project_id='${prepared.projectId}'::uuid AND proposal_revision_id='${opened.proposal.revision_id}'::uuid`);
      await assert.rejects(getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl }),
        (error) => requireStoryOSProtocolError(error).status === 503);
    }
    const unchangedHistory = await retainedHistory();
    const secondRequest = acceptRequest(
      afterFirst, [secondOperation.operation_id],
      firstAccepted.effect.authoritative_revision.revision_id,
      seeded.session.editor_session.editor_session_id, id(`${ns}0353`),
    );
    let secondNonce = "";
    const acceptRemaining = async () => secondNonce === "" ? challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      secondRequest.command_schema, await digestAcceptProposal(secondRequest), id(`${ns}0354`),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id(`${ns}0354`), antiForgery: (secondNonce = antiForgery), request: secondRequest,
      }),
    ) : acceptProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl,
      idempotencyKey: id(`${ns}0354`), antiForgery: secondNonce, request: secondRequest,
    });
    if (history !== "legacy_overwritten") {
      const settlementState = async () => JSON.parse(await queryPostgres(`SELECT jsonb_build_object(${[
        "authoritative_heads", "authoritative_revisions", "authoritative_commits", "author_action_entries",
        "scope_counters", "proposal_heads", "proposal_revisions", "proposal_operations", "validation_receipts",
        "domain_receipts", "acceptance_receipts", "proposal_validation_conditions",
      ].map((table) => `'${table}', (SELECT coalesce(jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text), '[]'::jsonb)
        FROM storyos.${table} AS record WHERE owner_user_id='${USER_A}'::uuid AND project_id='${prepared.projectId}'::uuid)`).join(",")})::text`));
      const beforeFault = await settlementState();
      const fault = `validation_fault_${ns}`;
      await queryPostgres(`CREATE FUNCTION storyos.${fault}() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN IF NEW.project_id='${prepared.projectId}'::uuid THEN RAISE EXCEPTION 'controlled condition fault'; END IF;
        RETURN NEW; END $$; CREATE TRIGGER ${fault} BEFORE INSERT ON storyos.proposal_validation_conditions
        FOR EACH ROW EXECUTE FUNCTION storyos.${fault}();`);
      try {
        await assert.rejects(acceptRemaining, (error) => requireStoryOSProtocolError(error).status === 503);
        assert.deepEqual(await settlementState(), beforeFault);
      } finally {
        await queryPostgres(`DROP TRIGGER ${fault} ON storyos.proposal_validation_conditions; DROP FUNCTION storyos.${fault}()`);
      }
    }
    const secondAccepted = await acceptRemaining();
    assert.deepEqual(secondAccepted.effect, { kind: "conflicted", reason: "changed_head" });
    assert.deepEqual(await retainedHistory(), unchangedHistory);
    assert.deepEqual(await acceptProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl,
      idempotencyKey: id(`${ns}0352`), antiForgery: nonce, request: firstRequest,
    }), firstAccepted);
    const conflictRef = secondAccepted.receipt.condition_refs[0]!;
    if (history !== "legacy_overwritten") {
      const afterConflict = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl,
    });
    assert.deepEqual(afterConflict.proposal, {
      ...afterFirst.proposal, validation: "conflicted",
      condition_refs: secondAccepted.receipt.condition_refs,
      source_condition: { kind: "proposal_conflict", proposal_conflict_ref: secondAccepted.receipt.condition_refs[0]! },
    });
    }
    const chapter = await getChapter({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      chapterId: prepared.chapterId, fetchImpl,
    });
    assert.deepEqual(chapter.chapter.current_revision, firstAccepted.effect.authoritative_revision);
    const replanRequest: ReplanProposalRequest = {
      command_schema: "storyos.command.replan-proposal.request.v1",
      replan_proposal_input: {
        ...BINDING, correlation_id: id(`${ns}0361`),
        conflicted_proposal_revision_id: opened.proposal.revision_id,
        expected_current_proposal_head: opened.proposal.revision_id,
        expected_current_target_revisions: [chapter.chapter.current_revision.revision_id],
        replacement_operations: [secondOperation.operation_id],
        source_condition: { kind: "proposal_conflict", proposal_conflict_ref: conflictRef },
        editor_session_id: seeded.session.editor_session.editor_session_id,
      },
    };
    const replanned = await challenged(started.baseUrl, fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans", replanRequest.command_schema,
      await digestReplanProposal(replanRequest), id(`${ns}0362`), (antiForgery) => replanProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId, proposalId: opened.proposal.proposal_id,
        fetchImpl, idempotencyKey: id(`${ns}0362`), antiForgery, request: replanRequest,
      }));
    assert.equal(replanned.effect.kind, "resolved");
    const afterReplan = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl });
    assert.notEqual(afterReplan.proposal.revision_id, opened.proposal.revision_id);
    assert.deepEqual(afterReplan.proposal, { ...afterFirst.proposal,
      revision_id: afterReplan.proposal.revision_id, validation: "pending",
      validation_receipt: { kind: "absent" },
      base_authoritative_revision_id: chapter.chapter.current_revision.revision_id,
      revision_comparison: afterReplan.proposal.revision_comparison,
    });
    const pendingRequest: AcceptProposalRequest = { ...secondRequest,
      accept_proposal_input: { ...secondRequest.accept_proposal_input,
        proposal_revision_id: afterReplan.proposal.revision_id, correlation_id: id(`${ns}0363`) } };
    const pendingAcceptance = await challenged(started.baseUrl, fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", pendingRequest.command_schema,
      await digestAcceptProposal(pendingRequest), id(`${ns}0364`), (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId, proposalId: opened.proposal.proposal_id,
        fetchImpl, idempotencyKey: id(`${ns}0364`), antiForgery, request: pendingRequest,
      }));
    assert.deepEqual(pendingAcceptance.effect, { kind: "refused", reason: "not_eligible" });
    assert.deepEqual((await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl })).proposal, afterReplan.proposal);
    assert.deepEqual(await retainedHistory(), unchangedHistory);
    const foreign = browserFetch(started.baseUrl, "session-b");
    await assert.rejects(getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: opened.proposal.proposal_id, fetchImpl: foreign }),
      (error) => requireStoryOSProtocolError(error).status === 404);
    const exportRequest = { command_schema: "storyos.command.export-project-archive.request.v1",
      export_project_archive_input: { ...BINDING, correlation_id: id(`${ns}0371`),
        archive_profile: "storyos.project-export.v1",
        archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
    const archive = async (key = id(`${ns}0372`)) => challenged(started.baseUrl, fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/exports", exportRequest.command_schema,
      await digestExportProjectArchive(exportRequest), key, (antiForgery) => exportProjectArchive({
        baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl,
        idempotencyKey: key, antiForgery, request: exportRequest,
      }));
    if (history === "legacy_overwritten") {
      await assert.rejects(() => archive(), (error) => {
        const problem = requireStoryOSProtocolError(error);
        return problem.status === 422 && JSON.parse(problem.responseBody ?? "null").code === "invalid_provenance";
      });
      assert.deepEqual(await retainedHistory(), unchangedHistory);
    } else {
      const admitted = await archive();
      if (admitted.effect.kind !== "admitted") throw new Error("expected archive Admission");
      await settleOnce();
      const exported = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        exportId: admitted.effect.export_id, fetchImpl });
      assert.equal(exported.status, "ready");
      const download = await fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${admitted.effect.export_id}`,
        { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
      assert.equal(download.status, 200);
      const files = zipStoreFiles(new Uint8Array(await download.arrayBuffer()));
      for (const [table, expected] of [["proposal_revisions", originalHistory.revision], ["validation_receipts", originalHistory.validation]] as const) {
        const records = JSON.parse(new TextDecoder().decode(files.get(`canonical/${table}.json`)));
        assert.deepEqual(records.find((row: { proposal_revision_id?: string; revision_id?: string }) =>
          (row.proposal_revision_id ?? row.revision_id) === opened.proposal.revision_id), expected);
      }
      const archivedAttempts = JSON.parse(new TextDecoder().decode(files.get("canonical/model_attempts.json")));
      const storedAttempts = JSON.parse(await queryPostgres(`SELECT jsonb_agg(to_jsonb(record))::text FROM storyos.model_attempts AS record WHERE project_id='${prepared.projectId}'::uuid;`));
      assert.deepEqual(archivedAttempts, storedAttempts);
      const originalOutput = archivedAttempts.find((attempt: { run_id: string }) => attempt.run_id === queried.run_id).payload;
      assert.deepEqual(originalOutput.decision.locations.map((location: { current: unknown }) => location.current), collection ? [null, null, null] : [null, null]);
      assert.deepEqual(originalOutput.decision.locations, queried.decision.kind === "prose_change"
        ? queried.decision.locations?.map((location) => ({ ...location, current: null })) : undefined);
      if (collection) {
        assert.equal(originalOutput.wire.digest, `sha256:${createHash("sha256").update(originalOutput.wire.serialized_payload).digest("hex")}`);
        assert.deepEqual((await getChapter({ baseUrl: started.baseUrl, ...prepared,
          chapterId: collection.chapters[1]!.chapter.chapter_id, fetchImpl })).chapter, collection.chapters[1]!.chapter);
      }
      const conditions = JSON.parse(new TextDecoder().decode(files.get("canonical/proposal_validation_conditions.json")));
      assert.equal(conditions.length, 1);
      assert.deepEqual(conditions, JSON.parse(await queryPostgres(`SELECT jsonb_agg(to_jsonb(record))::text
        FROM storyos.proposal_validation_conditions AS record WHERE project_id='${prepared.projectId}'::uuid`)));
      const historical = await archive(id(`${ns}0374`));
      if (historical.effect.kind !== "admitted") throw new Error("expected historical archive Admission");
      const exportId = historical.effect.export_id;
      const facts = JSON.parse(await queryPostgres(`SELECT facts::text FROM storyos.pinned_export_sources
        WHERE project_id='${prepared.projectId}'::uuid AND export_id='${exportId}'::uuid`));
      let replacement = "facts";
      for (const [familyIndex, family] of facts.families.entries()) {
        if (!["proposal_revisions", "validation_receipts"].includes(family.table)) continue;
        for (const [rowIndex, row] of family.rows.entries()) {
          if ((row.proposal_revision_id ?? row.revision_id) === opened.proposal.revision_id) {
            row.base_authoritative_revision_id = chapter.chapter.current_revision.revision_id;
            replacement = `jsonb_set(${replacement}, '{families,${familyIndex},rows,${rowIndex},base_authoritative_revision_id}',
              to_jsonb('${chapter.chapter.current_revision.revision_id}'::text))`;
          }
        }
      }
      const encoded = canonicalDraftValue(facts);
      const digest = createHash("sha256").update(encoded).digest("hex");
      await queryPostgres(`UPDATE storyos.pinned_export_sources SET facts=${replacement},
        facts_sha256='${digest}' WHERE project_id='${prepared.projectId}'::uuid AND export_id='${exportId}'::uuid`);
      await settleOnce();
      const failed = await getExportOperation({ baseUrl: started.baseUrl, projectId: prepared.projectId,
        exportId, fetchImpl });
      assert.equal(failed.status, "failed");
      assert.equal("immutable_root" in failed, false);
      const refused = await fetchImpl(`${started.baseUrl}/api/v1/projects/${prepared.projectId}/exports/${exportId}`,
        { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
      assert.equal(refused.status, 422);
      assert.deepEqual(await retainedHistory(), originalHistory);
    }

  } finally {
    if (started.server.exitCode === null && started.server.signalCode === null) await stopRealServer(started.server);
  }
});
}

test("acceptProposal refuses incomplete atomic Bundle closure then applies the closed set", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f3820211"), "Bundle Operation Novel", "f3824");
    const seeded = await seedTwoBlocks(started.baseUrl, prepared.fetchImpl, prepared.projectId, "f3825");
    const queried = await admitPassages(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId,
      "Revise these passages as a bundle: keep the voice.", id("f3820231"),
    );
    const opened = await getProposal({
      baseUrl: started.baseUrl, projectId: prepared.projectId,
      proposalId: openedProposal(queried), fetchImpl: prepared.fetchImpl,
    });
    assert.equal(opened.proposal.operations.length, 2);
    const firstOperation = opened.proposal.operations[0];
    if (!firstOperation) throw new Error("expected first Operation");
    const incomplete = acceptRequest(
      opened, [firstOperation.operation_id], seeded.revisionId,
      seeded.session.editor_session.editor_session_id, id("f3820251"),
    );
    const refused = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      incomplete.command_schema, await digestAcceptProposal(incomplete), id("f3820252"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f3820252"), antiForgery, request: incomplete,
      }),
    );
    assert.deepEqual(refused.effect, { kind: "refused", reason: "incomplete_bundle_closure" });
    const closed = acceptRequest(
      opened,
      [...opened.proposal.operations.map((operation) => operation.operation_id)].reverse(),
      seeded.revisionId, seeded.session.editor_session.editor_session_id, id("f3820253"),
    );
    const accepted = await challenged(
      started.baseUrl, prepared.fetchImpl, prepared.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
      closed.command_schema, await digestAcceptProposal(closed), id("f3820254"),
      (antiForgery) => acceptProposal({
        baseUrl: started.baseUrl, projectId: prepared.projectId,
        proposalId: opened.proposal.proposal_id, fetchImpl: prepared.fetchImpl,
        idempotencyKey: id("f3820254"), antiForgery, request: closed,
      }),
    );
    assert.equal(accepted.effect.kind, "applied");
    if (accepted.effect.kind !== "applied") throw new Error("expected applied");
    assert.equal(accepted.effect.authoritative_revision.body, `${PROSE}\n${SECOND_PROSE}`);
  } finally {
    await stopRealServer(started.server);
  }
});
