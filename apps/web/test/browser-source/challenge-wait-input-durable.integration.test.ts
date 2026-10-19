import { afterAll, expect, it } from "vitest";

import type {
  DigestValue,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import { openEditorWorkspace } from "../../src/editor-session.ts";
import type { ReplaceSelectionEdit } from "../../src/editor-types.ts";
import { readJournalSnapshot, rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import { FIRST_APPEND_EDIT, SECOND_APPEND_EDIT } from "./local-edit-journal-append-fixture.ts";
import {
  OWNER,
  PROJECT,
  REVISION,
  SESSION,
  chapterRevision,
  closeTrackedDatabases,
  createAppliedAuthorEditResponse,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  requestHeaders,
  requireDigestValue,
  requireEditorReady,
  requireRequestBody,
  trackDatabase,
} from "./scenario.ts";

const ACTIVE_SESSION_KEY = `active_session:${OWNER}:${PROJECT}`;
const AUTHOR_EDIT_SCHEMA = "storyos.command.apply-author-edit.request.v1";
const T0 = Date.parse(FIRST_APPEND_EDIT.createdAt!);

type Timer = { callback: () => void; timeout: number; cleared: boolean };

function typed(body: string, text: string): ReplaceSelectionEdit {
  return { from: body.length, to: body.length, text, resultingBody: `${body}${text}`, inputOrigin: "typing" };
}

async function sha256Hex(body: string) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * The Server refuses the first Author Edit Challenge with HTTP 429 and `Retry-After` 7.
 * Each later Challenge is admitted. Each Author Edit applies on the current head, or the first is conflicted.
 */
async function openWaitEditor(options: {
  firstResult: "applied" | "conflicted";
  times: number[];
  rateLimit?: boolean;
  authorEditGate?: Promise<void>;
}) {
  const scenario = createBrowserScenario();
  let head = REVISION;
  let body = scenario.session.base_snapshot.materialized_revision.body;
  let canonicalSession: GetEditorSessionResponse = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
  const challengeKeys: string[] = [];
  const authorEdits: Array<{ key: string; expected: unknown; units: number }> = [];
  let digest: DigestValue | undefined;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
      if (request.command_schema === AUTHOR_EDIT_SCHEMA) {
        challengeKeys.push(String(request.idempotency_key));
        digest = requireDigestValue(request.canonical_command_digest, "command digest");
        if (challengeKeys.length === 1 && options.rateLimit !== false) {
          return new Response(JSON.stringify({ schema_id: "storyos.problem.v1",
            code: "challenge_rate_limited", message: "The command challenge rate limit is exceeded." }),
          { status: 429, headers: { "content-type": "application/json", "retry-after": "7" } });
        }
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-08-15T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(canonicalSession);
    if (path.endsWith("/manuscript/author-edits") && digest !== undefined) {
      const request = requireRequestBody(init);
      const key = requestHeaders(init).get("idempotency-key") ?? "";
      const units = request.author_edit_units as Array<{ normalized_primitives: Array<{ text: string }> }>;
      authorEdits.push({ key, expected: request.expected_authoritative_revision_id, units: units.length });
      await options.authorEditGate;
      const index = authorEdits.length;
      const id = (lead: string) => `018f0000-0000-7001-8000-00000000${lead}${String(index).padStart(2, "0")}`;
      if (index === 1 && options.firstResult === "conflicted") {
        const current = "018f0000-0000-7001-8000-000000000064";
        return jsonResponse({ schema_id: "storyos.command.apply-author-edit.response.v2",
          correlation_id: request.correlation_id, project_scope: scenario.project.project_scope,
          command_id: id("c1"), author_command_admission_id: id("c2"),
          receipt: { receipt_id: id("c3"), project_scope: scenario.project.project_scope, command_kind: "applyAuthorEdit",
            command_digest: digest, idempotency_key: key, producer_cause: "author_command_admission",
            author_command_admission_id: id("c2"), expected_heads: [head], prior_heads: [current], resulting_heads: [current],
            authoritative_revision_ids: [], proposal_revision_ids: [], authoritative_commit_ids: [],
            author_action_sequence: null, draft_artifact_refs: [], artifact_lifecycle_event_refs: [], condition_refs: [],
            result: "conflicted", created_at: "2026-08-15T08:00:00.000Z" },
          effect: { kind: "conflicted", reason: "stale_authoritative_head", current_authoritative_revision_id: current },
          completed_intent_record_id: request.completed_intent_record_id, local_intent_sequence: request.local_intent_sequence });
      }
      for (const unit of units) body = `${body}${unit.normalized_primitives[0]!.text}`;
      const prior = head;
      head = id("a1");
      canonicalSession = { ...canonicalSession, correlation_id: id("a5"), base_snapshot: {
        ...canonicalSession.base_snapshot, snapshot_id: id("a6"), project_activity_position: String(index),
        authoritative_head_revision_id: head, materialized_revision: chapterRevision(head, body),
        materialized_payload_digest: { algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
          value_hex_lowercase: await sha256Hex(body) },
        created_at: "2026-08-15T08:00:00.000Z" } };
      return jsonResponse(createAppliedAuthorEditResponse({ request, commandDigest: digest, idempotencyKey: key,
        commandId: id("a2"), authorCommandAdmissionId: id("a3"), receiptId: id("a4"),
        authoritativeRevisionId: head, authoritativeCommitId: id("a7"), projectActivityPosition: String(index),
        priorRevisionId: prior, body }));
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  const open = async () => {
    const workspace = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
      chapter: scenario.chapter, profile: scenario.profile, fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    requireEditorReady(workspace);
    trackDatabase(workspace.database, openDatabases);
    return workspace;
  };
  const workspace = await open();
  const timers: Timer[] = [];
  const failures: unknown[] = [];
  const times = [...options.times];
  const controller = (target: typeof workspace) => createEditorSessionWritingController({ workspace: target,
    baseUrl: location.origin, fetchImpl, onFailure: (error) => { failures.push(error); },
    now: () => times.length > 1 ? times.shift()! : times[0]!,
    setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, cleared: false }) - 1,
    clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].cleared = true; } });
  const writing = controller(workspace);
  return {
    workspace, writing, timers, failures, challengeKeys, authorEdits, open, controller,
    async fireIdle() {
      for (const timer of timers.filter((item) => item.timeout === 250 && !item.cleared)) {
        timer.cleared = true;
        timer.callback();
      }
      await writing.whenIdle();
    },
    async close() {
      writing.close();
      closeTrackedDatabases(openDatabases);
      sessionStorage.removeItem(ACTIVE_SESSION_KEY);
      await deleteJournal(scenario.journalName);
    },
  };
}

