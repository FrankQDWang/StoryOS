import { afterEach, beforeEach, expect, it } from "vitest";
import setCurrentChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-current-chapter.json";
import takeoverFixture from "../../../../generated/golden-wire/storyos-public-release-1/take-over-project-writer.json";
import type { EditorWriterProjection, GetEditorSessionResponse, SnapshotDescriptor }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import { focusManuscriptEnd, manuscriptBody, manuscriptEditor, manuscriptIsEditable }
  from "../support/manuscript-surface.ts";
import {
  CHAPTER, OWNER, PROJECT, REVISION, SESSION, chapterRevision, closeTrackedDatabases, createBrowserScenario,
  deleteJournal, emptyActivityStream, jsonResponse, trackDatabase,
} from "./scenario.ts";

// The takeover of the writer when a Project opens, and the screen of a read-only tab (ADR 0106 section 1).
const P = `/api/v1/projects/${PROJECT}`;
const CHAPTER_B = setCurrentChapterFixture.project.open.current_chapter_id;
const REVISION_B = "018f0000-0000-7001-8000-0000000019a1";
const OTHER_SESSION = "018f0000-0000-7001-8000-0000000019a2";
const SNAPSHOTS = ["018f0000-0000-7001-8000-0000000019b0", "018f0000-0000-7001-8000-0000000019b1",
  "018f0000-0000-7001-8000-0000000019b2", "018f0000-0000-7001-8000-0000000019b3",
  "018f0000-0000-7001-8000-0000000019b4"];
const PROTOCOL = "GET /api/v1/protocol";
const LIST = "GET /api/v1/projects";
const PROJECT_READ = `GET ${P}`;
const chapterRead = (chapterId: string) => `GET ${P}/chapters/${chapterId}`;
const openProposals = (chapterId: string) => `GET ${P}/chapters/${chapterId}/open-proposals`;
const SESSION_CREATE = `POST ${P}/editor-sessions`;
const SESSION_READ = `GET ${P}/editor-sessions/${SESSION}`;
const CHALLENGE = `POST ${P}/anti-forgery-challenges`;
const TAKEOVER = `POST ${P}/editor-sessions/${SESSION}/takeovers`;
const SWITCH = `PUT ${P}/current-chapter`;
const AUTHOR_EDITS = `POST ${P}/manuscript/author-edits`;
const OUTCOME = `GET ${P}/manuscript/author-edit-outcomes/{idempotency_key}`;
const TREE = `GET ${P}/manuscript/tree`;
const ACTIVITY = `GET ${P}/activity`;
const UNAVAILABLE_PANELS = [`GET ${P}/manuscript/statistics`, `GET ${P}/assistance`, `GET ${P}/skills`];
const snapshotRead = (snapshotId: string) => `GET ${P}/snapshots/${snapshotId}`;
const TEXTS: Record<string, { title: string; revision: string; body: string }> = {
  [CHAPTER]: { title: "Chapter A", revision: REVISION, body: "Base" },
  [CHAPTER_B]: { title: "Chapter B", revision: REVISION_B, body: "Text of Chapter B" },
};
const LOST_SENTENCE = "已在另一个标签页继续写作。";
const SECONDARY_SENTENCE = "另一个标签页正在写作。接管写作后可以编辑。";

async function sha256(body: string): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * A fake Server of one Project with Chapters A (current) and B, and one other tab. It serves only the requests of these
 * tests, records each request with no filter, and throws on each other request. At the start, the other tab holds the
 * writer with generation 1, so a created Editor Session of this tab is `secondary_session`. `stored` is the writer of a
 * read of the Session of this tab that the tab stored before. A takeover gives this Session the next generation and
 * settles a new Snapshot. `takeover` is the answer to each takeover: applied, compare failed, or a lost response.
 * `takeOverElsewhere` is a takeover by the other tab: the Session of this tab is then `superseded_by_takeover`, and an
 * Author Edit of this Session gets `412 editor_writer_stale`. A switch sets the Current Chapter. `hold(name)` holds
 * the next request of that name until `release`.
 */
