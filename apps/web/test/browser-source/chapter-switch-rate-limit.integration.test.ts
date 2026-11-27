import { afterEach, beforeEach, expect, it } from "vitest";
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

const PROJECT_PATH = `/api/v1/projects/${PROJECT}`;
const CHALLENGE = `POST ${PROJECT_PATH}/anti-forgery-challenges`;
const readChapter = (chapterId: string) => `GET ${PROJECT_PATH}/chapters/${chapterId}`;

const LIMITED_BODY = JSON.stringify({ schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
  message: "The command challenge rate limit is exceeded." });

/** Runs the queued microtasks. The fake gives each response in microtasks, so no timer is necessary. */
async function drainMicrotasks() {
  for (let turn = 0; turn < 40; turn += 1) await Promise.resolve();
}

/**
 * A fake Server with Chapters A (current), B, and C. It serves only the reads of the workspace and the reads of
 * `chapters`, records each other request, and throws. It refuses the next Challenge of `limited` one time. When
 * `held` is true, it also refuses the Challenge after that one, but only when the test calls `release`. A switch
 * settles a new Snapshot, which the Editor Session base, the Chapter read, and the switch response name.
 */
function fakeServer(chapters: string[]) {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  let current = CHAPTER;
  let snapshot = { id: scenario.session.base_snapshot.snapshot_id, position: "0" };
  const server = { requests: [] as string[], challengeKeys: [] as string[], limited: undefined as string | undefined,
    held: false, release: undefined as (() => void) | undefined, switchRefused: false };
  const session = () => {
    const base = current === CHAPTER ? scenario.session.base_snapshot : { ...scenario.session.base_snapshot,
      chapter_id: current, authoritative_head_revision_id: REVISIONS[current]!, target_refs: [`manuscript:${current}`],
      materialized_revision: chapterRevision(REVISIONS[current]!, ""),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: EMPTY_BODY_DIGEST } };
    return { ...scenario.session,
      base_snapshot: { ...base, snapshot_id: snapshot.id, project_activity_position: snapshot.position } };
  };
  const open = () => ({ kind: "current_chapter" as const, current_chapter_id: current });
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    const name = `${method} ${path}`;
    const P = `/api/v1/projects/${PROJECT}`;
    server.requests.push(name);
    if (name === "GET /api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (name === "GET /api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: "Project A", revision: "1", open: open() }] });
    }
    if (name === `GET ${P}`) {
      return jsonResponse({ ...scenario.project, project: { ...scenario.project.project, open: open() } });
    }
    if (name === `GET ${P}/chapters/${CHAPTER}`) {
      return jsonResponse({ ...scenario.chapter, project_activity_position: snapshot.position });
    }
    const chapterId = chapters.find((id) => name === `GET ${P}/chapters/${id}`);
    if (chapterId !== undefined) {
      return jsonResponse({ ...scenario.chapter, project_activity_position: snapshot.position,
        chapter: { chapter_id: chapterId, title: TITLES[chapterId],
        current_revision: chapterRevision(REVISIONS[chapterId]!, "") } });
    }
    if (name === `GET ${P}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope, tree_revision: "3",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scope },
        volumes: [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [CHAPTER, CHAPTER_B, CHAPTER_C]
          .map((id, index) => ({ chapter_id: id, title: TITLES[id], order: String(index + 1) })) }] });
    }
    if (name === `POST ${P}/anti-forgery-challenges`) {
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
    if (name === `POST ${P}/editor-sessions`) return jsonResponse(session());
    if (name === `GET ${P}/editor-sessions/${SESSION}`) {
      return jsonResponse({ ...session(), schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (name === `GET ${P}/activity`) return emptyActivityStream();
    if (name === CURRENT_CHAPTER && !server.switchRefused) {
      current = (JSON.parse(String(init?.body)) as { set_current_chapter_input: { chapter_id: string } })
        .set_current_chapter_input.chapter_id;
      const position = Number(snapshot.position) + 1;
      snapshot = { id: `018f0000-0000-7001-8000-00000000094${position}`, position: String(position) };
      // The reopen opens the Chapter of the response, so the response names the Chapter of the request (#1875).
      return jsonResponse({ ...setCurrentChapterFixture,
        project: { ...setCurrentChapterFixture.project, open: open() },
        effect: { ...setCurrentChapterFixture.effect, base_snapshot_id: snapshot.id, current_chapter_id: current,
          project_activity_position: snapshot.position },
        receipt: { ...setCurrentChapterFixture.receipt, expected_heads: [REVISIONS[current]!],
          prior_heads: [REVISIONS[current]!], resulting_heads: [REVISIONS[current]!] } });
    }
    // The test is not about these panels, and a refused switch gets the same problem.
    const notFound = [`${P}/manuscript/statistics`, `${P}/assistance`, `${P}/skills`,
      ...[CHAPTER, ...chapters].map((id) => `${P}/chapters/${id}/open-proposals`)].map((route) => `GET ${route}`);
    if (notFound.includes(name) || name === CURRENT_CHAPTER) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };
  return { scenario, server, fetchImpl };
}

const openDatabases = new Set<IDBDatabase>();
// The writable workspace that opens again after a switch opens the Journal again, so the test closes each connection
// before it deletes the Journal.
const openJournal = indexedDB.open.bind(indexedDB);

beforeEach(() => {
  Object.defineProperty(indexedDB, "open", { configurable: true, value: (name: string, version?: number) => {
    const request = openJournal(name, version);
    request.addEventListener("success", () => { trackDatabase(request.result, openDatabases); });
    return request;
  } });
});
const unmounts: (() => void)[] = [];
// Each request that the fake does not serve. Each test asserts that it has none.
const unexpected: string[] = [];

afterEach(async () => {
  expect(unexpected.splice(0)).toEqual([]);
  for (const unmount of unmounts.splice(0)) unmount();
  Reflect.deleteProperty(indexedDB, "open");
  closeTrackedDatabases(openDatabases);
  document.body.replaceChildren();
  sessionStorage.clear();
  await deleteJournal(createBrowserScenario().journalName);
});

/** Opens the workspace, refuses the next Challenge of `limited`, and clicks Chapter B in the manuscript tree. */
async function clickWhileLimited(limited: string, held = false, chapters = [CHAPTER_B]) {
  const { scenario, server, fetchImpl } = fakeServer(chapters);
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
  const click = (chapterId: string) =>
    loaded.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${chapterId}"]`)!.click();
  await expect.poll(() => loaded.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
  server.limited = limited;
  server.held = held;
  const start = server.requests.length;
  click(CHAPTER_B);
  await expect.poll(() => timers.filter((timer) => timer.timeout === 7_000).length).toBe(1);
  // A writable workspace shows the saved state. A read-only recovery shows "needs_attention" and a failure.
  const shown = () => ({ title: loaded.root.querySelector(".editor-panel h2")?.textContent,
    save: loaded.root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
    failure: loaded.root.querySelector("[data-editor-failure]")?.getAttribute("data-editor-failure"),
    alert: loaded.root.querySelector('[role="alert"]')?.textContent,
    status: loaded.root.querySelector('.editor-panel > [role="status"]')?.textContent,
    waiting: loaded.root.querySelector("[data-chapter-switch-waiting]")?.getAttribute("data-chapter-switch-waiting") });
  return { server, timers, view, click, shown, waited: shown(), switches: () =>
    server.requests.filter((entry) => entry === CURRENT_CHAPTER).length,
    /** Each request from the click of Chapter B, with no filter. */
    sent: () => server.requests.slice(start) };
}

