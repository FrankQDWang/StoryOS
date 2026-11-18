import { afterEach, expect, it } from "vitest";
import setCurrentChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-current-chapter.json";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import {
  CHAPTER, OWNER, PROJECT, SESSION, chapterRevision, closeTrackedDatabases, createBrowserScenario, deleteJournal,
  emptyActivityStream, jsonResponse, trackDatabase,
} from "./scenario.ts";

// A Chapter switch waits for the Retry-After of a Command Challenge rate limit (#1679).
const CHAPTER_B = "018f0000-0000-7001-8000-000000000803";
const CHAPTER_C = "018f0000-0000-7001-8000-000000000804";
const REVISIONS: Record<string, string> = { [CHAPTER_B]: "018f0000-0000-7001-8000-000000000805",
  [CHAPTER_C]: "018f0000-0000-7001-8000-000000000806" };
const TITLES: Record<string, string> = { [CHAPTER]: "Chapter A", [CHAPTER_B]: "Chapter B", [CHAPTER_C]: "Chapter C" };
const VOLUME = "018f0000-0000-7001-8000-000000000815";
const EMPTY_BODY_DIGEST = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const CURRENT_CHAPTER = `PUT /api/v1/projects/${PROJECT}/current-chapter`;
const SET_CURRENT_CHAPTER = "storyos.command.set-current-chapter.request.v1";
// The sentence of the wait is the text of the author (#1689). The test gives it literally, so a changed character fails.
const WAITING = "操作有些频繁，稍等片刻会自动打开这一章。";

type Timer = { callback: () => void; timeout: number; cleared: boolean };

const LIMITED_BODY = JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
  message: "The command challenge rate limit is exceeded." });

/** Runs the queued microtasks. The fake gives each response in microtasks, so no timer is necessary. */
async function drainMicrotasks() {
  for (let turn = 0; turn < 40; turn += 1) await Promise.resolve();
}

/**
 * A fake Server with Chapters A (current), B, and C. It refuses the next Challenge of `limited` one time. When
 * `held` is true, it also refuses the Challenge after that one, but only when the test calls `release`.
 */
function fakeServer() {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  let current = CHAPTER;
  const server = { requests: [] as string[], challengeKeys: [] as string[], limited: undefined as string | undefined,
    held: false, release: undefined as (() => void) | undefined, switchRefused: false };
  const session = () => current === CHAPTER ? scenario.session : { ...scenario.session,
    base_snapshot: { ...scenario.session.base_snapshot, chapter_id: current,
      authoritative_head_revision_id: REVISIONS[current]!, target_refs: [`manuscript:${current}`],
      materialized_revision: chapterRevision(REVISIONS[current]!, ""),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: EMPTY_BODY_DIGEST } } };
  const open = () => ({ kind: "current_chapter" as const, current_chapter_id: current });
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    server.requests.push(`${method} ${path}`);
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: "Project A", revision: "1", open: open() }] });
    }
    if (path === `/api/v1/projects/${PROJECT}`) {
      return jsonResponse({ ...scenario.project, project: { ...scenario.project.project, open: open() } });
    }
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}`) return jsonResponse(scenario.chapter);
    const chapterId = path.split("/chapters/")[1];
    if (chapterId !== undefined && REVISIONS[chapterId] !== undefined) {
      return jsonResponse({ ...scenario.chapter, chapter: { chapter_id: chapterId, title: TITLES[chapterId],
        current_revision: chapterRevision(REVISIONS[chapterId]!, "") } });
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope, tree_revision: "3",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scope },
        volumes: [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [CHAPTER, CHAPTER_B, CHAPTER_C]
          .map((id, index) => ({ chapter_id: id, title: TITLES[id], order: String(index + 1) })) }] });
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = JSON.parse(String(init?.body)) as { command_schema: string; idempotency_key: string };
      if (request.command_schema === SET_CURRENT_CHAPTER) server.challengeKeys.push(request.idempotency_key);
      if (request.command_schema === server.limited) {
        // The body resolves in a microtask, so the test can wait for the end of the attempt with no timer.
        const refusal = new Response(LIMITED_BODY,
          { status: 429, headers: { "content-type": "application/json", "retry-after": "7" } });
        Object.defineProperty(refusal, "text", { value: async () => LIMITED_BODY });
        if (server.limited !== undefined && server.held && server.challengeKeys.length > 1) {
          return new Promise<Response>((resolve) => { server.release = () => resolve(refusal); });
        }
        if (!server.held) server.limited = undefined;
        return refusal;
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-09-12T00:00:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(session());
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({ ...session(), schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (path.endsWith("/activity")) return emptyActivityStream();
    if (`${method} ${path}` === CURRENT_CHAPTER && !server.switchRefused) {
      current = (JSON.parse(String(init?.body)) as { set_current_chapter_input: { chapter_id: string } })
        .set_current_chapter_input.chapter_id;
      return jsonResponse(setCurrentChapterFixture);
    }
    return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
  };
  return { scenario, server, fetchImpl };
}

const openDatabases = new Set<IDBDatabase>();
const unmounts: (() => void)[] = [];

afterEach(async () => {
  for (const unmount of unmounts.splice(0)) unmount();
  closeTrackedDatabases(openDatabases);
  document.body.replaceChildren();
  sessionStorage.clear();
  await deleteJournal(createBrowserScenario().journalName);
});

/** Opens the workspace, refuses the next Challenge of `limited`, and clicks Chapter B in the manuscript tree. */
async function clickWhileLimited(limited: string, held = false) {
  const { scenario, server, fetchImpl } = fakeServer();
  await deleteJournal(scenario.journalName);
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const timers: Timer[] = [];
  const view = mountStage1View(loaded.root, { ...loaded, writingTimers: {
    setTimeoutImpl: (callback, timeout) => timers.push({ callback, timeout, cleared: false }) - 1,
    clearTimeoutImpl: (timer) => { if (typeof timer === "number" && timers[timer]) timers[timer].cleared = true; },
  } });
  unmounts.push(() => view.unmount());
  if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
    trackDatabase(loaded.state.editor.database, openDatabases);
  }
  const click = (chapterId: string) =>
    loaded.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${chapterId}"]`)!.click();
  await expect.poll(() => loaded.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
  server.limited = limited;
  server.held = held;
  click(CHAPTER_B);
  await expect.poll(() => timers.filter((timer) => timer.timeout === 7_000).length).toBe(1);
  const shown = () => ({ title: loaded.root.querySelector(".editor-panel h2")?.textContent,
    alert: loaded.root.querySelector('[role="alert"]')?.textContent,
    status: loaded.root.querySelector('.editor-panel > [role="status"]')?.textContent,
    waiting: loaded.root.querySelector("[data-chapter-switch-waiting]")?.getAttribute("data-chapter-switch-waiting") });
  return { server, timers, view, click, shown, waited: shown(), switches: () =>
    server.requests.filter((entry) => entry === CURRENT_CHAPTER).length };
}