function fakeServer({ serves, stored, takeover = "applied", writer = "elsewhere" }: {
  serves: string[]; stored?: EditorWriterProjection; takeover?: "applied" | "compare_failed" | "lost";
  writer?: "elsewhere" | "this_tab";
}) {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  const state = { current: CHAPTER, generation: 1, writer: writer === "this_tab" ? SESSION : OTHER_SESSION,
    superseded: false, snapshot: 0 };
  const log = { started: [] as string[], unexpected: [] as string[], active: new Map<string, number>(),
    takeovers: [] as unknown[] };
  const held: { name: string; resolve: () => void }[] = [];
  let holding = new Set<string>();
  const projection = (): EditorWriterProjection => state.writer === SESSION
    ? { kind: "current_writer", writer_generation: String(state.generation) }
    : { kind: "read_only", observed_writer_generation: String(state.generation),
      reason: state.superseded ? "superseded_by_takeover" : "secondary_session" };
  const open = () => ({ kind: "current_chapter" as const, current_chapter_id: state.current });
  const settle = () => { state.snapshot += 1; };
  const session = async (): Promise<GetEditorSessionResponse> => {
    const text = TEXTS[state.current]!;
    return { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1", writer: projection(),
      base_snapshot: { ...scenario.session.base_snapshot, snapshot_id: SNAPSHOTS[state.snapshot]!,
        project_activity_position: String(state.snapshot), chapter_id: state.current,
        authoritative_head_revision_id: text.revision, target_refs: [`manuscript:${state.current}`],
        materialized_revision: chapterRevision(text.revision, text.body),
        materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
          value_hex_lowercase: await sha256(text.body) } } };
  };
  const canonical = (): SnapshotDescriptor => ({ snapshot_id: SNAPSHOTS[state.snapshot]!, project_scope: scope,
    snapshot_kind: "canonical", project_activity_position: String(state.snapshot), source_watermarks: {},
    projection_generations: {}, redaction_profile: "storyos.author.v1", schema_profile: "storyos.public.release.1",
    replay_generation: "2", created_at: "2026-08-20T04:00:00.000Z", expires_at: null });
  const answer = async (name: string, init: RequestInit | undefined): Promise<Response> => {
    if (!serves.includes(name)) {
      log.unexpected.push(name);
      throw new Error(`unexpected request: ${name}`);
    }
    if (name === PROTOCOL) return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (name === LIST) {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: "Project A", revision: "1", open: open() }] });
    }
    if (name === PROJECT_READ) {
      return jsonResponse({ ...scenario.project, project: { ...scenario.project.project, open: open() } });
    }
    const chapterId = [CHAPTER, CHAPTER_B].find((id) => name === chapterRead(id));
    if (chapterId !== undefined) {
      const text = TEXTS[chapterId]!;
      return jsonResponse({ ...scenario.chapter, project_activity_position: String(state.snapshot),
        chapter: { chapter_id: chapterId, title: text.title,
          current_revision: chapterRevision(text.revision, text.body) } });
    }
    const listed = [CHAPTER, CHAPTER_B].find((id) => name === openProposals(id));
    if (listed !== undefined) {
      return jsonResponse({ schema_id: "storyos.query.chapter-open-proposals.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000c71", project_scope: scope, chapter_id: listed,
        completeness: "complete", proposals: [] });
    }
    if (name === CHALLENGE) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (name === SESSION_CREATE) return jsonResponse({ ...await session(), schema_id: scenario.session.schema_id });
    if (name === SESSION_READ) {
      const read = await session();
      return jsonResponse(stored === undefined || state.writer === SESSION ? read : { ...read, writer: stored });
    }
    if (name === TREE) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope, tree_revision: "3",
        snapshot: canonical(), volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000815", title: "Volume A",
          order: "1", chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" },
            { chapter_id: CHAPTER_B, title: "Chapter B", order: "2" }] }] });
    }
    if (name === snapshotRead(SNAPSHOTS[state.snapshot]!)) {
      return jsonResponse({ schema_id: "storyos.query.snapshot.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000015", project_scope: scope, snapshot: canonical() });
    }
    if (name === ACTIVITY) return emptyActivityStream();
    if (UNAVAILABLE_PANELS.includes(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "service_unavailable", message: "Unavailable." }, 503);
    }
    if (name === TAKEOVER) {
      const request = JSON.parse(String(init?.body)) as { observed_writer_generation: string };
      log.takeovers.push(request.observed_writer_generation);
      if (takeover === "lost") throw new TypeError("Failed to fetch");
      const receipt = { ...takeoverFixture.receipt, expected_heads: [REVISION], prior_heads: [REVISION],
        resulting_heads: [REVISION] };
      if (takeover === "compare_failed") {
        return jsonResponse({ ...takeoverFixture, receipt, result: { kind: "takeover_compare_failed",
          observed_writer_generation: request.observed_writer_generation,
          current_writer_generation: String(state.generation), current_writer_projection: projection(),
          current_snapshot_id: SNAPSHOTS[state.snapshot]!, current_snapshot_activity_position: String(state.snapshot),
          current_heads: [REVISION], reason: "writer_generation_advanced_after_admission" } });
      }
      const prior = state.writer;
      state.writer = SESSION;
      state.generation += 1;
      settle();
      return jsonResponse({ ...takeoverFixture, receipt, result: { ...takeoverFixture.result,
        prior_editor_session_id: prior, prior_writer_generation: String(state.generation - 1),
        resulting_editor_session_id: SESSION, resulting_writer_generation: String(state.generation),
        resulting_heads: [REVISION], resulting_snapshot_id: SNAPSHOTS[state.snapshot]!,
        resulting_snapshot_activity_position: String(state.snapshot) } });
    }
    if (name === SWITCH) {
      state.current = (JSON.parse(String(init?.body)) as { set_current_chapter_input: { chapter_id: string } })
        .set_current_chapter_input.chapter_id;
      settle();
      return jsonResponse({ ...setCurrentChapterFixture,
        project: { ...setCurrentChapterFixture.project, open: open() },
        effect: { ...setCurrentChapterFixture.effect, base_snapshot_id: SNAPSHOTS[state.snapshot]!,
          current_chapter_id: state.current, project_activity_position: String(state.snapshot) },
        receipt: { ...setCurrentChapterFixture.receipt, expected_heads: [REVISION_B], prior_heads: [REVISION_B],
          resulting_heads: [REVISION_B] } });
    }
    if (name === OUTCOME) {
      // The Server refuses a stale writer before the Challenge is consumed (`author_edit_outcome.rs`).
      return jsonResponse({ schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000081", project_scope: scope, outcome: {
          outcome_kind: "still_unknown", observation: { observation_kind: "challenge_issued",
            expires_at: new Date(Date.now() + 60_000).toISOString() } } });
    }
    if (name !== AUTHOR_EDITS) throw new Error(`no answer for ${name}`);
    // An Author Edit of a Session that is not the writer (`author_edit.rs` of `storyos-server`).
    return jsonResponse({ schema_id: "storyos.problem.v1", code: "editor_writer_stale",
      message: "The Editor Session is not the current writer." }, 412);
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    // The outcome query names the idempotency key of the Author Edit.
    const name = path.startsWith(`${P}/manuscript/author-edit-outcomes/`) ? OUTCOME : `${init?.method ?? "GET"} ${path}`;
    log.started.push(name);
    log.active.set(name, (log.active.get(name) ?? 0) + 1);
    try {
      if (holding.delete(name)) await new Promise<void>((resolve) => { held.push({ name, resolve }); });
      return await answer(name, init);
    } finally {
      log.active.set(name, log.active.get(name)! - 1);
    }
  };
  return {
    scenario, log, fetchImpl,
    takeOverElsewhere() {
      state.writer = OTHER_SESSION;
      state.generation += 1;
      state.superseded = true;
      settle();
    },
    /** Another Session holds the writer with the next generation, and this Session is `secondary_session`. */
    giveWriterElsewhere() {
      state.writer = OTHER_SESSION;
      state.generation += 1;
    },
    currentChapter: () => state.current,
    hold(names: string[]) { holding = new Set(names); },
    release() { for (const { resolve } of held.splice(0)) resolve(); },
    inFlight: () => [...log.active].filter(([, count]) => count > 0).map(([name]) => name).sort(),
    count: (name: string) => log.started.filter((entry) => entry === name).length,
  };
}

