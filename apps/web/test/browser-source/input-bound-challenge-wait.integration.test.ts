import { expect, it } from "vitest";

import type {
  DigestValue,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import type { ChallengeAdmissionTimers } from "../../src/challenge-admission-wait.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
} from "../support/manuscript-surface.ts";
import {
  CHAPTER,
  OWNER,
  PROJECT,
  SESSION,
  chapterRevision,
  closeTrackedDatabases,
  createAppliedAuthorEditResponse,
  createBrowserScenario,
  deleteJournal,
  emptyActivityStream,
  jsonResponse,
  requestHeaders,
  requestResult,
  requireDigestValue,
  requireRequestBody,
  trackDatabase,
} from "./scenario.ts";

const AUTHOR_EDIT_SCHEMA = "storyos.command.apply-author-edit.request.v1";
const INPUT_BOUND_MESSAGE = "等待保存时输入过多，编辑器已停止输入。已写入本机的输入会在刷新页面后继续保存，最后几次输入可能没有保存。请先复制需要保留的文字，再刷新页面。";

type Timer = { callback: () => void; timeout: number; cleared: boolean };

function injectedTimers(timers: Timer[]): ChallengeAdmissionTimers {
  return {
    setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, cleared: false }) - 1,
    clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].cleared = true; },
  };
}

