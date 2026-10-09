import { expect, it } from "vitest";

import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import type { CandidateSelectionEdit } from "../../src/local-edit-journal.ts";
import { readJournalSnapshot } from "../../src/local-edit-journal.ts";
import { FIRST_APPEND_EDIT, openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { BLOCK, createAppliedAuthorEditResponse, createBrowserScenario, jsonResponse, requestHeaders } from "./scenario.ts";

const PROPOSAL = "018f0000-0000-7001-8000-000000000c01";
const OPERATION = "018f0000-0000-7001-8000-000000000c02";
const FIRST_REVISION = "018f0000-0000-7001-8000-000000000c03";
const SECOND_REVISION = "018f0000-0000-7001-8000-000000000c04";
const THIRD_REVISION = "018f0000-0000-7001-8000-000000000c05";

function candidateEdit(revision: string, priorText: string, text: string): CandidateSelectionEdit {
  return { kind: "candidate_selection",
    target: { proposal_id: PROPOSAL, operation_id: OPERATION, revision_id: revision, manuscript_block_id: BLOCK },
    expectedProposalHeads: [revision], priorText, from: priorText.length, to: priorText.length, text,
    resultingBody: `${priorText}${text}` };
}

async function revisedResponse(request: Record<string, unknown>, init: RequestInit | undefined, revision: string,
  sequence: string) {
  const applied = createAppliedAuthorEditResponse({ request, commandDigest: await digestApplyAuthorEdit(request as never),
    idempotencyKey: requestHeaders(init).get("idempotency-key")!, commandId: `018f0000-0000-7001-8000-0000000003${sequence}1`,
    authorCommandAdmissionId: `018f0000-0000-7001-8000-0000000003${sequence}2`,
    receiptId: `018f0000-0000-7001-8000-0000000003${sequence}3` });
  const head = request.expected_authoritative_revision_id as string;
  return { ...applied, effect: { kind: "proposal_revised", proposal_revision_id: revision, author_action_sequence: sequence },
    receipt: { ...applied.receipt, result: "proposal_revised", prior_heads: [head], resulting_heads: [head],
      proposal_revision_ids: [revision], authoritative_revision_ids: [], authoritative_commit_ids: [],
      author_action_sequence: sequence } };
}

async function conflictedResponse(request: Record<string, unknown>, init: RequestInit | undefined) {
  const applied = createAppliedAuthorEditResponse({ request, commandDigest: await digestApplyAuthorEdit(request as never),
    idempotencyKey: requestHeaders(init).get("idempotency-key")! });
  const head = request.expected_authoritative_revision_id as string;
  return { ...applied, effect: { kind: "conflicted", reason: "proposal_head_present", current_authoritative_revision_id: head },
    receipt: { ...applied.receipt, result: "conflicted", prior_heads: [head], resulting_heads: [head],
      proposal_revision_ids: [], authoritative_revision_ids: [], authoritative_commit_ids: [], author_action_sequence: null } };
}

type Respond = (request: Record<string, unknown>, init: RequestInit | undefined) => Promise<unknown>;

/** A Server whose Author Edit responses a test releases one at a time. */
async function openWriting(options: { challengeRefusals?: number } = {}) {
  const test = await openJournalAppendTestWorkspace();
  const scenario = createBrowserScenario();
  const sent: Array<{ request: Record<string, unknown>; init: RequestInit | undefined }> = [];
  const waiting: Array<(respond: Respond) => Promise<void>> = [];
  const outcomes: Array<() => Promise<unknown>> = [];
  let refusals = options.challengeRefusals ?? 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      if (refusals > 0) {
        refusals -= 1;
        return new Response(JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
          message: "The command challenge rate limit is exceeded." }),
        { status: 429, headers: { "content-type": "application/json", "retry-after": "7" } });
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.includes("/editor-sessions/")) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (path.includes("/manuscript/author-edit-outcomes/")) {
      return jsonResponse({ schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000081", project_scope: scenario.project.project_scope,
        outcome: await outcomes.shift()!() });
    }
    if (!path.endsWith("/manuscript/author-edits")) throw new Error(`No request handler: ${path}`);
    const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
    sent.push({ request, init });
    return new Promise<Response>((resolve, reject) => {
      waiting.push(async (respond) => {
        try { resolve(jsonResponse(await respond(request, init))); } catch (error) { reject(error); }
      });
    });
  };
  const timers: Array<{ callback: () => void; timeout: number; done: boolean }> = [];
  const failures: unknown[] = [];
  let clock = Date.parse("2026-08-15T08:00:00.000Z");
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin, fetchImpl,
    onFailure: (error) => { failures.push(error); }, now: () => clock,
    setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, done: false }) - 1,
    clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].done = true; } });
  const bodies: string[] = [];
  writing.subscribe(() => { bodies.push(writing.snapshot().projection.body); });
  return {
    workspace: test.workspace, writing, sent, failures, bodies, outcomes,
    tick(milliseconds: number) { clock += milliseconds; },
    /** Runs every open timer with this timeout. */
    fire(timeout: number) {
      for (const timer of timers.filter((item) => !item.done && item.timeout === timeout)) {
        timer.done = true;
        timer.callback();
      }
    },
    waitTimer: (timeout: number) => expect.poll(() => timers.some((timer) => !timer.done && timer.timeout === timeout))
      .toBe(true),
    sentCount: (count: number) => expect.poll(() => sent.length).toBe(count),
    async reply(respond: Respond) {
      await expect.poll(() => waiting.length).toBe(1);
      await waiting.shift()!(respond);
    },
    candidate: () => writing.snapshot().candidates.get(`${PROPOSAL}:${OPERATION}`),
    async close() {
      writing.close();
      await test.close();
    },
  };
}