type Server = ReturnType<typeof fakeServer>;
const unmounts: (() => void)[] = [];
const openDatabases = new Set<IDBDatabase>();
const openJournal = indexedDB.open.bind(indexedDB);

// The view opens the Journal at each open, so the test closes each connection before it deletes the Journal.
beforeEach(() => {
  Object.defineProperty(indexedDB, "open", { configurable: true, value: (name: string, version?: number) => {
    const request = openJournal(name, version);
    request.addEventListener("success", () => { trackDatabase(request.result, openDatabases); });
    return request;
  } });
});

afterEach(async () => {
  for (const unmount of unmounts.splice(0)) unmount();
  Reflect.deleteProperty(indexedDB, "open");
  closeTrackedDatabases(openDatabases);
  document.body.replaceChildren();
  sessionStorage.clear();
  await deleteJournal(createBrowserScenario().journalName);
});

/** Loads the page of the Project, as a new tab or a new load of the tab does. */
async function loadPage(server: Server) {
  for (const unmount of unmounts.splice(0)) unmount();
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const view = mountStage1View(loaded.root, loaded);
  unmounts.push(() => view.unmount());
  const root = loaded.root;
  const editor = () => root.querySelector<HTMLElement>("[data-manuscript-editor]");
  const shown = () => ({ title: root.querySelector(".editor-panel h2")?.textContent,
    footer: root.querySelector("[data-save-state]")?.textContent,
    editable: editor() === null ? undefined : manuscriptIsEditable(editor()!),
    reason: root.querySelector("[data-writer-read-only]")?.textContent,
    button: root.querySelector("button[data-take-over-writer]")?.textContent });
  const takeOver = () => root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
  return { root, shown, takeOver };
}