/** Captures "!?", gets the 429 wait, and then captures "x" and, after an idle pause, "y" during the wait. */
async function typeDuringWait(editor: Awaited<ReturnType<typeof openWaitEditor>>) {
  editor.writing.capture(FIRST_APPEND_EDIT, "typing");
  editor.writing.capture(SECOND_APPEND_EDIT, "typing");
  const flushed = editor.writing.flush();
  await expect.poll(() => editor.timers.some((timer) => timer.timeout === 7_000)).toBe(true);
  editor.writing.capture(typed("Base!?", "x"), "typing");
  editor.writing.capture(typed("Base!?x", "y"), "typing");
  await expect.poll(async () => (await readJournalSnapshot(editor.workspace)).records.length, { timeout: 2_000 }).toBe(4);
  // An async function that returns a promise waits for it, so return the flush in an object.
  return { flushed };
}

const WAIT_TIMES = [T0, T0 + 1, T0 + 2_000, T0 + 4_000];

afterAll(() => {
  expect(sessionStorage.getItem(ACTIVE_SESSION_KEY)).toBeNull();
});

it("saves input of a Challenge wait in the Journal at once and submits it in intent order on the installed base", async () => {
  const editor = await openWaitEditor({ firstResult: "applied", times: WAIT_TIMES });
  try {
    const { flushed } = await typeDuringWait(editor);
    const journal = await readJournalSnapshot(editor.workspace);
    const frozen = journal.groups.map((group) => group.journal_submission_group_id);
    expect({ frozen: frozen.length, follows: journal.records.map((record) => record.follows_submission_group) })
      .toEqual({ frozen: 1, follows: [undefined, undefined,
        { journal_submission_group_id: frozen[0] }, { journal_submission_group_id: frozen[0] }] });

    // A reload during the wait recovers the input of the wait and sends no request.
    const reopened = await editor.open();
    expect({ projection: await rebuildPendingProjection(reopened), challenges: editor.challengeKeys.length,
      authorEdits: editor.authorEdits.length })
      .toEqual({ projection: expect.objectContaining({ body: "Base!?xy", save_state: "saving", unsettled_intent_count: 4 }),
        challenges: 1, authorEdits: 0 });
    reopened.database.close();

    editor.timers.find((timer) => timer.timeout === 7_000)!.callback();
    await flushed;
    for (let round = 0; round < 4 && editor.writing.snapshot().projection.save_state !== "saved"; round += 1) {
      await editor.fireIdle();
    }
    const keys = editor.authorEdits.map((edit) => edit.key);
    expect({ failures: editor.failures, projection: editor.writing.snapshot().projection,
      authorEdits: editor.authorEdits, challengeKeys: editor.challengeKeys })
      .toEqual({ failures: [],
        projection: expect.objectContaining({ body: "Base!?xy", save_state: "saved", unsettled_intent_count: 0 }),
        // Each later group binds the installed head of the group before it.
        authorEdits: [
          { key: editor.challengeKeys[0], expected: REVISION, units: 2 },
          { key: keys[1], expected: "018f0000-0000-7001-8000-00000000a101", units: 1 },
          { key: keys[2], expected: "018f0000-0000-7001-8000-00000000a102", units: 1 },
        ],
        challengeKeys: [keys[0], keys[0], keys[1], keys[2]] });
    expect(new Set(keys).size).toBe(3);
  } finally {
    await editor.close();
  }
});