it("journals candidate input typed during its own edit and a Challenge wait against the new Proposal Revision", async () => {
  const session = await openWriting({ challengeRefusals: 1 });
  try {
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate", "!"), "typing");
    await session.waitTimer(250);
    session.fire(250);
    await session.waitTimer(7_000);
    expect(session.writing.canAcceptInput(false, { proposalId: PROPOSAL, operationId: OPERATION })).toBe(true);
    session.tick(10);
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate!", " More"), "typing");
    session.fire(7_000);
    await session.sentCount(1);
    session.tick(10);
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate! More", "."), "typing");
    expect(session.candidate()).toEqual({ revisions: [FIRST_REVISION], text: "Candidate! More." });

    await session.reply((request, init) => revisedResponse(request, init, SECOND_REVISION, "1"));
    await session.waitTimer(250);
    session.fire(250);
    await session.sentCount(2);
    await session.reply((request, init) => revisedResponse(request, init, THIRD_REVISION, "2"));
    await session.writing.whenIdle();

    expect(session.sent.map(({ request }) => ({ target: request.proposal_target,
      heads: request.expected_proposal_head_revision_ids })))
      .toEqual([FIRST_REVISION, SECOND_REVISION].map((revision) => ({ heads: [revision],
        target: { proposal_id: PROPOSAL, operation_id: OPERATION, revision_id: revision, manuscript_block_id: BLOCK } })));
    expect({ candidate: session.candidate(), saveState: session.writing.snapshot().projection.save_state,
      failures: session.failures }).toEqual({ saveState: "saved", failures: [],
      candidate: { revisions: [FIRST_REVISION, SECOND_REVISION, THIRD_REVISION], text: "Candidate! More." } });
  } finally {
    await session.close();
  }
});

it("journals a candidate paste after typing against the Revision that the typing settles", async () => {
  const session = await openWriting();
  try {
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate", "!"), "typing");
    session.tick(10);
    // The paste is a hard boundary, so it submits the typing first.
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate!", " pasted"), "paste");
    await session.reply((request, init) => revisedResponse(request, init, SECOND_REVISION, "1"));
    await session.reply((request, init) => revisedResponse(request, init, THIRD_REVISION, "2"));
    await session.writing.whenIdle();
    expect({ targets: session.sent.map(({ request }) =>
      (request.proposal_target as { revision_id: string }).revision_id), failures: session.failures })
      .toEqual({ targets: [FIRST_REVISION, SECOND_REVISION], failures: [] });
  } finally {
    await session.close();
  }
});

it("accepts typing in a candidate right after a paste in it, before the Journal append completes", async () => {
  const session = await openWriting();
  try {
    const candidate = { proposalId: PROPOSAL, operationId: OPERATION };
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate", " pasted"), "paste");
    expect(session.writing.canAcceptInput(false, candidate)).toBe(true);
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate pasted", "!"), "typing");
    await session.reply((request, init) => revisedResponse(request, init, SECOND_REVISION, "1"));
    await session.waitTimer(250);
    session.fire(250);
    await session.reply((request, init) => revisedResponse(request, init, THIRD_REVISION, "2"));
    await session.writing.whenIdle();
    expect({ targets: session.sent.map(({ request }) =>
      (request.proposal_target as { revision_id: string }).revision_id), text: session.candidate()?.text,
      failures: session.failures })
      .toEqual({ targets: [FIRST_REVISION, SECOND_REVISION], text: "Candidate pasted!", failures: [] });
  } finally {
    await session.close();
  }
});

it("keeps held candidate input visible and outside the Journal after a conflicted candidate edit", async () => {
  const session = await openWriting();
  try {
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate", "!"), "typing");
    await session.waitTimer(250);
    session.fire(250);
    await session.sentCount(1);
    session.tick(10);
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate!", "?"), "typing");
    await session.reply(conflictedResponse);
    await session.writing.whenIdle();

    expect({ requests: session.sent.length, failures: session.failures.map(String),
      saveState: session.writing.snapshot().projection.save_state, held: session.writing.hasIncompleteInput(),
      records: (await readJournalSnapshot(session.workspace)).records.length, candidate: session.candidate()?.text,
      accepts: session.writing.canAcceptInput(false, { proposalId: PROPOSAL, operationId: OPERATION }) })
      .toEqual({ requests: 1, failures: ["Error: Author Edit requires attention"], saveState: "needs_attention",
        held: true, records: 1, candidate: "Candidate!?", accepts: false });
  } finally {
    await session.close();
  }
});