/** The reads of an open of Chapter A, with no Editor Session request. */
const OPEN = [PROTOCOL, LIST, PROJECT_READ, chapterRead(CHAPTER), openProposals(CHAPTER), TREE, ACTIVITY,
  ...UNAVAILABLE_PANELS];
/** The requests of a takeover and its reopen with the takeover check of the Journal (#1897). */
const TAKEOVER_STEPS = [CHALLENGE, TAKEOVER, SESSION_READ, snapshotRead(SNAPSHOTS[1]!), snapshotRead(SNAPSHOTS[2]!)];

it("takes over the writer with no click when a new tab opens a Project, and says so while the takeover runs",
  async () => {
    const server = fakeServer({ serves: [...OPEN, SESSION_CREATE, ...TAKEOVER_STEPS] });
    server.hold([TAKEOVER]);
    const page = await loadPage(server);

    // While the takeover runs, the editor shows the Current Chapter read-only.
    await expect.poll(server.inFlight).toEqual([TAKEOVER]);
    expect(page.shown()).toEqual({ title: "Chapter A", footer: "正在接管写作…", editable: false, reason: undefined,
      button: undefined });
    server.release();

    await expect.poll(page.shown).toEqual({ title: "Chapter A", footer: "已保存", editable: true, reason: undefined,
      button: undefined });
    expect({ takeovers: server.log.takeovers, unexpected: server.log.unexpected })
      .toEqual({ takeovers: ["1"], unexpected: [] });
  });

