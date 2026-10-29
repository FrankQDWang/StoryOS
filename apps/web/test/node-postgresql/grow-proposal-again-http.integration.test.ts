// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/grow-block-proposal-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "vitest";
import { createAgentRun, createEditorSession, digestCreateAgentRun, digestCreateEditorSession, digestSteerAgentRun,
  getAgentRun, getChapter, steerAgentRun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunRequest, CreateEditorSessionRequest, GetChapterResponse, GetProposalResponse,
  SelectionReference, SteerAgentRunRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres } from "../support/node-integration.ts";
import { BINDING, challenged, drainLeftoverWork, id, operations, prepare, readProposal, revisions, sendAuthorEdit,
  settleOnce } from "../support/acceptance.ts";
import type { Started } from "../support/acceptance.ts";
import { runWorker, serverForFile } from "../support/http-harness.ts";

const started = serverForFile();

// ADR 0071: after a growth, the steering references move to the new Operations, and the Proposal can grow again.
const SENTENCES = ["Rain fell.", "Wind rose.", "Night came.", "Dawn broke."];
const GROW = "make these lines one tight scene. SCRIPT:grow_proposal";
const REVISED = " Keep the voice consistent.";
const JOINED = " Join the scene.";
type Proposal = GetProposalResponse["proposal"];
type Chapter = GetChapterResponse["chapter"];

/** A Chapter with one Block for each sentence, written by real author edits. */
async function seedChapter(started: Started, ns: string) {
  const prepared = await prepare(started.baseUrl, id(`${ns}11`), "Growth again", `${ns}2`);
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
  const chapter = (await getChapter({ baseUrl: started.baseUrl, projectId, chapterId, fetchImpl })).chapter;
  return { ...prepared, chapter };
}

type Seeded = Awaited<ReturnType<typeof seedChapter>>;

/**
 * Admits one Agent request. Then `settle` runs the Worker until the Run stops. `steerAfterDecision` holds the Worker
 * after the first Decision, adds a steering input to the same Run, and lets the Worker continue.
 */