// The workspace that opens again after the switch reads its stored Editor Session, so it needs no Challenge.
it("keeps the switch in progress through a Challenge rate limit and opens the Chapter after the Retry-After", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER);
  const switchesBefore = opened.switches();

  opened.timers.find((timer) => timer.timeout === 7_000)!.callback();

  await expect.poll(opened.shown).toEqual({ title: "Chapter B", save: "saved", failure: "", alert: undefined, status: undefined, waiting: "" });
  expect({ waited: opened.waited, switchesBefore, switches: opened.switches(),
    challenges: opened.server.challengeKeys.length, keys: new Set(opened.server.challengeKeys).size,
    // The repeat does not read Chapter B again, so its Revision and the idempotency key stay the same.
    chapterB: opened.server.requests.filter((entry) => entry.endsWith(`/chapters/${CHAPTER_B}`)).length,
    sessions: opened.server.requests.filter((entry) => entry.endsWith("/editor-sessions")).length }).toEqual({
    // The Chapter read of the switch answers before the Challenge, so the Arriving Chapter B is on the screen (ADR 0105).
    waited: { title: "Chapter B", save: "saved", failure: "", alert: undefined, status: WAITING, waiting: CHAPTER_B },
    switchesBefore: 0,
    switches: 1,
    challenges: 2,
    keys: 1,
    chapterB: 2,
    sessions: 1,
  });
});