it("opens the Chapter of the address after the automatic takeover gave the writer to the new tab", async () => {
  const server = fakeServer({ serves: [...OPEN, SESSION_CREATE, ...TAKEOVER_STEPS, chapterRead(CHAPTER_B),
    openProposals(CHAPTER_B), SWITCH, snapshotRead(SNAPSHOTS[3]!)] });
  history.replaceState(null, "", `${location.pathname}?chapter=${CHAPTER_B}`);
  const page = await loadPage(server);

  // A read-only tab cannot change the Current Chapter, so the address waits for the end of the takeover.
  await expect.poll(() => ({ ...page.shown(), current: server.currentChapter(), inFlight: server.inFlight(),
    alert: page.root.querySelector("[role=alert]")?.textContent }))
    .toEqual({ title: "Chapter B", footer: "已保存", editable: true, reason: undefined, button: undefined,
      current: CHAPTER_B, inFlight: [], alert: undefined });
  expect({ address: location.search, takeovers: server.log.takeovers, unexpected: server.log.unexpected })
    .toEqual({ address: "", takeovers: ["1"], unexpected: [] });
});

/** A new load of a tab whose stored Editor Session has the writer `stored`. */
function storedSession(stored: EditorWriterProjection, more: Partial<Parameters<typeof fakeServer>[0]> = {}) {
  sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  return fakeServer({ ...more, serves: [...OPEN, SESSION_READ, ...(more.serves ?? [])], stored });
}

it("sends no takeover at the open of a stored Session with binding_invalid", async () => {
  const server = storedSession({ kind: "read_only", observed_writer_generation: "1", reason: "binding_invalid" });
  const page = await loadPage(server);

  await expect.poll(page.shown).toEqual({ title: "Chapter A", editable: false, footer: "需要处理", reason: undefined,
    button: undefined });
  expect({ takeovers: server.log.takeovers, unexpected: server.log.unexpected })
    .toEqual({ takeovers: [], unexpected: [] });
});

it("shows the sentence of a lost writer after a new load, sends no takeover, and takes the writer back at a click",
  async () => {
    const server = storedSession({ kind: "read_only", observed_writer_generation: "2",
      reason: "superseded_by_takeover" }, { serves: TAKEOVER_STEPS });
    const page = await loadPage(server);
    await expect.poll(page.shown).toEqual({ title: "Chapter A", footer: "只读", editable: false,
      reason: LOST_SENTENCE, button: "接管写作" });
    expect(server.log.takeovers).toEqual([]);

    page.takeOver()!.click();

    await expect.poll(page.shown).toEqual({ title: "Chapter A", footer: "已保存", editable: true, reason: undefined,
      button: undefined });
    expect({ takeovers: server.log.takeovers, unexpected: server.log.unexpected })
      .toEqual({ takeovers: ["2"], unexpected: [] });
  });

it("shows the sentence of a lost writer after an Author Edit of this tab is refused, and keeps the typed text",
  async () => {
    const server = fakeServer({ writer: "this_tab", serves: [...OPEN, SESSION_CREATE, CHALLENGE, AUTHOR_EDITS,
      OUTCOME, ...TAKEOVER_STEPS] });
    const page = await loadPage(server);
    await expect.poll(page.shown).toEqual({ title: "Chapter A", footer: "已保存", editable: true, reason: undefined,
      button: undefined });
    server.takeOverElsewhere();
    const editor = manuscriptEditor(page.root, window);
    editor.focus();
    focusManuscriptEnd(editor, window);

    await applyTrustedInput({ operation: "insert_text", text: "!" });

    await expect.poll(page.shown).toEqual({ title: "Chapter A", footer: "只读", editable: false,
      reason: LOST_SENTENCE, button: "接管写作" });
    expect(manuscriptBody(editor)).toBe("Base!");
    // The refusal gives no writer generation, so the button reads the Editor Session first.
    page.takeOver()!.click();
    await expect.poll(page.shown).toEqual({ title: "Chapter A", footer: "已保存", editable: true, reason: undefined,
      button: undefined });
    expect({ takeovers: server.log.takeovers, unexpected: server.log.unexpected })
      .toEqual({ takeovers: ["2"], unexpected: [] });
  });

