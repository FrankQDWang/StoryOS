import { expect, it } from "vitest";

import type {
  DigestValue,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { createAuthorEditIdleController } from "../../src/author-edit-idle.ts";
import { openEditorWorkspace } from "../../src/editor-session.ts";
import type { PendingEditProjection } from "../../src/editor-types.ts";
import { FIRST_APPEND_EDIT, SECOND_APPEND_EDIT } from "./local-edit-journal-append-fixture.ts";
import {
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

const APPLIED_REVISION = "018f0000-0000-7001-8000-000000000034";

async function openRateLimitedEditor() {
  const scenario = createBrowserScenario();
  let canonicalSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
  };
  const appliedSession: GetEditorSessionResponse = {
    ...canonicalSession,
    correlation_id: "018f0000-0000-7001-8000-000000000037",
    base_snapshot: {
      ...canonicalSession.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000038",
      project_activity_position: "1",
      authoritative_head_revision_id: APPLIED_REVISION,
      materialized_revision: chapterRevision(APPLIED_REVISION, "Base!?"),
      materialized_payload_digest: {
        algorithm: "sha256",
        profile: "storyos.canonical-payload.sha256.v1",
        value_hex_lowercase: "d069d6b9c6ce7a4d9e97bb9d91c7096917777a2e31377dd589ba5720eb8a319c",
      },
      created_at: "2026-08-15T08:00:00.000Z",
    },
  };
  const challengeKeys: string[] = [];
  let digest: DigestValue | undefined;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
      if (request.command_schema !== "storyos.command.apply-author-edit.request.v1") {
        return jsonResponse({ nonce: "b".repeat(64), expires_at: "2026-08-15T08:05:00.000Z",
          limit_profile_revision: "storyos.foundation.absolute.v1" });
      }
      challengeKeys.push(String(request.idempotency_key));
      digest = requireDigestValue(request.canonical_command_digest, "command digest");
      if (challengeKeys.length === 1) {
        return new Response(JSON.stringify({ schema_id: "storyos.problem.v1",
          code: "challenge_rate_limited", message: "The command challenge rate limit is exceeded." }),
        { status: 429, headers: { "content-type": "application/json", "retry-after": "7" } });
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-08-15T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(canonicalSession);
    if (path.endsWith("/manuscript/author-edits") && digest !== undefined) {
      canonicalSession = appliedSession;
      return jsonResponse(createAppliedAuthorEditResponse({
        request: requireRequestBody(init),
        commandDigest: digest,
        idempotencyKey: requestHeaders(init).get("idempotency-key") ?? "",
        authoritativeRevisionId: APPLIED_REVISION,
      }));
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  const workspace = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: scenario.chapter, profile: scenario.profile, fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  requireEditorReady(workspace);
  trackDatabase(workspace.database, openDatabases);
  const timers: Array<{ callback: () => void; timeout: number; cleared: boolean }> = [];
  const projections: PendingEditProjection[] = [];
  const failures: unknown[] = [];
  const idle = createAuthorEditIdleController({ workspace, baseUrl: location.origin, fetchImpl,
    onProjection: (projection) => { projections.push(projection); },
    onFailure: (error) => { failures.push(error); },
    setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, cleared: false }) - 1,
    clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].cleared = true; } });
  await idle.persist(FIRST_APPEND_EDIT, "typing", "2026-08-15T08:00:00.000Z");
  await idle.persist(SECOND_APPEND_EDIT, "typing", "2026-08-15T08:00:00.001Z");
  const flushed = idle.flush();
  await expect.poll(() => timers.some((timer) => timer.timeout === 7_000)).toBe(true);
  return {
    workspace, idle, flushed, timers, projections, failures, challengeKeys,
    retryTimer: () => timers.find((timer) => timer.timeout === 7_000)!,
    async close() {
      idle.close();
      closeTrackedDatabases(openDatabases);
      await deleteJournal(scenario.journalName);
    },
  };
}

it("keeps an Author Edit saving through a Challenge rate limit and retries the same group", async () => {
  const editor = await openRateLimitedEditor();
  try {
    expect({ failures: editor.failures, saveState: editor.workspace.pending.save_state,
      challengeCount: editor.challengeKeys.length })
      .toEqual({ failures: [], saveState: "saving", challengeCount: 1 });

    editor.retryTimer().callback();
    await editor.flushed;

    expect({ failures: editor.failures, challengeKeys: new Set(editor.challengeKeys).size,
      challengeCount: editor.challengeKeys.length, last: editor.projections.at(-1) })
      .toEqual({ failures: [], challengeKeys: 1, challengeCount: 2,
        last: expect.objectContaining({ body: "Base!?", save_state: "saved", unsettled_intent_count: 0 }) });
  } finally {
    await editor.close();
  }
});

it("ends a Challenge rate-limit wait when the editor closes", async () => {
  const editor = await openRateLimitedEditor();
  try {
    editor.idle.close();
    await editor.idle.whenIdle();

    expect({ cleared: editor.retryTimer().cleared, challengeCount: editor.challengeKeys.length,
      failures: editor.failures }).toEqual({ cleared: true, challengeCount: 1, failures: [] });
  } finally {
    await editor.close();
  }
});