async function admit(started: Started, seeded: Seeded, text: string, key: string, references?: SelectionReference[]) {
  const { projectId, fetchImpl, chapterId } = seeded;
  const body: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: { conversation: { kind: "new" }, author_message: { text },
      working_target: references ? { kind: "selection_references", source_chapter_id: chapterId, references }
        : { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" }, cause: { kind: "author_request" }, ...BINDING, correlation_id: key },
  };
  const created = await challenged(started.baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/agent-runs",
    body.command_schema, await digestCreateAgentRun(body), key, (antiForgery) => createAgentRun({ baseUrl: started.baseUrl,
      projectId, fetchImpl, request: body, antiForgery, idempotencyKey: key }));
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  const options = { baseUrl: started.baseUrl, projectId, runId: created.effect.run_id, fetchImpl };
  const settle = async () => {
    let run = await getAgentRun(options);
    for (let attempt = 0; attempt < 8 && !["completed", "refused", "cancelled", "paused"].includes(run.status); attempt++) {
      await settleOnce();
      run = await getAgentRun(options);
    }
    return run;
  };
  const steerAfterDecision = async (message: string, steerKey: string, added: SelectionReference[]) => {
    const directory = await mkdtemp(join(tmpdir(), "storyos-grow-again-"));
    const hold = join(directory, "decision");
    await writeFile(hold, "hold");
    const worker = runWorker(["--once"], { extraEnv: { STORYOS_TEST_FAKE_DECISION_HOLD_PATH: hold } });
    try {
      await expect.poll(async () => (await getAgentRun(options)).decision.kind).toBe("prose_change");
      await steer(message, steerKey, added);
    } finally {
      await rm(directory, { recursive: true, force: true });
      await worker;
    }
  };
  const steer = async (message: string, steerKey: string, added: SelectionReference[]) => {
    const request: SteerAgentRunRequest = { command_schema: "storyos.command.steer-agent-run.request.v1",
      steer_agent_run_input: { conversation_id: created.conversation_id, author_message: { text: message },
        selection_references: added, ...BINDING, correlation_id: steerKey } };
    const steered = await challenged(started.baseUrl, fetchImpl, projectId, "POST",
      "/api/v1/projects/{project_id}/agent-runs/{run_id}/steering-inputs", request.command_schema,
      await digestSteerAgentRun(request), steerKey, (antiForgery) => steerAgentRun({ ...options, request,
        idempotencyKey: steerKey, antiForgery }));
    assert.equal(steered.effect.kind, "retained");
  };
  return { settle, steerAfterDecision };
}

function candidateReference(proposal: Proposal, operationId: string, revisionId: string, end: number): SelectionReference {
  return { kind: "proposal_candidate_text", chapter_id: proposal.chapter_id, proposal_id: proposal.proposal_id,
    operation_id: operationId, revision_id: revisionId, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
    start_offset: 0, end_offset: end };
}

function chapterReference(chapter: Chapter, start: [number, number], end: [number, number]): SelectionReference {
  const point = ([block, offset]: [number, number]) => ({
    manuscript_block_id: chapter.current_revision.blocks[block]!.manuscript_block_id, offset });
  return { kind: "chapter_text", chapter_id: chapter.chapter_id,
    base_authoritative_revision_id: chapter.current_revision.revision_id,
    coordinate_profile: "storyos.editor.utf16-code-unit.v1", start: point(start), end: point(end) };
}

/** The pending Operations with their Block and candidate text, in Block order. */
function pending(seeded: Seeded, texts: string[]) {
  return texts.map((candidate_text, block) => ({ manuscript_block_id:
    seeded.chapter.current_revision.blocks[block]!.manuscript_block_id, candidate_text, resolution: "pending",
    reservation_state: "unresolved" }));
}

/** Opens a Block Proposal on Block 0, then admits a growth over Block 1 that waits for the Worker. */
async function growOnce(started: Started, seeded: Seeded, ns: string) {
  const opened = await admit(started, seeded, "Revise this passage: keep the voice.", id(`${ns}61`));
  const run = await opened.settle();
  if (run.decision.kind !== "prose_change" || run.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened prose");
  }
  const before = await readProposal(started, seeded, run.decision.opened_proposal.proposal_id);
  const growth = await admit(started, seeded, GROW, id(`${ns}71`), [
    candidateReference(before, before.operation_id, before.revision_id, 4), chapterReference(seeded.chapter, [1, 0], [1, 4])]);
  return { before, growth, first: `${before.candidate_text}${REVISED}` };
}

/** The steering references after the first growth: one candidate reference for each Operation at the growth Revision. */
async function movedReferences(started: Started, seeded: Seeded, before: Proposal, first: string) {
  const after = await readProposal(started, seeded, before.proposal_id);
  const grownRevision = await queryPostgres(`SELECT revision_id::text FROM storyos.proposal_revisions
    WHERE proposal_id = '${before.proposal_id}'::uuid AND parent_revision_id = '${before.revision_id}'::uuid;`);
  const joined = after.operations.find((operation) => operation.manuscript_block_id
    === seeded.chapter.current_revision.blocks[1]!.manuscript_block_id)!;
  return { after, grownRevision, references: [
    { reference: candidateReference(before, before.operation_id, grownRevision, first.length),
      manuscript_block_ids: [before.manuscript_block_id] },
    { reference: candidateReference(before, joined.operation_id, grownRevision, `${SENTENCES[1]}${JOINED}`.length),
      manuscript_block_ids: [joined.manuscript_block_id] }] };
}

test("steering after a growth moves the references and grows the Proposal again over the next Block", async () => {
  await drainLeftoverWork();
  const ns = randomBytes(3).toString("hex");
  const seeded = await seedChapter(started, ns);
  const { before, growth, first } = await growOnce(started, seeded, ns);
  const added = chapterReference(seeded.chapter, [2, 0], [2, 4]);
  await growth.steerAfterDecision("Grow it again. SCRIPT:grow_proposal", id(`${ns}72`), [added]);
  const run = await growth.settle();
  const { after, grownRevision, references } = await movedReferences(started, seeded, before, first);
  assert.equal(run.status, "completed");
  assert.deepEqual(run.context.selection_references, [
    ...references.map((moved) => ({ ...moved, availability: { kind: "superseded", current_revision_id: after.revision_id } })),
    { reference: added, manuscript_block_ids: [seeded.chapter.current_revision.blocks[2]!.manuscript_block_id],
      availability: { kind: "current" } }]);
  assert.deepEqual(operations(after), pending(seeded, [first, `${SENTENCES[1]}${JOINED}${REVISED}`, `${SENTENCES[2]}${JOINED}`]));
  assert.equal(await revisions(before.proposal_id), "3 atomic");
  if (run.decision.kind !== "prose_change" || after.validation_receipt.kind !== "present") {
    throw new Error("expected a second growth Decision");
  }
  const validationReceiptId = after.validation_receipt.validation_receipt_id;
  assert.deepEqual(run.decision.locations!.map(({ manuscript_block_id, outcome }) => ({ manuscript_block_id, outcome })),
    after.operations.slice(1).map(({ manuscript_block_id, operation_id }) => ({ manuscript_block_id, outcome: {
      kind: "revised", proposal_id: before.proposal_id, operation_id, revision_id: after.revision_id,
      prior_revision_id: grownRevision, validation_receipt_id: validationReceiptId } })));
});

test("a second growth over a Block that is not adjacent to the run is refused", async () => {
  await drainLeftoverWork();
  const ns = randomBytes(3).toString("hex");
  const seeded = await seedChapter(started, ns);
  const { before, growth, first } = await growOnce(started, seeded, ns);
  const added = chapterReference(seeded.chapter, [2, 0], [3, 4]);
  await growth.steerAfterDecision("Join only the far line. SCRIPT:grow_gap", id(`${ns}72`), [added]);
  const run = await growth.settle();
  const { after, references } = await movedReferences(started, seeded, before, first);
  assert.deepEqual(run.context.selection_references, [
    ...references.map((moved) => ({ ...moved, availability: { kind: "current" } })),
    { reference: added, manuscript_block_ids: seeded.chapter.current_revision.blocks.slice(2)
      .map((block) => block.manuscript_block_id), availability: { kind: "current" } }]);
  assert.deepEqual(run.decision, { kind: "absent" });
  assert.deepEqual(operations(after), pending(seeded, [first, `${SENTENCES[1]}${JOINED}`]));
  assert.equal(await revisions(before.proposal_id), "2 atomic");
});

test("a Run that references two candidates of the grown Proposal gets the joinable marks", async () => {
  await drainLeftoverWork();
  const ns = randomBytes(3).toString("hex");
  const seeded = await seedChapter(started, ns);
  const { before, growth, first } = await growOnce(started, seeded, ns);
  await growth.settle();
  const { references } = await movedReferences(started, seeded, before, first);
  const again = await admit(started, seeded, GROW, id(`${ns}81`), [...references.map(({ reference }) => reference),
    chapterReference(seeded.chapter, [2, 0], [2, 4])]);
  await again.settle();
  assert.deepEqual(operations(await readProposal(started, seeded, before.proposal_id)),
    pending(seeded, [first, `${SENTENCES[1]}${JOINED}${REVISED}`, `${SENTENCES[2]}${JOINED}`]));
  assert.equal(await revisions(before.proposal_id), "3 atomic");
});