it.each([
  { result: "compare_failed" as const, takeovers: ["1"] },
  // A lost response gets the one retry of the same command (`owned-command.ts`).
  { result: "lost" as const, takeovers: ["1", "1"] },
])("sends the automatic takeover one time when its result is $result, and shows the read-only screen",
  async ({ result, takeovers }) => {
    const server = fakeServer({ takeover: result, serves: [...OPEN, SESSION_CREATE, CHALLENGE, TAKEOVER] });
    const page = await loadPage(server);

    await expect.poll(page.shown).toEqual({ title: "Chapter A", footer: "只读", editable: false,
      reason: SECONDARY_SENTENCE, button: "接管写作" });
    expect({ alert: page.root.querySelector("[role=alert]")?.textContent, inFlight: server.inFlight(),
      takeovers: server.log.takeovers, unexpected: server.log.unexpected })
      .toEqual({ alert: "无法接管写作。", inFlight: [], takeovers, unexpected: [] });
  });

it("sends no takeover when a reopen of the same open finds that another tab holds the writer", async () => {
  const server = fakeServer({ writer: "this_tab", serves: [...OPEN, SESSION_CREATE, CHALLENGE, SWITCH, SESSION_READ,
    chapterRead(CHAPTER_B), openProposals(CHAPTER_B)] });
  const page = await loadPage(server);
  await expect.poll(page.shown).toEqual({ title: "Chapter A", footer: "已保存", editable: true, reason: undefined,
    button: undefined });
  server.giveWriterElsewhere();

  page.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();

  await expect.poll(page.shown).toEqual({ title: "Chapter B", footer: "只读", editable: false,
    reason: SECONDARY_SENTENCE, button: "接管写作" });
  expect({ inFlight: server.inFlight(), takeovers: server.log.takeovers, unexpected: server.log.unexpected })
    .toEqual({ inFlight: [], takeovers: [], unexpected: [] });
});

it("keeps the shown Chapter after a takeover from the button, and stores it as the Current Chapter", async () => {
  const server = storedSession({ kind: "read_only", observed_writer_generation: "2",
    reason: "superseded_by_takeover" }, { serves: [...TAKEOVER_STEPS, chapterRead(CHAPTER_B), openProposals(CHAPTER_B),
    SWITCH, snapshotRead(SNAPSHOTS[3]!)] });
  const page = await loadPage(server);
  await expect.poll(() => page.shown().button).toBe("接管写作");
  page.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();
  await expect.poll(page.shown).toEqual({ title: "Chapter B", footer: "只读", editable: false,
    reason: LOST_SENTENCE, button: "接管写作" });
  // The observer records each title of the editor part from the click.
  const titles = new Set<string>();
  const observer = new MutationObserver(() => {
    titles.add(page.root.querySelector(".editor-panel h2")?.textContent ?? "");
  });
  observer.observe(page.root, { subtree: true, childList: true, characterData: true });

  page.takeOver()!.click();

  await expect.poll(() => ({ ...page.shown(), current: server.currentChapter(), inFlight: server.inFlight() }))
    .toEqual({ title: "Chapter B", footer: "已保存", editable: true, reason: undefined, button: undefined,
      current: CHAPTER_B, inFlight: [] });
  observer.disconnect();
  expect([...titles]).toEqual(["Chapter B"]);
  const reloaded = await loadPage(server);
  await expect.poll(reloaded.shown).toEqual({ title: "Chapter B", footer: "已保存", editable: true,
    reason: undefined, button: undefined });
  expect({ takeovers: server.log.takeovers, unexpected: server.log.unexpected })
    .toEqual({ takeovers: ["2"], unexpected: [] });
});