it("keeps the input of a wait in the Journal with needs_attention when the earlier group is conflicted", async () => {
  const editor = await openWaitEditor({ firstResult: "conflicted", times: WAIT_TIMES });
  try {
    const { flushed } = await typeDuringWait(editor);
    editor.timers.find((timer) => timer.timeout === 7_000)!.callback();
    await flushed;
    await editor.fireIdle();
    expect({ projection: editor.writing.snapshot().projection, authorEdits: editor.authorEdits.length,
      records: (await readJournalSnapshot(editor.workspace)).records.length })
      .toEqual({ projection: expect.objectContaining({ body: "Base!?xy", save_state: "needs_attention" }),
        authorEdits: 1, records: 4 });

    // After a reload, the records of the wait stay in local recovery, and no request sends them.
    editor.writing.close();
    const reopened = await editor.open();
    const recovered = editor.controller(reopened);
    await recovered.flush();
    for (const timer of editor.timers.filter((item) => !item.cleared)) timer.callback();
    await recovered.whenIdle();
    expect({ projection: recovered.snapshot().projection, authorEdits: editor.authorEdits.length,
      challenges: editor.challengeKeys.length })
      .toEqual({ projection: expect.objectContaining({ body: "Base!?xy", save_state: "needs_attention" }),
        authorEdits: 1, challenges: 2 });
    recovered.close();
  } finally {
    await editor.close();
  }
});

// Guard: the bound counts the Journal records only during a wait or while records follow a group.
it("keeps input open when a 240-unit group is in flight without a Challenge wait", async () => {
  let release!: () => void;
  const authorEditGate = new Promise<void>((resolve) => { release = resolve; });
  const editor = await openWaitEditor({ firstResult: "applied", times: [T0], rateLimit: false, authorEditGate });
  try {
    let body = "Base";
    for (let index = 0; index < 240; index += 1) {
      editor.writing.capture(typed(body, "x"), "typing");
      body = `${body}x`;
    }
    await expect.poll(() => editor.authorEdits.length, { timeout: 30_000 }).toBe(1);
    editor.writing.capture(typed(body, "y"), "typing");
    release();
    await editor.writing.flush();
    expect({ failures: editor.failures, projection: editor.writing.snapshot().projection,
      authorEdits: editor.authorEdits.map((edit) => edit.units) })
      .toEqual({ failures: [],
        projection: expect.objectContaining({ body: `${body}y`, save_state: "saved", unsettled_intent_count: 0 }),
        authorEdits: [240, 1] });
  } finally {
    await editor.close();
  }
}, 60_000);
