import { expect, it } from "vitest";

import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ApplyAuthorEditResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { createEditorSessionWritingController } from "../../src/editor-session-writing.ts";
import { FIRST_APPEND_EDIT, openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import {
  CHAPTER, SESSION, chapterRevision, createAppliedAuthorEditResponse, createBrowserScenario, jsonResponse,
  requestHeaders,
} from "./scenario.ts";

const SAVED_REVISION = "018f0000-0000-7001-8000-000000000034";
const SAVED_SNAPSHOT = "018f0000-0000-7001-8000-000000000038";
const CHALLENGE = "POST /api/v1/projects/018f0000-0000-7001-8000-000000000002/anti-forgery-challenges";
const AUTHOR_EDIT = "POST /api/v1/projects/018f0000-0000-7001-8000-000000000002/manuscript/author-edits";
const SESSION_READ = `GET /api/v1/projects/018f0000-0000-7001-8000-000000000002/editor-sessions/${SESSION}`;
const OUTCOME = "GET /api/v1/projects/018f0000-0000-7001-8000-000000000002/manuscript/author-edit-outcomes/";

async function bodyDigest(body: string): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** The base of the Editor Session after the save of "Base!", as a read of the Server gives it. */
async function savedBase() {
  return {
    ...createBrowserScenario().session.base_snapshot,
    snapshot_id: SAVED_SNAPSHOT,
    project_activity_position: "1",
    authoritative_head_revision_id: SAVED_REVISION,
    materialized_revision: chapterRevision(SAVED_REVISION, "Base!"),
    materialized_payload_digest: { algorithm: "sha256" as const, profile: "storyos.canonical-payload.sha256.v1",
      value_hex_lowercase: await bodyDigest("Base!") },
    created_at: "2026-08-15T08:00:00.000Z",
  };
}

/**
 * Saves one edit against a Server that answers each Author Edit with `respond`. The Server serves the Challenge and
 * the Author Edit, and the read of the Editor Session only when `read` is true. It records each request, in order.
 */
async function saveOnce(respond: (applied: ApplyAuthorEditResponse) => Promise<unknown>, read: boolean,
  outcome?: (applied: ApplyAuthorEditResponse) => unknown) {
  const test = await openJournalAppendTestWorkspace();
  const scenario = createBrowserScenario();
  const requests: string[] = [];
  let applied: ApplyAuthorEditResponse | undefined;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const name = `${init?.method ?? "GET"} ${path}`;
    requests.push(name);
    if (name === CHALLENGE) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (name === AUTHOR_EDIT) {
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      applied = createAppliedAuthorEditResponse({ request, commandDigest: await digestApplyAuthorEdit(request as never),
        idempotencyKey: requestHeaders(init).get("idempotency-key")!, body: "Base!" });
      return jsonResponse(await respond(applied));
    }
    if (outcome !== undefined && applied !== undefined && name.startsWith(OUTCOME)) {
      return jsonResponse({ schema_id: "storyos.query.apply-author-edit-outcome.response.v2",
        correlation_id: "018f0000-0000-7001-8000-000000000081", project_scope: applied.project_scope,
        outcome: { outcome_kind: "committed", response: outcome(applied) } });
    }
    if (read && name === SESSION_READ) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v2",
        base_snapshot: await savedBase(), author_undo_frontier_sequence: "1" });
    }
    throw new Error(`unexpected fetch ${name}`);
  };
  const timers: Array<{ callback: () => void; timeout: number; done: boolean }> = [];
  const failures: unknown[] = [];
  const writing = createEditorSessionWritingController({ workspace: test.workspace, baseUrl: location.origin,
    fetchImpl, onFailure: (error) => { failures.push(error); },
    setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, done: false }) - 1,
    clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].done = true; } });
  try {
    writing.capture(FIRST_APPEND_EDIT, "typing");
    // The idle step of the batch starts the save.
    await expect.poll(() => timers.some((timer) => !timer.done && timer.timeout === 250)).toBe(true);
    for (const timer of timers.filter((item) => !item.done && item.timeout === 250)) {
      timer.done = true;
      timer.callback();
    }
    if (outcome === undefined) {
      await expect.poll(() => ({ save: writing.snapshot().projection.save_state, failures }))
        .toEqual({ save: "saved", failures: [] });
    } else {
      // The response is lost, and a later step of the writing sends the outcome Query.
      await expect.poll(() => {
        for (const timer of timers.filter((item) => !item.done)) {
          timer.done = true;
          timer.callback();
        }
        return requests.some((name) => name.startsWith(OUTCOME));
      }).toBe(true);
    }
    await writing.whenIdle();
    return { requests, failures, session: test.workspace.session, projection: writing.snapshot().projection };
  } finally {
    writing.close();
    await test.close();
  }
}

it("saves with the Challenge and the Author Edit only, and takes the Editor Session state from the response",
  async () => {
    const base = await savedBase();
    const saved = await saveOnce(async (applied) => ({ ...applied,
      schema_id: "storyos.command.apply-author-edit.response.v3",
      editor_session_state: { editor_session: createBrowserScenario().session.editor_session, base_snapshot: base,
        author_undo_frontier_sequence: "1" } }), false);

    expect({ requests: saved.requests, failures: saved.failures, base: saved.session.base_snapshot,
      frontier: saved.session.author_undo_frontier_sequence, revision: saved.projection.authoritative_revision_id })
      .toEqual({ requests: [CHALLENGE, AUTHOR_EDIT], failures: [], base, frontier: "1", revision: SAVED_REVISION });
  });

it("reads the Editor Session after a save whose response has no Editor Session state", async () => {
  const saved = await saveOnce(async (applied) => applied, true);

  expect({ requests: saved.requests, failures: saved.failures, base: saved.session.base_snapshot.snapshot_id,
    chapter: saved.session.base_snapshot.chapter_id, frontier: saved.session.author_undo_frontier_sequence })
    .toEqual({ requests: [CHALLENGE, AUTHOR_EDIT, SESSION_READ], failures: [], base: SAVED_SNAPSHOT,
      chapter: CHAPTER, frontier: "1" });
});

// The outcome Query gives the first acknowledgement, and the state is not a part of it (ADR 0106 section 3).
it("does not settle a save from an outcome Query whose committed response has an Editor Session state", async () => {
  const base = await savedBase();
  const saved = await saveOnce(async () => { throw new TypeError("Failed to fetch"); }, false, (applied) => ({
    ...applied, schema_id: "storyos.command.apply-author-edit.response.v3",
    editor_session_state: { editor_session: createBrowserScenario().session.editor_session, base_snapshot: base,
      author_undo_frontier_sequence: "1" } }));

  expect({ save: saved.projection.save_state, base: saved.session.base_snapshot.snapshot_id === base.snapshot_id })
    .toEqual({ save: "saving", base: false });
});
