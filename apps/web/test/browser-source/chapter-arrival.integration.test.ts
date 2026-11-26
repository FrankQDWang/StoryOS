import { afterEach, beforeEach, expect, it } from "vitest";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import setCurrentChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-current-chapter.json";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { POINTER_STAY_MS, createChapterCopies } from "../../src/chapter-copies.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import { focusManuscriptEnd, manuscriptBody, manuscriptEditor, manuscriptIsEditable } from "../support/manuscript-surface.ts";
import {
  CHAPTER, OWNER, PROJECT, REVISION, SESSION, chapterRevision, closeTrackedDatabases, createAppliedActivityEvent,
  createBrowserScenario, deleteJournal, jsonResponse, trackDatabase,
} from "./scenario.ts";

// The Arriving Chapter of a selection in the contents (ADR 0105, sections 1 and 2).
const CHAPTER_B = "018f0000-0000-7001-8000-000000000803";
const CHAPTER_C = "018f0000-0000-7001-8000-000000000804";
const VOLUME = "018f0000-0000-7001-8000-000000000815";
const P = `/api/v1/projects/${PROJECT}`;
const READ_B = `GET ${P}/chapters/${CHAPTER_B}`;
const READ_C = `GET ${P}/chapters/${CHAPTER_C}`;
const CHALLENGE = `POST ${P}/anti-forgery-challenges`;
const SWITCH = `PUT ${P}/current-chapter`;
const AUTHOR_EDITS = `POST ${P}/manuscript/author-edits`;
const ACTIVITY = `GET ${P}/activity`;
const SEARCH = `POST ${P}/queries/manuscript-search`;
const SESSION_READ = `GET ${P}/editor-sessions/${SESSION}`;
const UNDO = `POST ${P}/author-actions/undo`;
const openProposals = (chapterId: string) => `GET ${P}/chapters/${chapterId}/open-proposals`;
const PROPOSAL = "018f0000-0000-7001-8000-000000000471";

type Text = { revision: string; title: string; body: string };
type Timer = { callback: () => void; cleared: boolean; fired: boolean };

async function sha256(body: string): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * A fake Server with Chapters A (current), B, and C. It serves only the requests of these tests, records each request
 * with no filter, and throws on each other request. `hold(name)` holds the next request of that name until
 * `release(name)`. `refuse` answers each request of the given names with a refusal. A switch settles a new Snapshot,
 * which the Editor Session base and the switch response name. `activity` is the frame of the Activity read. With
 * `assistance`, the writing assistant is available. With `undo`, the Editor Session has an Author Action to undo, and
 * the Server refuses the Undo. `lists` gives, for a Chapter, the Revision of the one open Proposal `PROPOSAL` in its
 * list. The Server does not find the Proposal itself.
 */