async function sha256Hex(body: string) {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function journalIntentCount(name: string): Promise<number> {
  const database = await requestResult(indexedDB.open(name));
  try {
    return await requestResult(database.transaction("intents").objectStore("intents").count());
  } finally {
    database.close();
  }
}

/**
 * The Server refuses the first Author Edit Challenge with a rate limit, and then admits each group.
 * Each Author Edit applies on the current head.
 */
function rateLimitedServer(scenario: ReturnType<typeof createBrowserScenario>) {
  let session: GetEditorSessionResponse = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
  let head = session.base_snapshot.authoritative_head_revision_id;
  let body = session.base_snapshot.materialized_revision.body;
  const challengeKeys: string[] = [];
  const authorEdits: string[] = [];
  let digest: DigestValue | undefined;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({
        schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013",
        owner_user_id: OWNER,
        projects: [{ project_scope: { owner_user_id: OWNER, project_id: PROJECT }, title: "Project A",
          lifecycle: { kind: "active" }, revision: "1", open: { kind: "current_chapter", current_chapter_id: CHAPTER } }],
      });
    }
    if (path === `/api/v1/projects/${PROJECT}`) return jsonResponse(scenario.project);
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}`) return jsonResponse(scenario.chapter);
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse({
        schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014",
        project_scope: scenario.project.project_scope,
        tree_revision: "2",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scenario.project.project_scope },
        volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000815", title: "Volume A", order: "1",
          chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }] }],
      });
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
      if (request.command_schema === AUTHOR_EDIT_SCHEMA) {
        challengeKeys.push(String(request.idempotency_key));
        digest = requireDigestValue(request.canonical_command_digest, "command digest");
        if (challengeKeys.length === 1) {
          return new Response(JSON.stringify({ schema_id: "storyos.problem.v1",
            code: "challenge_rate_limited", message: "The command challenge rate limit is exceeded." }),
          { status: 429, headers: { "content-type": "application/json", "retry-after": "7" } });
        }
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-08-15T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(session);
    if (path.endsWith("/activity")) return emptyActivityStream();
    if (path.endsWith("/manuscript/author-edits") && digest !== undefined) {
      const idempotencyKey = requestHeaders(init).get("idempotency-key") ?? "";
      const request = requireRequestBody(init);
      authorEdits.push(idempotencyKey);
      const units = request.author_edit_units as Array<{ normalized_primitives: Array<{ text: string }> }>;
      for (const unit of units) body = `${body}${unit.normalized_primitives[0]!.text}`;
      const index = authorEdits.length;
      const id = (lead: string) => `018f0000-0000-7001-8000-00000000${lead}${String(index).padStart(2, "0")}`;
      const prior = head;
      head = id("a1");
      session = { ...session, correlation_id: id("a5"), base_snapshot: { ...session.base_snapshot,
        snapshot_id: id("a6"), project_activity_position: String(index), authoritative_head_revision_id: head,
        materialized_revision: chapterRevision(head, body),
        materialized_payload_digest: { algorithm: "sha256", profile: "storyos.canonical-payload.sha256.v1",
          value_hex_lowercase: await sha256Hex(body) },
        created_at: "2026-08-15T08:00:00.000Z" } };
      return jsonResponse(createAppliedAuthorEditResponse({ request, commandDigest: digest, idempotencyKey,
        commandId: id("a2"), authorCommandAdmissionId: id("a3"), receiptId: id("a4"), authoritativeRevisionId: head,
        authoritativeCommitId: id("a7"), projectActivityPosition: String(index), priorRevisionId: prior, body }));
    }
    throw new Error(`unexpected fetch ${init?.method ?? "GET"} ${path}`);
  };
  return { fetchImpl, challengeKeys, authorEdits };
}

it("stops the Challenge wait when the input bound stops input, and recovers the input of the wait after a reload", async () => {
  const scenario = createBrowserScenario();
  const server = rateLimitedServer(scenario);
  const openDatabases = new Set<IDBDatabase>();
  let view: ReturnType<typeof mountStage1View> | undefined;
  const mount = async (timers: Timer[]) => {
    document.body.innerHTML = '<main id="app"></main>';
    const loaded = await loadStoryOSWebState({ documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    view = mountStage1View(loaded.root, { ...loaded, writingTimers: injectedTimers(timers) });
    if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
      trackDatabase(loaded.state.editor.database, openDatabases);
    }
    await expect.poll(() => loaded.root.querySelector("[contenteditable='true']") !== null).toBe(true);
    return loaded.root;
  };
  const fireIdle = (timers: Timer[]) => {
    for (const timer of timers.filter((item) => item.timeout === 250 && !item.cleared)) {
      timer.cleared = true;
      timer.callback();
    }
  };
  await deleteJournal(scenario.journalName);
  sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
  try {
    const timers: Timer[] = [];
    let root = await mount(timers);
    const editor = () => manuscriptEditor(root, window);
    focusManuscriptEnd(editor(), window);
    await applyTrustedInput({ operation: "insert_text", text: "!?" });
    await expect.poll(() => timers.some((timer) => timer.timeout === 250)).toBe(true);
    fireIdle(timers);
    await expect.poll(() => timers.some((timer) => timer.timeout === 7_000)).toBe(true);

    // Each input during the wait enters the Journal. The bound counts the frozen group and the wait (ADR 0065).
    let inputs = 0;
    while (manuscriptIsEditable(editor()) && inputs < 300) {
      await applyTrustedInput({ operation: "insert_text", text: "x" });
      inputs += 1;
    }
    const retryCleared = timers.filter((timer) => timer.timeout === 7_000).every((timer) => timer.cleared);
    // Time passes: each timer that is still set fires.
    for (const timer of timers.filter((item) => !item.cleared)) timer.callback();
    // No request is the expected result, so no event can end this wait.
    await new Promise((resolve) => { setTimeout(resolve, 100); });
    expect({ inputs, retryCleared, challenges: server.challengeKeys.length, authorEdits: server.authorEdits,
      alert: root.querySelector('[role="alert"]')?.textContent,
      saveState: root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
      editable: manuscriptIsEditable(editor()) })
      .toEqual({ inputs: 240, retryCleared: true, challenges: 1, authorEdits: [], alert: INPUT_BOUND_MESSAGE,
        saveState: "needs_attention", editable: false });

    // Input that was queued before the stop still enters the Journal.
    await expect.poll(() => journalIntentCount(scenario.journalName), { timeout: 10_000 }).toBe(240);
    view?.unmount();
    closeTrackedDatabases(openDatabases);
    const reloadTimers: Timer[] = [];
    root = await mount(reloadTimers);
    // The last input passed the bound, so it is not in the Journal.
    const recovered = `Base!?${"x".repeat(239)}`;
    expect({ body: manuscriptBody(editor()),
      saveState: root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
      unsettled: root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count") })
      .toEqual({ body: recovered, saveState: "saving", unsettled: "240" });
    await expect.poll(() => {
      fireIdle(reloadTimers);
      return root.querySelector("[data-save-state]")?.getAttribute("data-save-state");
    }, { timeout: 5_000 }).toBe("saved");
    // The frozen group goes first with its original key. The groups of the wait follow it in intent order.
    expect({ body: manuscriptBody(editor()), challengeKeys: server.challengeKeys.slice(0, 2),
      first: server.authorEdits[0], distinct: new Set(server.authorEdits).size === server.authorEdits.length })
      .toEqual({ body: recovered, challengeKeys: [server.challengeKeys[0], server.challengeKeys[0]],
        first: server.challengeKeys[0], distinct: true });
    expect(server.authorEdits.length).toBeGreaterThan(1);
  } finally {
    view?.unmount();
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
