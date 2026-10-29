// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/settle-multi-operation-selections-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { test } from "vitest";
import { createAgentRun, createEditorSession, digestCreateAgentRun, digestCreateEditorSession, getAgentRun, getChapter,
  getProposal } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunRequest, CreateEditorSessionRequest, GetChapterResponse, GetProposalResponse,
  SelectionReference } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { BINDING, challenged, drainLeftoverWork, id, prepare, sendAuthorEdit, settleOnce,
} from "../support/acceptance.ts";
import type { Started } from "../support/acceptance.ts";
import { startServer } from "../support/http-harness.ts";

// ADR 0071: one reply revises the referenced candidate and grows its Proposal over adjacent Chapter blocks.
const SENTENCES = ["Rain fell.", "Wind rose.", "Night came.", "Dawn broke."];
const GROW = "make these lines one tight scene. SCRIPT:grow_proposal";
type Proposal = GetProposalResponse["proposal"];

/** A Chapter with one Block for each sentence, written by real author edits. */
async function seedChapter(started: Started, ns: string, count: number) {
  const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Growth", `${ns}2`);
  const { projectId, fetchImpl, chapterId } = prepared;
  const sessionRequest: CreateEditorSessionRequest = { command_schema: "storyos.command.create-editor-session.request.v1",
    ...BINDING, correlation_id: id(`${ns}31`) };
  const session = await challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/editor-sessions",
    sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest), id(`${ns}32`),
    (antiForgery) => createEditorSession({ baseUrl: started.baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}32`),
      antiForgery, request: sessionRequest }));
  const context = { prepared, session };
  const sentences = SENTENCES.slice(0, count);
  let written = await sendAuthorEdit(started, context, { kind: "replace_selection", from: 0, to: 0,
    text: sentences.join("") }, { kind: "authoritative" }, "1", `${ns}40`);
  for (const [index, sentence] of sentences.slice(0, -1).entries()) {
    if (written.effect.kind !== "authoritative_applied") throw new Error("expected written prose");
    written = await sendAuthorEdit(started, context, { kind: "split_block", offset: sentence.length,
      manuscript_block_id: written.effect.authoritative_revision.blocks[index]!.manuscript_block_id,
      new_manuscript_block_id: id(`${ns}5${index}`) }, { kind: "authoritative" }, String(index + 2), `${ns}4${index + 1}`);
  }
  const chapter = (await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter;
  assert.deepEqual(chapter.current_revision.blocks.map((block) => block.text), sentences);
  return { ...prepared, chapter };
}

type Seeded = Awaited<ReturnType<typeof seedChapter>>;

/** Sends one Agent request and settles its Run. `repeat` sends the exact request again. */
async function request(started: Started, seeded: Seeded, text: string, key: string, references?: SelectionReference[]) {
  const { projectId, fetchImpl, chapterId } = seeded;
  const body: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: { conversation: { kind: "new" }, author_message: { text },
      working_target: references ? { kind: "selection_references", source_chapter_id: chapterId, references }
        : { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" }, cause: { kind: "author_request" }, ...BINDING, correlation_id: key },
  };
  let nonce = "";
  const repeat = async () => nonce ? createAgentRun({ baseUrl: started.baseUrl, projectId, fetchImpl, request: body,
    antiForgery: nonce, idempotencyKey: key }) : challenged(started.baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs", body.command_schema, await digestCreateAgentRun(body), key,
    (antiForgery) => createAgentRun({ baseUrl: started.baseUrl, projectId, fetchImpl, request: body,
      antiForgery: (nonce = antiForgery), idempotencyKey: key }));
  const created = await repeat();
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  let run = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
  for (let attempt = 0; attempt < 8 && !["completed", "refused", "cancelled", "paused"].includes(run.status); attempt++) {
    await settleOnce();
    run = await getAgentRun({ baseUrl: started.baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
  }
  return { run, created, repeat };
}

/** Opens a Block Proposal on the first Block that no Proposal reserves, or on the last one. */
async function openProposal(started: Started, seeded: Seeded, key: string, block: "first" | "last" = "first") {
  const { run } = await request(started, seeded, block === "first" ? "Revise this passage: keep the voice."
    : "Revise these passages: keep the voice. SCRIPT:partial_location", key);
  if (run.decision.kind !== "prose_change" || run.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened prose");
  }
  return readProposal(started, seeded, run.decision.opened_proposal.proposal_id);
}

async function readProposal(started: Started, seeded: Seeded, proposalId: string) {
  return (await getProposal({ baseUrl: started.baseUrl, projectId: seeded.projectId, proposalId,
    fetchImpl: seeded.fetchImpl })).proposal;
}

function candidateReference(proposal: Proposal): SelectionReference {
  return { kind: "proposal_candidate_text", chapter_id: proposal.chapter_id, proposal_id: proposal.proposal_id,
    operation_id: proposal.operation_id, revision_id: proposal.revision_id,
    coordinate_profile: "storyos.editor.utf16-code-unit.v1", start_offset: 0, end_offset: 4 };
}

function chapterReference(chapter: GetChapterResponse["chapter"], start: [number, number], end: [number, number]): SelectionReference {
  const point = ([block, offset]: [number, number]) => ({
    manuscript_block_id: chapter.current_revision.blocks[block]!.manuscript_block_id, offset });
  return { kind: "chapter_text", chapter_id: chapter.chapter_id,
    base_authoritative_revision_id: chapter.current_revision.revision_id,
    coordinate_profile: "storyos.editor.utf16-code-unit.v1", start: point(start), end: point(end) };
}

async function revisions(proposalId: string) {
  return queryPostgres(`SELECT count(*)::text || ' ' || (SELECT bundle_policy FROM storyos.proposals
    WHERE proposal_id = '${proposalId}'::uuid) FROM storyos.proposal_revisions WHERE proposal_id = '${proposalId}'::uuid;`);
}

/** The Operations of a grown Proposal: the revised candidate, and each joined Block with its full new text. */
function grownOperations(before: Proposal, seeded: Seeded, joined: number[]) {
  return [
    { manuscript_block_id: before.manuscript_block_id,
      candidate_text: `${before.operations[0]!.candidate_text} Keep the voice consistent.` },
    ...joined.map((block) => ({ manuscript_block_id: seeded.chapter.current_revision.blocks[block]!.manuscript_block_id,
      candidate_text: `${SENTENCES[block]} Join the scene.` })),
  ].map((operation) => ({ ...operation, resolution: "pending", reservation_state: "unresolved" }));
}

function operations(proposal: Proposal) {
  return proposal.operations.map(({ manuscript_block_id, candidate_text, resolution, reservation_state }) =>
    ({ manuscript_block_id, candidate_text, resolution, reservation_state }));
}

test.each(["all", "part"])("a selection from the candidate into %s of the next Block grows the Proposal", async (cover) => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const seeded = await seedChapter(started, ns, 3);
    const before = await openProposal(started, seeded, id(`${ns}61`));
    assert.equal(await revisions(before.proposal_id), "1 none");
    const grown = await request(started, seeded, GROW, id(`${ns}71`), [candidateReference(before),
      chapterReference(seeded.chapter, [1, 0], [1, cover === "all" ? SENTENCES[1]!.length : 4])]);
    const after = await readProposal(started, seeded, before.proposal_id);
    assert.deepEqual(operations(after), grownOperations(before, seeded, [1]));
    assert.equal(await revisions(before.proposal_id), "2 atomic");
    assert.equal(await queryPostgres(`SELECT parent_revision_id::text FROM storyos.proposal_revisions
      WHERE revision_id = '${after.revision_id}'::uuid;`), before.revision_id);
    if (grown.run.decision.kind !== "prose_change" || after.validation_receipt.kind !== "present") {
      throw new Error("expected a growth Decision");
    }
    const validationReceiptId = after.validation_receipt.validation_receipt_id;
    assert.deepEqual(grown.run.decision.locations!.map(({ manuscript_block_id, outcome }) => ({ manuscript_block_id, outcome })),
      after.operations.map(({ manuscript_block_id, operation_id }) => ({ manuscript_block_id, outcome: {
        kind: "revised", proposal_id: before.proposal_id, operation_id, revision_id: after.revision_id,
        prior_revision_id: before.revision_id, validation_receipt_id: validationReceiptId } })));
    // An exact retry returns the same Run and writes no second Revision.
    assert.deepEqual(await grown.repeat(), grown.created);
    await settleOnce();
    assert.deepEqual(await readProposal(started, seeded, before.proposal_id), after);
    assert.equal(await revisions(before.proposal_id), "2 atomic");
  } finally { await stopRealServer(started.server); }
});

test("a selection from the Block above into the candidate grows the Proposal upward", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const seeded = await seedChapter(started, ns, 2);
    const before = await openProposal(started, seeded, id(`${ns}61`), "last");
    await request(started, seeded, GROW, id(`${ns}71`), [chapterReference(seeded.chapter, [0, 3], [1, SENTENCES[1]!.length]),
      candidateReference(before)]);
    const after = await readProposal(started, seeded, before.proposal_id);
    assert.deepEqual(operations(after), grownOperations(before, seeded, [0]));
    assert.equal(await revisions(before.proposal_id), "2 atomic");
  } finally { await stopRealServer(started.server); }
});

test("a Block that another pending Proposal reserves does not join", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const seeded = await seedChapter(started, ns, 4);
    const reserved = await openProposal(started, seeded, id(`${ns}61`));
    const before = await openProposal(started, seeded, id(`${ns}62`));
    assert.equal(before.manuscript_block_id, seeded.chapter.current_revision.blocks[1]!.manuscript_block_id);
    await request(started, seeded, GROW, id(`${ns}71`), [chapterReference(seeded.chapter, [0, 3], [1, SENTENCES[1]!.length]),
      candidateReference(before), chapterReference(seeded.chapter, [2, 0], [2, 4])]);
    assert.deepEqual(operations(await readProposal(started, seeded, before.proposal_id)), grownOperations(before, seeded, [2]));
    assert.deepEqual(await readProposal(started, seeded, reserved.proposal_id), reserved);
  } finally { await stopRealServer(started.server); }
});

test.each(["grow_unjoinable", "grow_gap"])("a %s reply is refused and the Proposal does not change", async (script) => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const ns = randomBytes(3).toString("hex");
    const seeded = await seedChapter(started, ns, 4);
    const before = await openProposal(started, seeded, id(`${ns}61`));
    const { run } = await request(started, seeded, `make these lines one tight scene. SCRIPT:${script}`, id(`${ns}71`),
      [candidateReference(before), chapterReference(seeded.chapter, [1, 0], [2, 4])]);
    assert.deepEqual(run.decision, { kind: "absent" });
    assert.deepEqual(await readProposal(started, seeded, before.proposal_id), before);
    assert.equal(await revisions(before.proposal_id), "1 none");
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.proposals WHERE source_run_id = '${run.run_id}'::uuid;`), "0");
  } finally { await stopRealServer(started.server); }
});
