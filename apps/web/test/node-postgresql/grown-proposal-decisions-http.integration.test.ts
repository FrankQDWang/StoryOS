// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/grow-block-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { test } from "vitest";
import { acceptProposal, createAgentRun, createEditorSession, digestAcceptProposal, digestCreateAgentRun,
  digestCreateEditorSession, digestUndoLatestAuthorAction, digestWithdrawProposal, getAgentRun, getChapter,
  getEditorSession, getProposal, undoLatestAuthorAction, withdrawProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, CreateAgentRunRequest, CreateEditorSessionRequest, GetChapterResponse,
  GetProposalResponse, SelectionReference, UndoLatestAuthorActionRequest, WithdrawProposalRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { BINDING, challenged, drainLeftoverWork, id, prepare, sendAuthorEdit, settleOnce, startRealServer,
} from "../support/acceptance.ts";
import type { Started } from "../support/acceptance.ts";

// ADR 0071: the author accepts, withdraws, and undoes around a grown Proposal as one unit.
const SENTENCES = ["Rain fell.", "Wind rose.", "Night came."];
const GROW = "make these lines one tight scene. SCRIPT:grow_proposal";
type Proposal = GetProposalResponse["proposal"];
type Chapter = GetChapterResponse["chapter"];

/** A Chapter with one Block for each sentence, and the editor session of its writer. */
async function seedChapter(started: Started) {
  const ns = `bb${randomBytes(2).toString("hex")}`;
  const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Grown decisions", `${ns}2`);
  const { projectId, fetchImpl, chapterId } = prepared;
  const sessionRequest: CreateEditorSessionRequest = { command_schema: "storyos.command.create-editor-session.request.v1",
    ...BINDING, correlation_id: id(`${ns}31`) };
  const session = await challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/editor-sessions",
    sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest), id(`${ns}32`),
    (antiForgery) => createEditorSession({ baseUrl: started.baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}32`),
      antiForgery, request: sessionRequest }));
  const context = { prepared, session };
  let written = await sendAuthorEdit(started, context, { kind: "replace_selection", from: 0, to: 0,
    text: SENTENCES.join("") }, { kind: "authoritative" }, "1", `${ns}40`);
  for (const [index, sentence] of SENTENCES.slice(0, -1).entries()) {
    if (written.effect.kind !== "authoritative_applied") throw new Error("expected written prose");
    written = await sendAuthorEdit(started, context, { kind: "split_block", offset: sentence.length,
      manuscript_block_id: written.effect.authoritative_revision.blocks[index]!.manuscript_block_id,
      new_manuscript_block_id: id(`${ns}5${index}`) }, { kind: "authoritative" }, String(index + 2), `${ns}4${index + 1}`);
  }
  return { ...prepared, ns, session, chapter: await readChapter(started, prepared) };
}

type Seeded = Awaited<ReturnType<typeof seedChapter>>;

async function readChapter(started: Started, seeded: { projectId: string; chapterId: string; fetchImpl: typeof fetch }) {
  const { projectId, chapterId, fetchImpl } = seeded;
  return (await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter;
}

async function readProposal(started: Started, seeded: Seeded, proposalId: string) {
  return (await getProposal({ baseUrl: started.baseUrl, projectId: seeded.projectId, proposalId,
    fetchImpl: seeded.fetchImpl })).proposal;
}

/** Sends one Agent request and settles its Run. */
async function request(started: Started, seeded: Seeded, text: string, key: string, references?: SelectionReference[]) {
  const { projectId, fetchImpl, chapterId } = seeded;
  const body: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: { conversation: { kind: "new" }, author_message: { text },
      working_target: references ? { kind: "selection_references", source_chapter_id: chapterId, references }
        : { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" }, cause: { kind: "author_request" }, ...BINDING, correlation_id: key },
  };
  const created = await challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/agent-runs",
    body.command_schema, await digestCreateAgentRun(body), key,
    (antiForgery) => createAgentRun({ baseUrl: started.baseUrl, projectId, fetchImpl, request: body, antiForgery,
      idempotencyKey: key }));
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  let run = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
  for (let attempt = 0; attempt < 8 && !["completed", "refused", "cancelled", "paused"].includes(run.status); attempt++) {
    await settleOnce();
    run = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
  }
  return run;
}

/** Opens a Block Proposal on the first Block. */
async function openProposal(started: Started, seeded: Seeded) {
  const run = await request(started, seeded, "Revise this passage: keep the voice.", id(`${seeded.ns}61`));
  if (run.decision.kind !== "prose_change" || run.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened prose");
  }
  return { run, proposal: await readProposal(started, seeded, run.decision.opened_proposal.proposal_id) };
}

function candidateReference(proposal: Proposal): SelectionReference {
  return { kind: "proposal_candidate_text", chapter_id: proposal.chapter_id, proposal_id: proposal.proposal_id,
    operation_id: proposal.operation_id, revision_id: proposal.revision_id,
    coordinate_profile: "storyos.editor.utf16-code-unit.v1", start_offset: 0, end_offset: 4 };
}

/** A reference from the start of the second Block to its end. */
function secondBlockReference(chapter: Chapter): SelectionReference {
  const block = chapter.current_revision.blocks[1]!.manuscript_block_id;
  return { kind: "chapter_text", chapter_id: chapter.chapter_id,
    base_authoritative_revision_id: chapter.current_revision.revision_id,
    coordinate_profile: "storyos.editor.utf16-code-unit.v1", start: { manuscript_block_id: block, offset: 0 },
    end: { manuscript_block_id: block, offset: SENTENCES[1]!.length } };
}

/** Grows the Proposal over the second Block and returns the growth Run and the grown Proposal. */
async function grow(started: Started, seeded: Seeded, before: Proposal) {
  const run = await request(started, seeded, GROW, id(`${seeded.ns}71`),
    [candidateReference(before), secondBlockReference(seeded.chapter)]);
  const grown = await readProposal(started, seeded, before.proposal_id);
  assert.deepEqual(grown.operations.map((operation) => operation.manuscript_block_id),
    seeded.chapter.current_revision.blocks.slice(0, 2).map((block) => block.manuscript_block_id));
  return { run, grown };
}

async function acceptOperations(started: Started, seeded: Seeded, proposal: Proposal, operationIds: string[], key: string) {
  if (proposal.validation_receipt.kind !== "present") throw new Error("expected a Validation Receipt");
  const body: AcceptProposalRequest = { command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: { proposal_revision_id: proposal.revision_id,
      validation_receipt_id: proposal.validation_receipt.validation_receipt_id, selected_operation_ids: operationIds,
      expected_authoritative_revision_id: seeded.chapter.current_revision.revision_id,
      editor_session_id: seeded.session.editor_session.editor_session_id, ...BINDING, correlation_id: id(`${key}1`) } };
  return challenged(started.baseUrl, seeded.fetchImpl, seeded.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", body.command_schema,
    await digestAcceptProposal(body), id(`${key}2`), (antiForgery) => acceptProposal({ baseUrl: started.baseUrl,
      projectId: seeded.projectId, proposalId: proposal.proposal_id, fetchImpl: seeded.fetchImpl,
      idempotencyKey: id(`${key}2`), antiForgery, request: body }));
}

test("accept all of a grown Proposal applies each Operation, and a part of it is refused", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const seeded = await seedChapter(started);
    const { proposal: before } = await openProposal(started, seeded);
    const { grown } = await grow(started, seeded, before);
    const operationIds = grown.operations.map((operation) => operation.operation_id);
    const part = await acceptOperations(started, seeded, grown, [grown.operation_id], `${seeded.ns}81`);
    assert.deepEqual(part.effect, { kind: "refused", reason: "incomplete_bundle_closure" });
    const all = await acceptOperations(started, seeded, grown, [...operationIds].sort(), `${seeded.ns}82`);
    assert.equal(all.effect.kind, "applied");
    assert.deepEqual((await readChapter(started, seeded)).current_revision.blocks.map((block) => block.text),
      [...grown.operations.map((operation) => operation.candidate_text), SENTENCES[2]]);
    assert.deepEqual((await readProposal(started, seeded, grown.proposal_id)).operations
      .map(({ resolution, reservation_state }) => ({ resolution, reservation_state })),
      operationIds.map(() => ({ resolution: "applied", reservation_state: "resolved" })));
  } finally { await stopRealServer(started.server); }
});

test("a withdrawal closes all of a grown Proposal in one step", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const seeded = await seedChapter(started);
    const { proposal: before } = await openProposal(started, seeded);
    const { grown } = await grow(started, seeded, before);
    const key = `${seeded.ns}81`;
    const body: WithdrawProposalRequest = { command_schema: "storyos.command.withdraw-proposal.request.v1",
      withdraw_proposal_input: { cause: "author", proposal_revision_id: grown.revision_id, expected_closure: "open",
        expected_target_revisions: [seeded.chapter.current_revision.revision_id],
        withdrawal_reason: { kind: "author_withdrew", note: { kind: "omitted" } },
        editor_session_id: seeded.session.editor_session.editor_session_id, ...BINDING, correlation_id: id(`${key}1`) } };
    const withdrawn = await challenged(started.baseUrl, seeded.fetchImpl, seeded.projectId, "POST",
      "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals", body.command_schema,
      await digestWithdrawProposal(body), id(`${key}2`), (antiForgery) => withdrawProposal({ baseUrl: started.baseUrl,
        projectId: seeded.projectId, proposalId: grown.proposal_id, fetchImpl: seeded.fetchImpl,
        idempotencyKey: id(`${key}2`), antiForgery, request: body }));
    assert.equal(withdrawn.effect.kind, "resolved");
    // The Operations stay pending and keep their reservations, as for each withdrawn Proposal.
    assert.deepEqual(await readProposal(started, seeded, grown.proposal_id), { ...grown, closure: "withdrawn" });
    assert.deepEqual(await readChapter(started, seeded), seeded.chapter);
  } finally { await stopRealServer(started.server); }
});

test("after a growth, Author Undo of an earlier candidate edit is a Barrier", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const seeded = await seedChapter(started);
    const { proposal: opened } = await openProposal(started, seeded);
    const edited = await sendAuthorEdit(started, { prepared: seeded, session: seeded.session }, { kind: "replace_selection", from: 0, to: 4, text: "Hold" },
      { kind: "targeted", proposal: opened }, "4", `${seeded.ns}65`);
    assert.equal(edited.effect.kind, "proposal_revised");
    const before = await readProposal(started, seeded, opened.proposal_id);
    const { grown } = await grow(started, seeded, before);
    const session = await getEditorSession({ baseUrl: started.baseUrl, projectId: seeded.projectId,
      fetchImpl: seeded.fetchImpl, editorSessionId: seeded.session.editor_session.editor_session_id });
    const key = `${seeded.ns}91`;
    const body: UndoLatestAuthorActionRequest = { command_schema: "storyos.command.undo-latest-author-action.request.v1",
      undo_latest_author_action_input: { expected_author_undo_frontier_sequence: session.author_undo_frontier_sequence!,
        expected_authoritative_revision_id: seeded.chapter.current_revision.revision_id,
        editor_session_id: seeded.session.editor_session.editor_session_id, ...BINDING, correlation_id: id(`${key}1`) } };
    const undone = await challenged(started.baseUrl, seeded.fetchImpl, seeded.projectId, "POST",
      "/api/v1/projects/{project_id}/author-actions/undo", body.command_schema, await digestUndoLatestAuthorAction(body),
      id(`${key}2`), (antiForgery) => undoLatestAuthorAction({ baseUrl: started.baseUrl, projectId: seeded.projectId,
        fetchImpl: seeded.fetchImpl, idempotencyKey: id(`${key}2`), antiForgery, request: body }));
    assert.deepEqual(undone.effect, { kind: "unavailable", reason: "barrier" });
    assert.deepEqual(await readProposal(started, seeded, grown.proposal_id), grown);
  } finally { await stopRealServer(started.server); }
});

test("the reads show the joinable blocks of a crossing selection and the atomic bundle of the growth", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const seeded = await seedChapter(started);
    const { run: opening, proposal: before } = await openProposal(started, seeded);
    assert.equal("joinable_block_ids" in opening.context, false);
    assert.equal(before.bundle_policy, "none");
    const { run, grown } = await grow(started, seeded, before);
    assert.equal(grown.bundle_policy, "atomic");
    assert.deepEqual(run.context.joinable_block_ids, [seeded.chapter.current_revision.blocks[1]!.manuscript_block_id]);
  } finally { await stopRealServer(started.server); }
});