it("ends the wait of a switch when the author clicks another Chapter, and opens the later Chapter", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER, false, [CHAPTER_B, CHAPTER_C]);

  opened.click(CHAPTER_C);
  await expect.poll(opened.shown).toEqual({ title: "Chapter C", save: "saved", failure: "", alert: undefined, status: undefined, waiting: "" });
  opened.timers.find((timer) => timer.timeout === 7_000)!.callback();
  await drainMicrotasks();

  const sent = opened.sent();
  expect({ cleared: opened.timers.find((timer) => timer.timeout === 7_000)!.cleared, sent: sent.slice(0, 9),
    // The panels of the editor part of Chapter C read with no order between them.
    panels: sent.slice(9).sort(), shown: opened.shown() }).toEqual({
    cleared: true,
    // The switch to Chapter B ends at the refused Challenge. The switch to Chapter C then reopens the workspace.
    sent: [readChapter(CHAPTER_B), CHALLENGE, readChapter(CHAPTER_C), CHALLENGE, CURRENT_CHAPTER, readChapter(CHAPTER_C),
      `GET ${PROJECT_PATH}`, "GET /api/v1/projects", `GET ${PROJECT_PATH}/editor-sessions/${SESSION}`],
    panels: [`GET ${PROJECT_PATH}/activity`, `GET ${PROJECT_PATH}/chapters/${CHAPTER_C}/open-proposals`,
      `GET ${PROJECT_PATH}/manuscript/statistics`, `GET ${PROJECT_PATH}/manuscript/tree`],
    shown: { title: "Chapter C", save: "saved", failure: "", alert: undefined, status: undefined, waiting: "" },
  });
});

it("ends the wait of a switch and shows the earlier Chapter again when the author clicks the earlier Chapter", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER);

  opened.click(CHAPTER);

  await expect.poll(opened.shown).toEqual({ title: "Chapter A", save: "saved", failure: "", alert: undefined, status: undefined, waiting: "" });
  await drainMicrotasks();
  expect({ cleared: opened.timers.find((timer) => timer.timeout === 7_000)!.cleared, sent: opened.sent() })
    .toEqual({ cleared: true, sent: [readChapter(CHAPTER_B), CHALLENGE] });
});

it("shows the sentence of the wait only while the later switch also waits", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER, false, [CHAPTER_B, CHAPTER_C]);

  opened.server.limited = SET_CURRENT_CHAPTER;
  opened.click(CHAPTER_C);
  await expect.poll(() => opened.timers.filter((timer) => timer.timeout === 7_000).length).toBe(2);
  const laterWait = opened.shown();
  opened.timers.filter((timer) => timer.timeout === 7_000 && !timer.cleared).at(-1)!.callback();

  await expect.poll(opened.shown).toEqual({ title: "Chapter C", save: "saved", failure: "", alert: undefined, status: undefined, waiting: "" });
  expect(laterWait).toEqual({ title: "Chapter C", save: "saved", failure: "", alert: undefined, status: WAITING, waiting: CHAPTER_C });
});

it("ends the sentence of the wait when the repeat gets a refusal that is not a rate limit", async () => {
  const opened = await clickWhileLimited(SET_CURRENT_CHAPTER);

  opened.server.switchRefused = true;
  opened.timers.find((timer) => timer.timeout === 7_000)!.callback();

  await expect.poll(opened.shown).toEqual({ title: "Chapter A", save: "saved", failure: "", alert: "无法打开这一章。", status: undefined,
    waiting: "" });
  // A later click completes the command of the same key, so the next test starts with no command in flight.
  opened.server.switchRefused = false;
  opened.click(CHAPTER_B);
  await expect.poll(opened.shown).toEqual({ title: "Chapter B", save: "saved", failure: "", alert: undefined, status: undefined, waiting: "" });
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