it("holds candidate input outside the Journal while the outcome of its earlier edit is unknown", async () => {
  const session = await openWriting();
  try {
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate", "!"), "typing");
    await session.waitTimer(250);
    session.fire(250);
    await session.sentCount(1);
    session.outcomes.push(async () => ({ outcome_kind: "still_unknown", observation: {
      observation_kind: "admission_committed", command_id: "018f0000-0000-7001-8000-000000000311",
      author_command_admission_id: "018f0000-0000-7001-8000-000000000312", reconciliation_required: true } }));
    await session.reply(async () => { throw new TypeError("Failed to fetch"); });
    await session.writing.whenIdle();
    session.tick(10);
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate!", "?"), "typing");
    await session.writing.whenIdle();
    expect({ held: session.writing.hasIncompleteInput(), records: (await readJournalSnapshot(session.workspace)).records.length,
      saveState: session.writing.snapshot().projection.save_state }).toEqual({ held: true, records: 1, saveState: "saving" });

    const first = session.sent[0]!;
    session.outcomes.push(async () => ({ outcome_kind: "committed",
      response: await revisedResponse(first.request, first.init, SECOND_REVISION, "1") }));
    session.fire(250);
    await session.writing.whenIdle();
    await session.waitTimer(250);
    session.fire(250);
    await session.sentCount(2);
    expect({ held: session.writing.hasIncompleteInput(), target: session.sent[1]!.request.proposal_target, failures: session.failures })
      .toEqual({ held: false, failures: [], target: { proposal_id: PROPOSAL, operation_id: OPERATION,
        revision_id: SECOND_REVISION, manuscript_block_id: BLOCK } });
  } finally {
    await session.close();
  }
});

it("does not install a Journal read that started before newer input", async () => {
  const session = await openWriting();
  try {
    const read = session.writing.refresh();
    session.writing.capture(FIRST_APPEND_EDIT, "typing", [{ manuscript_block_id: BLOCK, text: "Base!" }]);
    await read;
    await session.writing.whenIdle();
    const shown = session.bodies.slice(session.bodies.indexOf("Base!"));
    expect({ shown: [...new Set(shown)], journal: session.writing.snapshot().projection.body, failures: session.failures })
      .toEqual({ shown: ["Base!"], journal: "Base!", failures: [] });
  } finally {
    await session.close();
  }
});

it("runs a journaled command after captured input enters the Journal, without a submission", async () => {
  const session = await openWriting();
  try {
    session.writing.setComposing(true);
    expect(await session.writing.runAfterQuiesce("journaled", async () => "ran"))
      .toEqual({ kind: "refused", reason: "incomplete_semantic_intent" });
    session.writing.setComposing(false);
    session.writing.capture(FIRST_APPEND_EDIT, "typing", [{ manuscript_block_id: BLOCK, text: "Base!" }]);
    const quiet = await session.writing.runAfterQuiesce("journaled", async (projection) => projection);
    expect({ quiet, sent: session.sent.length, records: (await readJournalSnapshot(session.workspace)).records.length })
      .toEqual({ quiet: { kind: "ran", result: expect.objectContaining({ body: "Base!", save_state: "saving" }) },
        sent: 0, records: 1 });
  } finally {
    await session.close();
  }
});

it("runs a settled command only after the captured input settles", async () => {
  const session = await openWriting();
  try {
    session.writing.capture(candidateEdit(FIRST_REVISION, "Candidate", "!"), "typing");
    const order: string[] = [];
    const quiet = session.writing.runAfterQuiesce("settled", async (projection) => {
      order.push("command");
      return projection.save_state;
    });
    await session.reply(async (request, init) => {
      order.push("settlement");
      return revisedResponse(request, init, SECOND_REVISION, "1");
    });
    expect({ quiet: await quiet, order }).toEqual({ quiet: { kind: "ran", result: "saved" }, order: ["settlement", "command"] });
  } finally {
    await session.close();
  }
});

it("refuses a settled command while the outcome of the input is unknown", async () => {
  const session = await openWriting();
  try {
    session.writing.capture(FIRST_APPEND_EDIT, "typing", [{ manuscript_block_id: BLOCK, text: "Base!" }]);
    session.outcomes.push(async () => ({ outcome_kind: "still_unknown", observation: {
      observation_kind: "admission_committed", command_id: "018f0000-0000-7001-8000-000000000311",
      author_command_admission_id: "018f0000-0000-7001-8000-000000000312", reconciliation_required: true } }));
    const quiet = session.writing.runAfterQuiesce("settled", async () => "ran");
    await session.reply(async () => { throw new TypeError("Failed to fetch"); });
    expect(await quiet).toEqual({ kind: "refused", reason: "unsettled_input" });
  } finally {
    await session.close();
  }
});