// The workspace that opens again after the switch reads its stored Editor Session, so it needs no Challenge.
it("keeps the switch in progress through a Challenge rate limit and opens the Chapter after the Retry-After", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER);
  const switchesBefore = opened.switches();

  opened.timers.find((timer) => timer.timeout === 7_000)!.callback();

  await expect.poll(opened.shown).toEqual({ title: "Chapter B", alert: undefined, status: undefined, waiting: "" });
  expect({ waited: opened.waited, switchesBefore, switches: opened.switches(),
    challenges: opened.server.challengeKeys.length, keys: new Set(opened.server.challengeKeys).size,
    // The repeat does not read Chapter B again, so its Revision and the idempotency key stay the same.
    chapterB: opened.server.requests.filter((entry) => entry.endsWith(`/chapters/${CHAPTER_B}`)).length,
    sessions: opened.server.requests.filter((entry) => entry.endsWith("/editor-sessions")).length }).toEqual({
    waited: { title: "Chapter A", alert: undefined, status: WAITING, waiting: CHAPTER_B },
    switchesBefore: 0,
    switches: 1,
    challenges: 2,
    keys: 1,
    chapterB: 2,
    sessions: 1,
  });
});

it("ends the wait of a switch when the author clicks another Chapter, and opens the later Chapter", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER);

  opened.click(CHAPTER_C);
  await expect.poll(opened.shown).toEqual({ title: "Chapter C", alert: undefined, status: undefined, waiting: "" });
  opened.timers.find((timer) => timer.timeout === 7_000)!.callback();
  await drainMicrotasks();

  expect({ cleared: opened.timers.find((timer) => timer.timeout === 7_000)!.cleared, switches: opened.switches(),
    chapterB: opened.server.requests.filter((entry) => entry.endsWith(`/chapters/${CHAPTER_B}`)).length,
    shown: opened.shown() }).toEqual({
    cleared: true, switches: 1, chapterB: 1,
    shown: { title: "Chapter C", alert: undefined, status: undefined, waiting: "" },
  });
});

it("shows the sentence of the wait only while the later switch also waits", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER);

  opened.server.limited = SET_CURRENT_CHAPTER;
  opened.click(CHAPTER_C);
  await expect.poll(() => opened.timers.filter((timer) => timer.timeout === 7_000).length).toBe(2);
  const laterWait = opened.shown();
  opened.timers.filter((timer) => timer.timeout === 7_000 && !timer.cleared).at(-1)!.callback();

  await expect.poll(opened.shown).toEqual({ title: "Chapter C", alert: undefined, status: undefined, waiting: "" });
  expect(laterWait).toEqual({ title: "Chapter A", alert: undefined, status: WAITING, waiting: CHAPTER_C });
});

it("ends the sentence of the wait when the repeat gets a refusal that is not a rate limit", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER);

  opened.server.switchRefused = true;
  opened.timers.find((timer) => timer.timeout === 7_000)!.callback();

  await expect.poll(opened.shown).toEqual({ title: "Chapter A", alert: "无法打开这一章。", status: undefined,
    waiting: "" });
  // A later click completes the command of the same key, so the next test starts with no command in flight.
  opened.server.switchRefused = false;
  opened.click(CHAPTER_B);
  await expect.poll(opened.shown).toEqual({ title: "Chapter B", alert: undefined, status: undefined, waiting: "" });
});

it("ends the wait of a switch when the author leaves the Project, and sends no request later", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER);
  const requests = opened.server.requests.length;

  opened.view.unmount();
  opened.timers.find((timer) => timer.timeout === 7_000)!.callback();
  await drainMicrotasks();

  expect({ waited: opened.waited.status, cleared: opened.timers.find((timer) => timer.timeout === 7_000)!.cleared,
    later: opened.server.requests.slice(requests), shown: opened.shown().status }).toEqual({
    waited: WAITING, cleared: true, later: [], shown: undefined });
});

it("ends a repeat in progress when the author leaves the Project, and starts no later wait", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER, true);

  opened.timers.find((timer) => timer.timeout === 7_000)!.callback();
  await expect.poll(() => opened.server.release).not.toBeUndefined();
  const requests = opened.server.requests.length;
  opened.view.unmount();
  opened.server.release!();
  await drainMicrotasks();

  expect({ timeouts: opened.timers.filter((timer) => timer.timeout === 7_000).length,
    later: opened.server.requests.slice(requests) }).toEqual({ timeouts: 1, later: [] });
});