function arrivalServer() {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  const texts: Record<string, Text> = {
    [CHAPTER]: { revision: REVISION, title: "Chapter A", body: "Base" },
    [CHAPTER_B]: { revision: "018f0000-0000-7001-8000-000000000805", title: "Chapter B", body: "Beta" },
    [CHAPTER_C]: { revision: "018f0000-0000-7001-8000-000000000806", title: "Chapter C", body: "Gamma" },
  };
  let current = CHAPTER;
  let snapshot = { id: scenario.session.base_snapshot.snapshot_id, position: "0" };
  const holding = new Set<string>();
  const held = new Map<string, () => void>();
  // The idempotency keys of the Author Edit commands that the network refused.
  const editKeys = new Set<string>();
  const server = { requests: [] as string[], unexpected: [] as string[], texts, refuse: new Set<string>(),
    activity: undefined as unknown, assistance: false, undo: false, editKeys, lists: {} as Record<string, string>,
    hold(name: string) { holding.add(name); },
    isHeld(name: string) { return held.has(name); },
    release(name: string) { held.get(name)?.(); held.delete(name); } };
  const session = async () => {
    const text = texts[current]!;
    const base = { ...scenario.session.base_snapshot, chapter_id: current, authoritative_head_revision_id: text.revision,
      target_refs: [`manuscript:${current}`], materialized_revision: chapterRevision(text.revision, text.body),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: await sha256(text.body) } };
    return { ...scenario.session, ...(server.undo ? { author_undo_frontier_sequence: "1" } : {}),
      base_snapshot: { ...base, snapshot_id: snapshot.id, project_activity_position: snapshot.position } };
  };
  const open = () => ({ kind: "current_chapter" as const, current_chapter_id: current });
  const answer = async (name: string, init: RequestInit | undefined): Promise<Response> => {
    if (name === "GET /api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (name === "GET /api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: "Project A", revision: "1", open: open() }] });
    }
    if (name === `GET ${P}`) return jsonResponse({ ...scenario.project, project: { ...scenario.project.project, open: open() } });
    const chapterId = Object.keys(texts).find((id) => name === `GET ${P}/chapters/${id}`);
    if (chapterId !== undefined) {
      if (server.refuse.has(name)) {
        return jsonResponse({ schema_id: "storyos.problem.v1", code: "snapshot_expired",
          message: "The Snapshot is no longer available." }, 409);
      }
      const text = texts[chapterId]!;
      return jsonResponse({ ...scenario.chapter, project_activity_position: snapshot.position,
        chapter: { chapter_id: chapterId, title: text.title, current_revision: chapterRevision(text.revision, text.body) } });
    }
    const listed = Object.keys(texts).find((id) => name === openProposals(id));
    if (listed !== undefined) {
      return jsonResponse({ schema_id: "storyos.query.chapter-open-proposals.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000c71", project_scope: scope, chapter_id: listed,
        completeness: "complete", proposals: server.lists[listed] === undefined ? [] : [{ kind: "block_edit",
          proposal_id: PROPOSAL, revision_id: server.lists[listed], source: { kind: "agent_run_decision",
            run_id: "018f0000-0000-7001-8000-000000000472", decision_id: "018f0000-0000-7001-8000-000000000473" } }] });
    }
    if (name === `GET ${P}/proposals/${PROPOSAL}` && Object.keys(server.lists).length > 0) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (name === `GET ${P}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope, tree_revision: "3",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scope },
        volumes: [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [CHAPTER, CHAPTER_B, CHAPTER_C]
          .map((id, index) => ({ chapter_id: id, title: texts[id]!.title, order: String(index + 1) })) }] });
    }
    if (name === CHALLENGE) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-09-12T00:00:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (name === `POST ${P}/editor-sessions`) return jsonResponse(await session());
    if (name === UNDO && server.undo) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (name === SESSION_READ) {
      return jsonResponse({ ...await session(), schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (name === ACTIVITY) {
      return new Response(server.activity === undefined ? ""
        : `id: cursor-1\nevent: storyos.project-activity\ndata: ${JSON.stringify(server.activity)}\n\n`,
      { status: 200, headers: { "content-type": "text/event-stream" } });
    }
    if (name === SEARCH) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-search.response.v1", completeness: "complete",
        items: [{ chapter_id: CHAPTER_B, manuscript_block_id: "018f0000-0000-7001-8000-0000000000b1", start: "0",
          end: "4" }], lag: "0", projection_watermark: "1",
        source_snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scope } });
    }
    // With no network, the Author Edit command and the read of its outcome fail.
    if (name === AUTHOR_EDITS && server.refuse.has(name)) {
      editKeys.add(new Headers(init?.headers).get("idempotency-key") ?? "");
      throw new TypeError("Failed to fetch");
    }
    if ([...editKeys].some((key) => name === `GET ${P}/manuscript/author-edit-outcomes/${key}`)) {
      throw new TypeError("Failed to fetch");
    }
    if (name === `GET ${P}/assistance` && server.assistance) {
      return jsonResponse({ ...assistanceFixture, project_scope: scope });
    }
    if (name === SWITCH && server.refuse.has(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    if (name === SWITCH) {
      current = (JSON.parse(String(init?.body)) as { set_current_chapter_input: { chapter_id: string } })
        .set_current_chapter_input.chapter_id;
      const position = Number(snapshot.position) + 1;
      snapshot = { id: `018f0000-0000-7001-8000-00000000094${position}`, position: String(position) };
      const revision = texts[current]!.revision;
      return jsonResponse({ ...setCurrentChapterFixture,
        project: { ...setCurrentChapterFixture.project, open: open() },
        effect: { ...setCurrentChapterFixture.effect, base_snapshot_id: snapshot.id, current_chapter_id: current,
          project_activity_position: snapshot.position },
        receipt: { ...setCurrentChapterFixture.receipt, expected_heads: [revision], prior_heads: [revision],
          resulting_heads: [revision] } });
    }
    // The tests are not about these panels.
    if ([`GET ${P}/manuscript/statistics`, `GET ${P}/assistance`, `GET ${P}/skills`].includes(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
    }
    server.unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    server.requests.push(name);
    if (holding.delete(name)) await new Promise<void>((resolve) => { held.set(name, resolve); });
    return answer(name, init);
  };
  return { scenario, server, fetchImpl };
}

const openDatabases = new Set<IDBDatabase>();
// The workspace that opens again after a switch opens the Journal again, so the test closes each connection.
const openJournal = indexedDB.open.bind(indexedDB);
const unmounts: (() => void)[] = [];
const servers: ReturnType<typeof arrivalServer>["server"][] = [];

beforeEach(() => {
  Object.defineProperty(indexedDB, "open", { configurable: true, value: (name: string, version?: number) => {
    const request = openJournal(name, version);
    request.addEventListener("success", () => { trackDatabase(request.result, openDatabases); });
    return request;
  } });
});

afterEach(async () => {
  const unexpected = servers.splice(0).flatMap((server) => server.unexpected);
  for (const unmount of unmounts.splice(0)) unmount();
  Reflect.deleteProperty(indexedDB, "open");
  closeTrackedDatabases(openDatabases);
  document.body.replaceChildren();
  sessionStorage.clear();
  await deleteJournal(createBrowserScenario().journalName);
  expect(unexpected).toEqual([]);
});

/**
 * Opens the workspace of Chapter A with the writer. The stay timers of the pointer wait until the test runs them; each
 * other timer is a real timer.
 */
async function openWorkspace(prepare: (server: ReturnType<typeof arrivalServer>["server"]) => void = () => {}) {
  const { scenario, server, fetchImpl } = arrivalServer();
  servers.push(server);
  prepare(server);
  await deleteJournal(scenario.journalName);
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const stays: Timer[] = [];
  const view = mountStage1View(loaded.root, { ...loaded, writingTimers: {
    setTimeoutImpl: (callback, timeout) => timeout === POINTER_STAY_MS
      ? -stays.push({ callback, cleared: false, fired: false }) : globalThis.setTimeout(callback, timeout),
    clearTimeoutImpl: (timer) => {
      if (typeof timer === "number" && timer < 0) stays[-timer - 1]!.cleared = true;
      else globalThis.clearTimeout(timer);
    },
  } });
  unmounts.push(() => view.unmount());
  const root = loaded.root;
  const button = (chapterId: string) =>
    root.querySelector<HTMLButtonElement>(`.tree-panel button[data-chapter-id="${chapterId}"]`)!;
  await expect.poll(() => root.querySelector(`.tree-panel button[data-chapter-id="${CHAPTER_C}"]`)).not.toBeNull();
  await expect.poll(() => manuscriptIsEditable(manuscriptEditor(root, window))).toBe(true);
  const editor = () => manuscriptEditor(root, window);
  const shown = () => ({ title: root.querySelector(".editor-panel h2")?.textContent, body: manuscriptBody(editor()),
    editable: manuscriptIsEditable(editor()), alert: root.querySelector('.editor-panel [role="alert"]')?.textContent });
  /** The fact "no open suggestion" of the Arriving Chapter on the screen. */
  const fact = () => root.querySelector("[data-arrival-no-open-suggestion]")?.getAttribute("data-arrival-no-open-suggestion");
  const pointer = (chapterId: string, inside: boolean) => button(chapterId)
    .dispatchEvent(new PointerEvent(inside ? "pointerover" : "pointerout", { bubbles: true }));
  /** Runs each stay timer that waits. */
  const stay = () => {
    const waiting = stays.filter((timer) => !timer.cleared && !timer.fired);
    for (const timer of waiting) { timer.fired = true; timer.callback(); }
    return waiting.length;
  };
  /** The number of stay timers that a stay of the pointer on a Chapter starts. A Chapter with a copy starts none. */
  const stayTimers = (chapterId: string) => {
    pointer(chapterId, true);
    const started = stays.filter((timer) => !timer.cleared && !timer.fired).length;
    pointer(chapterId, false);
    return started;
  };
  /** Keeps the pointer on a Chapter until the tab has its copy. */
  const copy = async (chapterId: string) => {
    pointer(chapterId, true);
    expect(stay()).toBe(1);
    pointer(chapterId, false);
    await expect.poll(() => stayTimers(chapterId)).toBe(0);
  };
  // The page reads each panel of the open before a test starts.
  await expect.poll(() => server.requests.includes(openProposals(CHAPTER))).toBe(true);
  return { server, root, button, editor, shown, fact, pointer, stay, stays, stayTimers, copy };
}

/** Each title that the editor part showed, in order. The observer sees each change of the page. */
function watchTitles(root: HTMLElement) {
  const titles: string[] = [];
  const observer = new MutationObserver(() => {
    const title = root.querySelector(".editor-panel h2")?.textContent ?? "";
    if (titles.at(-1) !== title) titles.push(title);
  });
  observer.observe(root, { subtree: true, childList: true, characterData: true });
  return { titles, stop: () => observer.disconnect() };
}

it("reads a Chapter and its open suggestions when the pointer stays on it, with no command and no Challenge", async () => {
  const workspace = await openWorkspace();
  const before = workspace.server.requests.length;

  // The read of the open gave the copy of the Current Chapter.
  expect(workspace.stayTimers(CHAPTER)).toBe(0);
  // A pointer that only moves across the list starts no read.
  workspace.pointer(CHAPTER_B, true);
  workspace.pointer(CHAPTER_B, false);
  expect({ stays: workspace.stay(), cleared: workspace.stays.map((timer) => timer.cleared) })
    .toEqual({ stays: 0, cleared: [true] });

  workspace.pointer(CHAPTER_B, true);
  expect(workspace.stay()).toBe(1);
  workspace.pointer(CHAPTER_B, false);
  // A second stay on a Chapter with a copy starts no read.
  await expect.poll(() => workspace.stayTimers(CHAPTER_B)).toBe(0);
  expect(workspace.server.requests.slice(before)).toEqual([READ_B, openProposals(CHAPTER_B)]);
  // A Chapter has at most one such read in flight.
  workspace.server.hold(READ_C);
  workspace.pointer(CHAPTER_C, true);
  expect(workspace.stay()).toBe(1);
  await expect.poll(() => workspace.server.isHeld(READ_C)).toBe(true);
  workspace.pointer(CHAPTER_C, false);
  workspace.pointer(CHAPTER_C, true);
  expect(workspace.stay()).toBe(1);
  workspace.pointer(CHAPTER_C, false);
  workspace.server.release(READ_C);
  await expect.poll(() => workspace.stayTimers(CHAPTER_C)).toBe(0);
  expect(workspace.server.requests.slice(before)).toEqual([READ_B, openProposals(CHAPTER_B), READ_C,
    openProposals(CHAPTER_C)]);
});

it.each([
  { first: READ_B, second: openProposals(CHAPTER_B) },
  { first: openProposals(CHAPTER_B), second: READ_B },
])("writes each result of the pointer read when it answers, when $first answers first", async ({ first, second }) => {
  const workspace = await openWorkspace();
  workspace.server.hold(READ_B);
  workspace.server.hold(openProposals(CHAPTER_B));
  workspace.pointer(CHAPTER_B, true);
  expect(workspace.stay()).toBe(1);
  workspace.pointer(CHAPTER_B, false);
  await expect.poll(() => [first, second].every((name) => workspace.server.isHeld(name))).toBe(true);

  workspace.server.release(first);
  // The copy exists when the Chapter read answered, also while the read of the open suggestions waits.
  if (first === READ_B) await expect.poll(() => workspace.stayTimers(CHAPTER_B)).toBe(0);
  else expect(workspace.stayTimers(CHAPTER_B)).toBe(1);
  workspace.server.release(second);
  await expect.poll(() => workspace.stayTimers(CHAPTER_B)).toBe(0);
  workspace.server.hold(READ_B);
  workspace.button(CHAPTER_B).click();

  await expect.poll(() => ({ ...workspace.shown(), fact: workspace.fact() })).toEqual({ title: "Chapter B", body: "Beta",
    editable: false, alert: undefined, fact: "true" });
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  workspace.server.release(READ_B);
  await expect.poll(workspace.shown).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
});

it.each([
  // From a selection in the contents, no editable editor element of the earlier Chapter is in the document.
  { source: "contents", during: { title: "Chapter B", body: "Beta", editable: false, alert: undefined, editors: 0 } },
  { source: "a search result", during: { title: "Chapter A", body: "Base", editable: true, alert: undefined,
    editors: 1 } },
])("shows the selection from $source, and sends the requests of a switch of today in the same order", async ({
  source, during }) => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  if (source !== "contents") {
    const query = workspace.root.querySelector<HTMLInputElement>('input[name="manuscript-search-query"]')!;
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(query, "Beta");
    query.dispatchEvent(new Event("input", { bubbles: true }));
    query.form!.requestSubmit();
    await expect.poll(() => workspace.root.querySelector("[data-search-match] button")).not.toBeNull();
  }
  const before = workspace.server.requests.length;
  workspace.server.hold(READ_B);

  (source === "contents" ? workspace.button(CHAPTER_B)
    : workspace.root.querySelector<HTMLButtonElement>("[data-search-match] button")!).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  await expect.poll(() => ({ ...workspace.shown(),
    editors: document.querySelectorAll('[contenteditable="true"]').length })).toEqual(during);
  workspace.server.release(READ_B);

  await expect.poll(workspace.shown).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
  // The reopen reads the Chapter, the Project, the list, and the Editor Session at the same time. Then the panels of
  // the new editor part read with no order between them: the Activity read starts after a read of the Journal.
  const steps = () => workspace.server.requests.slice(before);
  await expect.poll(() => [...steps().slice(0, 7), ...steps().slice(7).sort()]).toEqual([READ_B, CHALLENGE, SWITCH,
    READ_B, `GET ${P}`, "GET /api/v1/projects", `GET ${P}/editor-sessions/${SESSION}`,
    ...[openProposals(CHAPTER_B), `GET ${P}/manuscript/tree`, ACTIVITY, `GET ${P}/manuscript/statistics`].sort()]);
});

it("replaces the shown text and the copy when the Chapter read of the switch gives a newer Revision", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.texts[CHAPTER_B] = { revision: "018f0000-0000-7001-8000-000000000807", title: "Chapter B",
    body: "Beta two" };
  workspace.server.refuse.add(SWITCH);
  workspace.server.hold(CHALLENGE);

  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(CHALLENGE)).toBe(true);
  const replaced = workspace.shown();
  workspace.server.release(CHALLENGE);
  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: "无法打开这一章。" });
  // The next selection shows the newer Revision from the copy before its Chapter read answers.
  workspace.server.hold(READ_B);
  workspace.button(CHAPTER_B).click();
  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  const again = workspace.shown();
  workspace.server.release(READ_B);
  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: "无法打开这一章。" });

  expect({ replaced, again }).toEqual({
    replaced: { title: "Chapter B", body: "Beta two", editable: false, alert: undefined },
    // The sentence of the earlier end stays until the next end, as today.
    again: { title: "Chapter B", body: "Beta two", editable: false, alert: "无法打开这一章。" },
  });
});

it.each([
  // The quiet step reads the outcome of the Author Edit that the network refused.
  { end: "the quiet step refuses", refuse: AUTHOR_EDITS, body: "Base!", alert: "无法切换章节：请先完成当前输入。",
    steps: ["outcome"] },
  // The command of today sends its request again one time after the refusal.
  { end: "the command is refused", refuse: SWITCH, body: "Base", alert: "无法打开这一章。",
    steps: [READ_B, CHALLENGE, SWITCH, SWITCH] },
  { end: "the Chapter read fails", refuse: READ_B, body: "Base", alert: "无法打开这一章：快照已过期。", steps: [READ_B] },
])("shows the earlier Chapter editable again with the sentence and the requests of today when $end", async ({
  refuse, body, alert, steps }) => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  workspace.server.refuse.add(refuse);
  if (refuse === AUTHOR_EDITS) {
    focusManuscriptEnd(workspace.editor(), window);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await expect.poll(() => workspace.server.requests.includes(AUTHOR_EDITS)).toBe(true);
  }
  const watch = watchTitles(workspace.root);
  const before = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();

  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body, editable: true, alert });
  watch.stop();
  // The display of the earlier Chapter stayed, so it reads its open suggestions no second time.
  expect({ titles: watch.titles, steps: workspace.server.requests.slice(before) }).toEqual({
    titles: ["Chapter B", "Chapter A"], steps: steps.map((step) => step !== "outcome" ? step
      : `GET ${P}/manuscript/author-edit-outcomes/${[...workspace.server.editKeys].join()}`) });
});

it("keeps the earlier editor during an IME composition, and the quiet step refuses as today", async () => {
  const workspace = await openWorkspace();
  await workspace.copy(CHAPTER_B);
  const editor = workspace.editor();
  focusManuscriptEnd(editor, window);
  editor.dispatchEvent(new CompositionEvent("compositionstart", { bubbles: true }));
  const before = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();

  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: "无法切换章节：请先完成当前输入。" });
  const during = { connected: editor.isConnected, steps: workspace.server.requests.slice(before) };
  // The composition ends in the earlier editor, and then a selection switches as today.
  editor.dispatchEvent(new CompositionEvent("compositionend", { bubbles: true, data: "" }));
  workspace.button(CHAPTER_B).click();
  await expect.poll(workspace.shown).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
  expect(during).toEqual({ connected: true, steps: [] });
});

it("keeps the earlier editor during an Author Undo, so a switch that does not reach the editor keeps the Undo", async () => {
  const workspace = await openWorkspace((server) => { server.undo = true; });
  await workspace.copy(CHAPTER_B);
  const editor = workspace.editor();
  workspace.server.refuse.add(READ_B);
  workspace.server.hold(SESSION_READ);
  focusManuscriptEnd(editor, window);
  await applyTrustedInput({ operation: "undo_latest_author_action" });
  await expect.poll(() => workspace.server.isHeld(SESSION_READ)).toBe(true);
  const selected = workspace.server.requests.length;

  workspace.button(CHAPTER_B).click();

  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true,
    alert: "无法打开这一章：快照已过期。" });
  const connected = editor.isConnected;
  const before = workspace.server.requests.length;
  const switchSteps = workspace.server.requests.slice(selected);
  workspace.server.release(SESSION_READ);
  // From the selection to the end, the switch sends the requests of today. The Undo continues after the end of the
  // switch: it sends its Challenge and its command. The command of today sends its request again one time after the
  // refusal, and the failure reads the statistics again.
  await expect.poll(() => workspace.server.requests.slice(before)).toEqual([CHALLENGE, UNDO, UNDO,
    `GET ${P}/manuscript/statistics`]);
  expect({ connected, switchSteps }).toEqual({ connected: true, switchSteps: [READ_B] });
});

it("sends no message to the assistant during the arrival", async () => {
  const workspace = await openWorkspace((server) => { server.assistance = true; });
  await workspace.copy(CHAPTER_B);
  const message = workspace.root.querySelector<HTMLTextAreaElement>('textarea[name="assistant-message"]')!;
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(message, "Help.");
  message.dispatchEvent(new Event("input", { bubbles: true }));
  const send = () => workspace.root.querySelector<HTMLButtonElement>("button.send-button")?.disabled;
  await expect.poll(send).toBe(false);
  workspace.server.hold(READ_B);

  workspace.button(CHAPTER_B).click();

  await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
  const during = { title: workspace.shown().title, disabled: send() };
  workspace.server.release(READ_B);
  await expect.poll(() => ({ editable: workspace.shown().editable, disabled: send() }))
    .toEqual({ editable: true, disabled: false });
  expect(during).toEqual({ title: "Chapter B", disabled: true });
});

it.each([
  { activity: "a Run event before the selection", facts: ["false"] },
  { activity: "no event", facts: ["true"] },
  { activity: "a Run event during the arrival", facts: ["true", "false"] },
])("keeps the fact of no open suggestion in a copy only while the tab learned of no Proposal change ($activity)",
  async ({ activity, facts }) => {
    const workspace = await openWorkspace((server) => {
      server.hold(ACTIVITY);
    });
    await workspace.copy(CHAPTER_B);
    if (activity !== "no event") workspace.server.activity = await runEvent();
    const processed = () => workspace.root.querySelector("[data-activity-processed-through]")
      ?.getAttribute("data-activity-processed-through");
    const consume = async () => {
      workspace.server.release(ACTIVITY);
      await expect.poll(processed).toBe(activity === "no event" ? "0" : "1");
    };
    if (activity !== "a Run event during the arrival") await consume();
    workspace.server.hold(READ_B);

    workspace.button(CHAPTER_B).click();

    await expect.poll(workspace.fact).toBe(facts[0]);
    await expect.poll(() => workspace.server.isHeld(READ_B)).toBe(true);
    // The screen follows the removal of the fact while the Chapter read of the switch waits.
    if (activity === "a Run event during the arrival") {
      await consume();
      await expect.poll(workspace.fact).toBe(facts[1]);
    }
    workspace.server.release(READ_B);
    await expect.poll(workspace.shown).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
  });

it("keeps the fact of no open suggestion from the settled display of the Chapter that the tab leaves", async () => {
  const workspace = await openWorkspace();
  workspace.button(CHAPTER_B).click();
  await expect.poll(workspace.shown).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
  workspace.server.hold(`GET ${P}/chapters/${CHAPTER}`);

  workspace.button(CHAPTER).click();

  await expect.poll(() => ({ ...workspace.shown(), fact: workspace.fact() })).toEqual({ title: "Chapter A", body: "Base", editable: false,
    alert: undefined, fact: "true" });
  await expect.poll(() => workspace.server.isHeld(`GET ${P}/chapters/${CHAPTER}`)).toBe(true);
  workspace.server.release(`GET ${P}/chapters/${CHAPTER}`);
  await expect.poll(workspace.shown).toEqual({ title: "Chapter A", body: "Base", editable: true, alert: undefined });
});

it("removes the fact of no open suggestion when a list of the same Proposal has a new Proposal Revision", async () => {
  const workspace = await openWorkspace((server) => {
    server.lists[CHAPTER_C] = "018f0000-0000-7001-8000-000000000474";
  });
  await workspace.copy(CHAPTER_C);
  await workspace.copy(CHAPTER_B);
  // Another tab edits the candidate text of the Proposal in Chapter C. The display of Chapter C reads the list again.
  workspace.server.lists[CHAPTER_C] = "018f0000-0000-7001-8000-000000000475";
  workspace.button(CHAPTER_C).click();
  await expect.poll(() => ({ title: workspace.shown().title, listed: workspace.server.requests
    .filter((name) => name === openProposals(CHAPTER_C)).length })).toEqual({ title: "Chapter C", listed: 2 });
  await expect.poll(() => workspace.root.querySelector("[data-save-state]")?.getAttribute("data-save-state"))
    .toBe("saved");
  workspace.server.hold(READ_B);

  workspace.button(CHAPTER_B).click();

  await expect.poll(() => ({ fact: workspace.fact(), held: workspace.server.isHeld(READ_B) }))
    .toEqual({ fact: "false", held: true });
  workspace.server.release(READ_B);
  await expect.poll(workspace.shown).toEqual({ title: "Chapter B", body: "Beta", editable: true, alert: undefined });
});

/** An `agent_run_created` event at stream position 1. */
async function runEvent() {
  const payload = { kind: "agent_run_created", project_agent_id: "018f0000-0000-7001-8000-000000000451",
    conversation_id: "018f0000-0000-7001-8000-000000000452",
    memory_settings_revision: "018f0000-0000-7001-8000-000000000453", run_id: "018f0000-0000-7001-8000-000000000454" };
  const base = await createAppliedActivityEvent({ eventId: "018f0000-0000-7001-8000-000000000455",
    commandId: "018f0000-0000-7001-8000-000000000456", receiptId: "018f0000-0000-7001-8000-000000000457",
    correlationId: "018f0000-0000-7001-8000-000000000458", revisionId: REVISION,
    commitId: "018f0000-0000-7001-8000-000000000459", sequence: "1", occurredAt: "2026-08-20T04:00:01.000Z" });
  return { ...base, event_schema: "storyos.event.agent-run-created.v1", event_kind: "agent_run_created",
    aggregate_ref: { kind: "agent_run", id: payload.run_id }, payload,
    payload_digest: { ...base.payload_digest, value_hex_lowercase: await sha256(JSON.stringify(payload)) } };
}

it("removes the fact of no open suggestion from each copy at a Proposal change, also for a read in flight", () => {
  const copies = createChapterCopies({ scope: createBrowserScenario().project.project_scope,
    baseUrl: location.origin, fetchImpl: async () => { throw new Error("no request"); } });
  const listed = (chapterId: string, proposalIds?: string[]) => copies.suggestionRead(chapterId)(proposalIds);
  const text = { title: "Chapter B", revisionId: "018f0000-0000-7001-8000-000000000805", blocks: [] };
  copies.write(CHAPTER_B, text);
  copies.write(CHAPTER_C, { ...text, title: "Chapter C" });
  listed(CHAPTER_B, []);
  listed(CHAPTER_C, []);
  const earlier = copies.suggestionRead(CHAPTER_B);
  const facts = () => [CHAPTER_B, CHAPTER_C].map((id) => copies.get(id)?.noOpenSuggestion);
  const before = facts();

  copies.proposalsChanged();
  earlier([]);
  const after = facts();
  listed(CHAPTER_B, []);
  copies.activity([{ event_id: "018f0000-0000-7001-8000-000000000455", event_kind: "agent_run_created" }]);
  const firstRun = facts();
  listed(CHAPTER_B, []);
  // A Run event that the copies saw before is no new change, and an event of another kind is no change.
  copies.activity([{ event_id: "018f0000-0000-7001-8000-000000000455", event_kind: "agent_run_created" },
    { event_id: "018f0000-0000-7001-8000-000000000460", event_kind: "authoritative_author_edit_applied" }]);
  const seen = facts();
  listed(CHAPTER_C, []);
  // A list of one Chapter that is not its last list is a change for each copy. The same list again is no change.
  listed(CHAPTER_C, ["018f0000-0000-7001-8000-000000000461"]);
  const newList = facts();
  listed(CHAPTER_B, []);
  listed(CHAPTER_C, ["018f0000-0000-7001-8000-000000000461"]);
  const sameList = facts();
  listed(CHAPTER_C);

  expect({ before, after, firstRun, seen, newList, sameList, incomplete: facts() }).toEqual({ before: [true, true],
    after: [false, false], firstRun: [false, false], seen: [true, false], newList: [false, false],
    sameList: [true, false], incomplete: [false, false] });
});
