// ADR 0107, section 2: a tab that comes to the front shows the later Revision of another tab, with its own input on it.
import { afterEach, expect, it, vi } from "vitest";

import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import setCurrentChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-current-chapter.json";
import openProposalsFixture
  from "../../../../generated/golden-wire/storyos-public-release-1/list-chapter-open-proposals.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import statisticsFixture
  from "../../../../generated/golden-wire/storyos-public-release-1/get-statistics.json";
import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import type { ChallengeAdmissionTimers } from "../../src/challenge-admission-wait.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyImeComposition, applyTrustedInput } from "../support/browser-command-client.ts";
import { focusManuscriptEnd, manuscriptBody, manuscriptEditor, manuscriptIsEditable }
  from "../support/manuscript-surface.ts";
import {
  CHAPTER, OWNER, PROJECT, REVISION, SESSION, closeTrackedDatabases, createAppliedAuthorEditResponse,
  createBrowserScenario, deleteJournal, emptyActivityStream, jsonResponse, requestHeaders, requireRequestBody,
  trackDatabase,
} from "./scenario.ts";

const LATER_REVISION = "018f0000-0000-7001-8000-000000000a61";
const NEW_SESSION = "018f0000-0000-7001-8000-000000000a62";
const [FIRST, SECOND] = ["018f0000-0000-7001-8000-000000000a63", "018f0000-0000-7001-8000-000000000a64"];
const CHAPTER_B = "018f0000-0000-7001-8000-000000000a65";
const CHAPTER_C = "018f0000-0000-7001-8000-000000000a6b";
const ACTIVE_KEY = `active_session:${OWNER}:${PROJECT}`;
const AUTHOR_EDIT = `POST /api/v1/projects/${PROJECT}/manuscript/author-edits`;
const CHAPTER_READ = `GET /api/v1/projects/${PROJECT}/chapters/${CHAPTER}`;
const PROJECT_READ = `GET /api/v1/projects/${PROJECT}`;
const CHAPTER_B_READ = `GET /api/v1/projects/${PROJECT}/chapters/${CHAPTER_B}`;
const CREATION = `POST /api/v1/projects/${PROJECT}/editor-sessions`;
const CHALLENGE = `POST /api/v1/projects/${PROJECT}/anti-forgery-challenges`;
const scenario = createBrowserScenario();

const APPLIED_REVISION = "018f0000-0000-7001-8000-000000000a66";
type Revision = ReturnType<typeof revision>;
const revision = (revisionId: string, first: string, second = "渡口。") => ({ revision_id: revisionId,
  body: `${first}\n${second}`, blocks: [{ manuscript_block_id: FIRST, block_kind: "paragraph" as const, text: first },
    { manuscript_block_id: SECOND, block_kind: "paragraph" as const, text: second }] });
const BASE = revision(REVISION, "雨夜。");
const LATER = revision(LATER_REVISION, "雨夜，风起。");
const LATEST = revision("018f0000-0000-7001-8000-000000000a6a", "雨夜，风停。");
const REVISION_B = "018f0000-0000-7001-8000-000000000a68";
const BLOCK_B = "018f0000-0000-7001-8000-000000000a69";
const IN_B: Revision = { revision_id: REVISION_B, body: "乙章。",
  blocks: [{ manuscript_block_id: BLOCK_B, block_kind: "paragraph", text: "乙章。" }] };

afterEach(async () => {
  document.body.replaceChildren();
  sessionStorage.removeItem(ACTIVE_KEY);
  await deleteJournal(scenario.journalName);
});

async function sha256(text: string): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/** A gate that holds a response until the test opens it. */
function gate() {
  let open = () => {};
  const opened = new Promise<void>((resolve) => { open = resolve; });
  return { opened, open: () => { open(); } };
}

/**
 * A fake Server of one Project. The Chapter starts at the base Revision, and `writeInOtherTab` gives it a later
 * Revision of another tab. An Author Edit on an earlier Revision gets `stale_authoritative_head`. With `applies`, an
 * Author Edit on the current Revision of this tab (the base, or a Revision of `applies`) applies and gives the next
 * Revision of `applies`; else it gets no response, so the test sees that it went out. A Command Challenge waits for
 * `gates.challenge`, when the test gives it. With `readHeadAtRequest`, a
 * Chapter read gives the head at its request, before its gate. The first creation gives the Editor Session on the base, the second one a new Editor Session on the
 * current Revision. A Chapter read waits for `gates.chapter`, a Project read for `gates.project`, an Author Edit on
 * the base for `gates.edit`, and a creation after the first for `gates.creation`, when the test gives them.
 * Each Set Current Chapter first runs the next action of `beforeSwitch`, and each Project read the next action of
 * `beforeProjectRead`. It records each request and
 * the body of each Author Edit, and it records and throws on each request that it does not serve.
 */
function fakeServer(applies?: Revision | Revision[]) {
  const own = applies === undefined ? [] : Array.isArray(applies) ? [...applies] : [applies];
  const chain = new Set([REVISION, ...own.map((applied) => applied.revision_id)]);
  const requests: string[] = [];
  const unexpected: string[] = [];
  const switches: unknown[] = [];
  const edits: Record<string, unknown>[] = [];
  const hooks: { beforeSwitch: (() => void)[]; beforeProjectRead: (() => void)[]; readHeadAtRequest?: boolean } =
    { beforeSwitch: [], beforeProjectRead: [] };
  const gates: { chapter?: Promise<void>; project?: Promise<void>; edit?: Promise<void>; creation?: Promise<void>;
    challenge?: Promise<void> } = {};
  // The head of the Chapter and the base of the stored Editor Session, with their activity positions.
  const world = { head: BASE, position: 0, stored: BASE, storedPosition: 0, current: CHAPTER };
  // One Editor Session response for each Editor Session and base, so a read gives the response of the creation. Each
  // move of a base gives a new Snapshot, also on the same Revision, as the Server does.
  const sessions = new Map<string, Promise<unknown>>();
  const session = (editorSessionId: string, at: Revision, position: number, chapterId = CHAPTER) => {
    const key = `${editorSessionId}:${at.revision_id}:${position}`;
    if (!sessions.has(key)) {
      sessions.set(key, sha256(at.body).then((digest) => ({ ...scenario.session,
        editor_session: { ...scenario.session.editor_session, editor_session_id: editorSessionId },
        base_snapshot: { ...scenario.session.base_snapshot, snapshot_id: crypto.randomUUID(), chapter_id: chapterId,
          target_refs: [`manuscript:${chapterId}`],
          project_activity_position: String(position), authoritative_head_revision_id: at.revision_id,
          materialized_revision: at, materialized_payload_digest: {
            ...scenario.session.base_snapshot.materialized_payload_digest, value_hex_lowercase: digest } } })));
    }
    return sessions.get(key)! as Promise<typeof scenario.session>;
  };
  let creations = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const route = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    requests.push(route);
    const project = `/api/v1/projects/${PROJECT}`;
    if (route === "GET /api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (route === "GET /api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: { owner_user_id: OWNER, project_id: PROJECT }, title: "Project A", revision: "1",
          open: { kind: "current_chapter", current_chapter_id: world.current } }] });
    }
    if (route === PROJECT_READ) {
      await gates.project;
      hooks.beforeProjectRead.shift()?.();
      return jsonResponse({ ...scenario.project, project: { ...scenario.project.project,
        open: { kind: "current_chapter", current_chapter_id: world.current } } });
    }
    if (route === `GET ${project}/chapters/${CHAPTER_B}`) {
      return jsonResponse({ ...scenario.chapter, project_activity_position: String(world.position),
        chapter: { ...scenario.chapter.chapter, chapter_id: CHAPTER_B, title: "Chapter B", current_revision: IN_B } });
    }
    if (route === `PUT ${project}/current-chapter`) {
      const input = requireRequestBody(init).set_current_chapter_input as Record<string, unknown>;
      switches.push({ chapter_id: input.chapter_id, expected_current_chapter_id: input.expected_current_chapter_id,
        expected_target_revision_id: input.expected_target_revision_id, editor_session_id: input.editor_session_id });
      hooks.beforeSwitch.shift()?.();
      const reply = (effect: Record<string, unknown>) => jsonResponse({ ...setCurrentChapterFixture,
        project_scope: scenario.project.project_scope, effect, receipt: { ...setCurrentChapterFixture.receipt,
          result: effect.kind }, project: { ...setCurrentChapterFixture.project, project_id: PROJECT,
          open: { kind: "current_chapter", current_chapter_id: world.current } } });
      // The Server checks the Current Chapter, then the head of the target (`set_current_chapter.rs`).
      const target = input.chapter_id === CHAPTER ? world.head : IN_B;
      if (input.expected_current_chapter_id !== world.current) {
        return reply({ kind: "conflicted", reason: "stale_current_chapter" });
      }
      if (input.expected_target_revision_id !== target.revision_id) {
        return reply({ kind: "conflicted", reason: "wrong_target_head" });
      }
      world.current = input.chapter_id as string;
      world.position += 1;
      // The Server moves the base of the named Editor Session to the head of the Chapter (`chapter_selection.rs`), and
      // the effect names that base, as the read of the Editor Session does.
      if (input.editor_session_id !== SESSION || world.current !== CHAPTER) {
        throw new Error("the fake moves only the base of the stored Editor Session to Chapter A");
      }
      world.stored = world.head;
      world.storedPosition = world.position;
      const installed = (await session(SESSION, world.stored, world.storedPosition)).base_snapshot;
      return reply({ ...setCurrentChapterFixture.effect, current_chapter_id: world.current,
        base_snapshot_id: installed.snapshot_id, project_activity_position: installed.project_activity_position });
    }
    if (route === CHAPTER_READ) {
      const atRequest = hooks.readHeadAtRequest === true ? { head: world.head, position: world.position } : undefined;
      await gates.chapter;
      const { head, position } = atRequest ?? world;
      return jsonResponse({ ...scenario.chapter, project_activity_position: String(position),
        chapter: { ...scenario.chapter.chapter, current_revision: head } });
    }
    if (route === CHALLENGE) {
      await gates.challenge;
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (route === CREATION) {
      creations += 1;
      if (creations > 1) await gates.creation;
      return jsonResponse(creations === 1 ? await session(SESSION, BASE, 0)
        : world.current === CHAPTER_B ? await session(NEW_SESSION, IN_B, world.position, CHAPTER_B)
          : await session(NEW_SESSION, world.head, world.position));
    }
    if (route === `GET ${project}/editor-sessions/${SESSION}`) {
      return jsonResponse({ ...await session(SESSION, world.stored, world.storedPosition),
        schema_id: "storyos.query.editor-session.response.v2" });
    }
    if (route === `GET ${project}/activity`) return emptyActivityStream();
    // The page also reads the statistics, the open Proposals, the assistance settings, the Skills, and the tree.
    const scope = scenario.project.project_scope;
    if (route === `GET ${project}/manuscript/statistics`) {
      const position = String(world.position);
      const count = { word_count: "1", character_count: "1" };
      const manuscript = { chapter_count: "1", ...count };
      return jsonResponse({ ...statisticsFixture, project_scope: scope, manuscript,
        page_bytes: String(JSON.stringify(manuscript).length), current_chapter: { chapter_id: CHAPTER, ...count },
        projection_generation: position, projection_watermark: position,
        source_snapshot: { ...statisticsFixture.source_snapshot, project_scope: scope,
          project_activity_position: position } });
    }
    for (const chapterId of [CHAPTER, CHAPTER_B]) {
      if (route === `GET ${project}/chapters/${chapterId}/open-proposals`) {
        return jsonResponse({ ...openProposalsFixture, project_scope: scope, chapter_id: chapterId, proposals: [] });
      }
    }
    if (route === `GET ${project}/assistance`) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (route === `GET ${project}/skills`) return jsonResponse({ ...skillsFixture, project_scope: scope, skills: [] });
    if (route === `GET ${project}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope, tree_revision: "1",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scope },
        volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000815", title: "Volume A", order: "1",
          chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" },
            { chapter_id: CHAPTER_B, title: "Chapter B", order: "2" }] }] });
    }
    if (route === AUTHOR_EDIT) {
      const request = requireRequestBody(init);
      edits.push(request);
      const expected = request.expected_authoritative_revision_id;
      const next = chain.has(expected as string) ? own[0] : undefined;
      if (expected === world.head.revision_id && next === undefined) return new Promise<Response>(() => {});
      const applied = next !== undefined && expected === world.head.revision_id;
      if (applied) {
        // The Server applies the edit before it answers, so a read gives the applied Revision.
        own.shift();
        world.head = next;
        world.position += 1;
        world.stored = next;
        world.storedPosition = world.position;
      }
      // The answer has the state at the time when the Server classified the edit.
      const [position, head] = [world.position, world.head.revision_id];
      await gates.edit;
      const answer = createAppliedAuthorEditResponse({ request, commandDigest: await digestApplyAuthorEdit(request as never),
        idempotencyKey: requestHeaders(init).get("idempotency-key")!, priorRevisionId: expected as string,
        ...next === undefined ? {} : { authoritativeRevisionId: next.revision_id },
        projectActivityPosition: String(position) });
      if (applied) return jsonResponse({ ...answer, effect: { ...answer.effect, authoritative_revision: next } });
      return jsonResponse({ ...answer, effect: { kind: "conflicted", reason: "stale_authoritative_head",
        current_authoritative_revision_id: head }, receipt: { ...answer.receipt, result: "conflicted",
        prior_heads: [head], resulting_heads: [head], proposal_revision_ids: [], authoritative_revision_ids: [],
        authoritative_commit_ids: [], author_action_sequence: null } });
    }
    unexpected.push(route);
    throw new Error(`unexpected request: ${route}`);
  };
  const writeInOtherTab = (later = LATER) => {
    world.head = later;
    world.position += 1;
  };
  const setCurrentInOtherTab = (chapterId: string) => {
    world.current = chapterId;
    world.position += 1;
  };
  return { fetchImpl, requests, unexpected, switches, edits, gates, hooks, writeInOtherTab, setCurrentInOtherTab };
}

/** Timers that never fire, so the input of the tab stays unsent until the tab comes to the front. */
const heldTimers: ChallengeAdmissionTimers = { setTimeoutImpl: () => 0, clearTimeoutImpl: () => {} };

/** Timers that fire only when the test calls `fire`, so the test starts the save of the input. */
function manualTimers() {
  const due = new Map<number, () => void>();
  let next = 0;
  const timers: ChallengeAdmissionTimers = {
    setTimeoutImpl: (callback) => { next += 1; due.set(next, callback); return next; },
    clearTimeoutImpl: (timer) => { due.delete(timer as number); },
  };
  const fire = () => {
    const callbacks = [...due.values()];
    due.clear();
    for (const callback of callbacks) callback();
  };
  return { timers, fire };
}

/** Records each Journal connection that the page opens, so the test can close them as a page close does. */
function trackOpens() {
  const databases = new Set<IDBDatabase>();
  const open = IDBFactory.prototype.open;
  const openSpy = vi.spyOn(indexedDB, "open").mockImplementation((name, version) => {
    const request = open.call(indexedDB, name, version);
    request.addEventListener("success", () => trackDatabase(request.result, databases));
    return request;
  });
  return { databases, openSpy };
}

const load = (server: ReturnType<typeof fakeServer>) => {
  document.body.innerHTML = '<main id="app"></main>';
  return loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
};

/** Starts a load with the stored Editor Session, and closes the page when `stopped` is true. */
async function closeDuringLoad(server: ReturnType<typeof fakeServer>, stopped: () => boolean) {
  const { databases, openSpy } = trackOpens();
  try {
    void load(server).catch(() => undefined);
    await expect.poll(stopped).toBe(true);
  } finally {
    closeTrackedDatabases(databases);
    openSpy.mockRestore();
  }
}

/**
 * Mounts the view on the base Revision, runs `act`, and unmounts the view. A reopen opens the Journal again in the view,
 * so the cleanup closes each connection, and the deletion of the Journal is not blocked.
 */
async function withView(server: ReturnType<typeof fakeServer>, act: (root: HTMLElement) => Promise<void>,
  writingTimers = heldTimers, reload = false) {
  if (!reload) sessionStorage.removeItem(ACTIVE_KEY);
  const { databases, openSpy } = trackOpens();
  try {
    const loaded = await load(server);
    const view = mountStage1View(loaded.root, { ...loaded, writingTimers });
    try {
      await expect.poll(() => loaded.root.querySelector("[contenteditable='true']") !== null).toBe(true);
      await act(loaded.root);
    } finally {
      view.unmount();
    }
  } finally {
    closeTrackedDatabases(databases);
    openSpy.mockRestore();
  }
}

const type = async (root: HTMLElement, text: string) => {
  focusManuscriptEnd(manuscriptEditor(root, window), window);
  await applyTrustedInput({ operation: "insert_text", text });
};

const unsettled = (root: HTMLElement) =>
  root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count");
const count = (server: ReturnType<typeof fakeServer>, route: string) =>
  server.requests.filter((request) => request === route).length;

const shown = (root: HTMLElement) => {
  const editor = manuscriptEditor(root, window);
  const status = root.querySelector("[data-save-state]");
  return { text: manuscriptBody(editor), editable: manuscriptIsEditable(editor),
    save: status?.getAttribute("data-save-state"), failure: status?.getAttribute("data-editor-failure"),
    stored: sessionStorage.getItem(ACTIVE_KEY) };
};

it("reads the Chapter one time and shows the later Revision when the tab with no input comes to the front", async () => {
  const server = fakeServer();
  await withView(server, async (root) => {
    server.writeInOtherTab();
    server.requests.length = 0;

    document.dispatchEvent(new Event("visibilitychange"));

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。", editable: true,
      save: "saved", failure: "", stored: NEW_SESSION });
    expect({ chapterReads: server.requests.filter((request) => request === CHAPTER_READ).length,
      edits: server.requests.filter((request) => request === AUTHOR_EDIT).length, unexpected: server.unexpected })
      .toEqual({ chapterReads: 1, edits: 0, unexpected: [] });
  });
});

it("keeps its Chapter and shows the later Revision when the tab with no input comes to the front after another tab set "
  + "another Chapter current", async () => {
  const server = fakeServer();
  await withView(server, async (root) => {
    server.writeInOtherTab();
    server.setCurrentInOtherTab(CHAPTER_B);
    server.requests.length = 0;

    document.dispatchEvent(new Event("visibilitychange"));

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。", editable: true,
      save: "saved", failure: "", stored: NEW_SESSION });
    await type(root, "无人");
    await expect.poll(() => shown(root).text).toBe("雨夜，风起。\n渡口。无人");
    expect({ chapterReads: count(server, CHAPTER_READ) + count(server, CHAPTER_B_READ), switches: server.switches,
      unexpected: server.unexpected }).toEqual({ chapterReads: 1, switches: [{ chapter_id: CHAPTER,
      expected_current_chapter_id: CHAPTER_B, expected_target_revision_id: LATER_REVISION, editor_session_id: SESSION }],
      unexpected: [] });
  });
});

it("reads again and sets its Chapter current when other tabs save it and set another Chapter current after each read",
  async () => {
    const server = fakeServer();
    await withView(server, async (root) => {
      server.writeInOtherTab();
      server.setCurrentInOtherTab(CHAPTER_B);
      server.hooks.beforeSwitch.push(() => { server.writeInOtherTab(LATEST); },
        () => { server.setCurrentInOtherTab(CHAPTER_C); });
      server.requests.length = 0;

      document.dispatchEvent(new Event("visibilitychange"));

      await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风停。\n渡口。", editable: true,
        save: "saved", failure: "", stored: NEW_SESSION });
      const switched = { chapter_id: CHAPTER, expected_current_chapter_id: CHAPTER_B, editor_session_id: SESSION };
      // The first command gets `wrong_target_head`, the second one `stale_current_chapter`, and the third one applies.
      expect({ chapterReads: count(server, CHAPTER_READ), otherReads: count(server, CHAPTER_B_READ),
        switches: server.switches, unexpected: server.unexpected }).toEqual({ chapterReads: 3, otherReads: 0,
        switches: [{ ...switched, expected_target_revision_id: LATER_REVISION },
          { ...switched, expected_target_revision_id: LATEST.revision_id },
          { ...switched, expected_current_chapter_id: CHAPTER_C, expected_target_revision_id: LATEST.revision_id }],
        unexpected: [] });
    });
  });

it("keeps its Chapter, reads no other Chapter, and sends nothing more when each Set Current Chapter of the front "
  + "conflicts, and tries again at the next return to the front", async () => {
  const server = fakeServer();
  await withView(server, async (root) => {
    server.writeInOtherTab();
    server.setCurrentInOtherTab(CHAPTER_B);
    // Before each of 8 commands, another tab saves Chapter A again, so each command gets `wrong_target_head`.
    for (let index = 0; index < 8; index += 1) {
      server.hooks.beforeSwitch.push(() => { server.writeInOtherTab(index % 2 === 0 ? LATEST : LATER); });
    }
    server.requests.length = 0;
    document.dispatchEvent(new Event("visibilitychange"));
    await expect.poll(() => server.switches.length, { timeout: 10_000 }).toBe(8);
    const atLimit = server.requests.length;

    // A return to the front starts a new check only after the reopen ended, so the ninth command proves that end.
    await expect.poll(() => {
      document.dispatchEvent(new Event("visibilitychange"));
      return server.switches.length;
    }, { timeout: 10_000 }).toBe(9);
    const afterLimit = server.requests.slice(atLimit);

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。", editable: true,
      save: "saved", failure: "", stored: NEW_SESSION });
    expect({ otherReads: count(server, CHAPTER_B_READ), nextRequests: afterLimit.slice(0, 2),
      unexpected: server.unexpected }).toEqual({ otherReads: 0,
      nextRequests: [CHAPTER_READ, PROJECT_READ], unexpected: [] });
  });
});

it("sets its Chapter current again when another tab sets another Chapter current after Set Current Chapter of the "
  + "front applied", async () => {
  const server = fakeServer();
  await withView(server, async (root) => {
    server.writeInOtherTab();
    server.setCurrentInOtherTab(CHAPTER_B);
    // The first Project read is of the reopen. The second one comes after the command applied.
    server.hooks.beforeProjectRead.push(() => {}, () => { server.setCurrentInOtherTab(CHAPTER_C); });
    server.requests.length = 0;

    document.dispatchEvent(new Event("visibilitychange"));

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。", editable: true,
      save: "saved", failure: "", stored: NEW_SESSION });
    const switched = { chapter_id: CHAPTER, expected_target_revision_id: LATER_REVISION, editor_session_id: SESSION };
    expect({ otherReads: count(server, CHAPTER_B_READ), switches: server.switches, unexpected: server.unexpected })
      .toEqual({ otherReads: 0, switches: [{ ...switched, expected_current_chapter_id: CHAPTER_B },
        { ...switched, expected_current_chapter_id: CHAPTER_C }], unexpected: [] });
  });
});

it("puts the unsent input on the later Revision when the save at the front finds it", async () => {
  const server = fakeServer();
  await withView(server, async (root) => {
    await type(root, "无人");
    await expect.poll(() => root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"))
      .toBe("1");
    server.writeInOtherTab();

    document.dispatchEvent(new Event("visibilitychange"));

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", editable: true,
      save: "saving", failure: "", stored: NEW_SESSION });
    expect({ edits: server.requests.filter((request) => request === AUTHOR_EDIT).length,
      unexpected: server.unexpected }).toEqual({ edits: 1, unexpected: [] });
  });
});

it("accepts no input after the front found the later Revision and before the tab opens again on it", async () => {
  const server = fakeServer();
  const project = gate();
  await withView(server, async (root) => {
    server.writeInOtherTab();
    server.gates.project = project.opened;
    server.requests.length = 0;
    document.dispatchEvent(new Event("visibilitychange"));
    await expect.poll(() => server.requests.includes(PROJECT_READ)).toBe(true);

    // The catch-up started: the editor is read-only until the tab opens again on the later Revision.
    await expect.poll(() => manuscriptIsEditable(manuscriptEditor(root, window))).toBe(false);
    project.open();

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。", editable: true,
      save: "saved", failure: "", stored: NEW_SESSION });
    expect({ edits: server.requests.filter((request) => request === AUTHOR_EDIT).length,
      unexpected: server.unexpected }).toEqual({ edits: 0, unexpected: [] });
  });
});

it("sends nothing and stays writable when the tab comes to the front and the Chapter has no later Revision",
  async () => {
    const server = fakeServer();
    await withView(server, async (root) => {
      server.requests.length = 0;
      document.dispatchEvent(new Event("visibilitychange"));
      await expect.poll(() => count(server, CHAPTER_READ)).toBe(1);
      // A second return to the front reads again only after the first check ended with no catch-up.
      document.dispatchEvent(new Event("visibilitychange"));
      await expect.poll(() => count(server, CHAPTER_READ)).toBe(2);

      await type(root, "无人");
      expect({ ...shown(root), requests: server.requests.filter((request) => request !== AUTHOR_EDIT),
        unexpected: server.unexpected }).toEqual({ text: "雨夜。\n渡口。无人", editable: true, save: "saving",
        failure: "", stored: SESSION, requests: [CHAPTER_READ, CHAPTER_READ], unexpected: [] });
    });
  });

it("checks again after input during the quiet step of the front check, and catches up", async () => {
  const server = fakeServer(revision(APPLIED_REVISION, "雨夜。", "渡口。无"));
  const edit = gate();
  // No idle timer fires, so only the front check sends the input.
  const { timers } = manualTimers();
  await withView(server, async (root) => {
    await type(root, "无");
    await expect.poll(() => unsettled(root)).toBe("1");
    server.gates.edit = edit.opened;
    server.requests.length = 0;
    // The quiet step of the front check sends "无", and "人" comes while the save waits for its response.
    document.dispatchEvent(new Event("visibilitychange"));
    await expect.poll(() => count(server, AUTHOR_EDIT)).toBe(1);
    await type(root, "人");
    server.writeInOtherTab();
    edit.open();

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", editable: true,
      save: "saving", failure: "", stored: NEW_SESSION });
    expect(server.unexpected).toEqual([]);
  }, timers);
});

it("checks again after input during each of five quiet steps of the front check, and catches up", async () => {
  const chars = ["一", "二", "三", "四", "五", "六"];
  const typed = (end: number) => `渡口。${chars.slice(0, end).join("")}`;
  const server = fakeServer(chars.map((_, index) =>
    revision(`018f0000-0000-7001-8000-000000000a7${index}`, "雨夜。", typed(index + 1))));
  const { timers } = manualTimers();
  await withView(server, async (root) => {
    await type(root, chars[0]!);
    await expect.poll(() => unsettled(root)).toBe("1");
    let held = gate();
    server.gates.edit = held.opened;
    server.requests.length = 0;
    document.dispatchEvent(new Event("visibilitychange"));
    // Each quiet step sends the last character, and the next character comes while that save waits for its response.
    for (let index = 1; index < chars.length; index += 1) {
      await expect.poll(() => count(server, AUTHOR_EDIT)).toBe(index);
      const next = gate();
      server.gates.edit = next.opened;
      await type(root, chars[index]!);
      held.open();
      held = next;
    }
    await expect.poll(() => count(server, AUTHOR_EDIT)).toBe(chars.length);
    server.writeInOtherTab(revision(LATER_REVISION, "雨夜，风起。", typed(chars.length)));
    held.open();

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: `雨夜，风起。\n${typed(chars.length)}`,
      editable: true, save: "saved", failure: "", stored: NEW_SESSION });
    expect({ chapterReads: count(server, CHAPTER_READ), unexpected: server.unexpected })
      .toEqual({ chapterReads: 1, unexpected: [] });
  }, timers);
});

it("ends the front check when the tab goes to the background during its quiet step, and checks again at the next "
  + "return to the front", async () => {
  const server = fakeServer([revision(APPLIED_REVISION, "雨夜。", "渡口。无"),
    revision("018f0000-0000-7001-8000-000000000a76", "雨夜。", "渡口。无人"),
    revision("018f0000-0000-7001-8000-000000000a77", "雨夜。", "渡口。无人了")]);
  const edit = gate();
  const { timers, fire } = manualTimers();
  const visibility = vi.spyOn(document, "visibilityState", "get");
  try {
    await withView(server, async (root) => {
      await type(root, "无");
      await expect.poll(() => unsettled(root)).toBe("1");
      server.gates.edit = edit.opened;
      server.requests.length = 0;
      document.dispatchEvent(new Event("visibilitychange"));
      await expect.poll(() => count(server, AUTHOR_EDIT)).toBe(1);
      await type(root, "人");
      visibility.mockReturnValue("hidden");
      edit.open();
      // In the background, the idle timer sends each save, and the front check reads nothing.
      const saved = (edits: number) => {
        fire();
        return count(server, AUTHOR_EDIT) === edits && unsettled(root) === "0";
      };
      await expect.poll(() => saved(2)).toBe(true);
      await type(root, "了");
      await expect.poll(() => saved(3)).toBe(true);
      expect(count(server, CHAPTER_READ)).toBe(0);

      visibility.mockReturnValue("visible");
      server.writeInOtherTab(revision(LATER_REVISION, "雨夜，风起。", "渡口。无人了"));
      document.dispatchEvent(new Event("visibilitychange"));

      await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人了",
        editable: true, save: "saved", failure: "", stored: NEW_SESSION });
      expect({ chapterReads: count(server, CHAPTER_READ), unexpected: server.unexpected })
        .toEqual({ chapterReads: 1, unexpected: [] });
    }, timers);
  } finally {
    visibility.mockRestore();
  }
});

it("reads the Chapter when an IME composition that is open when the tab comes to the front ends", async () => {
  const server = fakeServer();
  await withView(server, async (root) => {
    focusManuscriptEnd(manuscriptEditor(root, window), window);
    // The offsets are positions of the editor, and 8 is the end of the second block.
    await applyImeComposition({ text: "zhong", replacementStart: 8, replacementEnd: 8, selectionStart: 5,
      selectionEnd: 5 });
    await expect.poll(() => manuscriptBody(manuscriptEditor(root, window))).toBe("雨夜。\n渡口。zhong");
    server.writeInOtherTab();
    server.requests.length = 0;
    document.dispatchEvent(new Event("visibilitychange"));

    await applyImeComposition({ operation: "cancel" });

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。", editable: true,
      save: "saved", failure: "", stored: NEW_SESSION });
    expect({ chapterReads: count(server, CHAPTER_READ), edits: count(server, AUTHOR_EDIT),
      unexpected: server.unexpected }).toEqual({ chapterReads: 1, edits: 0, unexpected: [] });
  });
});

/**
 * Starts a save of "无" while the front check reads the Chapter, and types "人" while the save waits for its response.
 * `settle` then gives the response of the save and the response of the Chapter read in one of the two orders.
 */
async function saveDuringFrontRead(settle: (root: HTMLElement, open: { edit: () => void; chapter: () => void })
  => Promise<void>, applies?: Revision, end = { text: "雨夜，风起。\n渡口。无人", stored: NEW_SESSION }) {
  const server = fakeServer(applies);
  const [chapter, edit] = [gate(), gate()];
  const { timers, fire } = manualTimers();
  await withView(server, async (root) => {
    if (applies === undefined) server.writeInOtherTab();
    server.gates.chapter = chapter.opened;
    server.gates.edit = edit.opened;
    server.requests.length = 0;
    document.dispatchEvent(new Event("visibilitychange"));
    await expect.poll(() => server.requests.includes(CHAPTER_READ)).toBe(true);
    await type(root, "无");
    await expect.poll(() => unsettled(root)).toBe("1");
    fire();
    await expect.poll(() => server.requests.includes(AUTHOR_EDIT)).toBe(true);
    // The keystroke waits in the queue behind the save, and the editor shows it.
    await type(root, "人");
    await expect.poll(() => manuscriptBody(manuscriptEditor(root, window))).toBe("雨夜。\n渡口。无人");

    await settle(root, { edit: edit.open, chapter: chapter.open });

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ ...end, editable: true, save: "saving",
      failure: "" });
    expect({ chapterReads: count(server, CHAPTER_READ), edits: count(server, AUTHOR_EDIT),
      unexpected: server.unexpected }).toEqual({ chapterReads: 1, edits: 1, unexpected: [] });
  }, timers);
}

it("reads the Chapter one time when a save during the front read finds the later Revision before the read ends",
  async () => {
    await saveDuringFrontRead(async (root, open) => {
      open.edit();
      // The catch-up starts: the editor is read-only while the front read waits for its response.
      await expect.poll(() => manuscriptIsEditable(manuscriptEditor(root, window))).toBe(false);
      open.chapter();
    });
  });

it("reads the Chapter one time when the front read ends before a save during it finds the later Revision",
  async () => {
    await saveDuringFrontRead(async (root, open) => {
      open.chapter();
      // The catch-up starts: the editor is read-only while the save waits for its response.
      await expect.poll(() => manuscriptIsEditable(manuscriptEditor(root, window))).toBe(false);
      open.edit();
    });
  });

// The save applies, so the front read finds the Revision of that save, and the keystroke enters the Journal.
it("keeps a keystroke behind a save that applies when the front read finds the Revision of that save first",
  async () => {
    await saveDuringFrontRead(async (root, open) => {
      open.chapter();
      await expect.poll(() => manuscriptIsEditable(manuscriptEditor(root, window))).toBe(false);
      open.edit();
    }, revision(APPLIED_REVISION, "雨夜。", "渡口。无"), { text: "雨夜。\n渡口。无人", stored: SESSION });
  });

// The response of the save comes first, so the front read finds the Revision of the editor and starts no catch-up.
it("keeps a keystroke behind a save that applies when the save answers before the front read", async () => {
  const server = fakeServer(revision(APPLIED_REVISION, "雨夜。", "渡口。无"));
  const [chapter, edit] = [gate(), gate()];
  const { timers, fire } = manualTimers();
  await withView(server, async (root) => {
    server.gates.chapter = chapter.opened;
    server.gates.edit = edit.opened;
    server.requests.length = 0;
    document.dispatchEvent(new Event("visibilitychange"));
    await expect.poll(() => server.requests.includes(CHAPTER_READ)).toBe(true);
    await type(root, "无");
    await expect.poll(() => unsettled(root)).toBe("1");
    fire();
    await expect.poll(() => server.requests.includes(AUTHOR_EDIT)).toBe(true);
    await type(root, "人");

    edit.open();
    await expect.poll(() => root.querySelector("[data-save-state]")?.getAttribute("data-authoritative-revision-id"))
      .toBe(APPLIED_REVISION);
    chapter.open();
    // A later front check saves the keystroke only when the first check ended with no catch-up.
    await expect.poll(() => {
      document.dispatchEvent(new Event("visibilitychange"));
      return count(server, AUTHOR_EDIT);
    }).toBe(2);

    expect({ ...shown(root), chapterReads: count(server, CHAPTER_READ), reopens: count(server, PROJECT_READ),
      unexpected: server.unexpected }).toEqual({ text: "雨夜。\n渡口。无人", editable: true, save: "saving",
      failure: "", stored: SESSION, chapterReads: 1, reopens: 0, unexpected: [] });
  }, timers);
});

it("starts no catch-up when the front read gives a Revision earlier than the Revision that a save installed during "
  + "the read", async () => {
  const server = fakeServer(revision(APPLIED_REVISION, "雨夜。", "渡口。无"));
  const chapter = gate();
  const { timers, fire } = manualTimers();
  await withView(server, async (root) => {
    // The read takes the base Revision at its request, and its response comes after the save installed a later one.
    server.hooks.readHeadAtRequest = true;
    server.gates.chapter = chapter.opened;
    server.requests.length = 0;
    document.dispatchEvent(new Event("visibilitychange"));
    await expect.poll(() => server.requests.includes(CHAPTER_READ)).toBe(true);
    await type(root, "无");
    await expect.poll(() => unsettled(root)).toBe("1");
    fire();
    await expect.poll(() => root.querySelector("[data-save-state]")?.getAttribute("data-authoritative-revision-id"))
      .toBe(APPLIED_REVISION);

    chapter.open();
    // A later front check reads again only when the first check ended with no catch-up.
    await expect.poll(() => {
      document.dispatchEvent(new Event("visibilitychange"));
      return count(server, CHAPTER_READ);
    }).toBe(2);

    expect({ ...shown(root), reopens: count(server, PROJECT_READ), creations: count(server, CREATION),
      unexpected: server.unexpected }).toEqual({ text: "雨夜。\n渡口。无", editable: true, save: "saved", failure: "",
      stored: SESSION, reopens: 0, creations: 0, unexpected: [] });
  }, timers);
});

/** Types "无人" in Chapter A and leaves it unsent, then another tab saves Chapter A and sets Chapter B current. */
async function leaveTextAndSetB(server: ReturnType<typeof fakeServer>, text: string) {
  await withView(server, async (root) => {
    if (text === "") return;
    await type(root, text);
    await expect.poll(() => unsettled(root)).toBe("1");
  });
  server.writeInOtherTab();
  server.setCurrentInOtherTab(CHAPTER_B);
  server.requests.length = 0;
}

it("sets its Chapter current again at a load and keeps its unsent text when another tab set another Chapter current",
  async () => {
    const server = fakeServer();
    await leaveTextAndSetB(server, "无人");

    await withView(server, async (root) => {
      await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", editable: true,
        save: "saving", failure: "", stored: NEW_SESSION });
      expect({ switches: server.switches, unexpected: server.unexpected }).toEqual({ switches: [{ chapter_id: CHAPTER,
        expected_current_chapter_id: CHAPTER_B, expected_target_revision_id: LATER_REVISION,
        editor_session_id: SESSION }], unexpected: [] });
    }, heldTimers, true);
  });

it("continues the catch-up with the unsent text at the next load when the page closed after Set Current Chapter",
  async () => {
    const server = fakeServer();
    await leaveTextAndSetB(server, "无人");
    // The page closes after Set Current Chapter moved the base of the stored Editor Session and before the copy.
    server.gates.creation = new Promise(() => {});
    await closeDuringLoad(server, () => count(server, CREATION) === 1);
    expect(server.switches).toHaveLength(1);
    delete server.gates.creation;
    server.requests.length = 0;

    const { timers, fire } = manualTimers();
    await withView(server, async (root) => {
      await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", editable: true,
        save: "saving", failure: "", stored: NEW_SESSION });
      // The save of the text goes out on the later Revision, and the fake gives it no response.
      fire();
      await expect.poll(() => count(server, AUTHOR_EDIT)).toBe(1);
      expect({ expected: server.edits[0]!.expected_authoritative_revision_id,
        text: JSON.stringify(server.edits[0]).includes("无人") }).toEqual({ expected: LATER_REVISION, text: true });
      expect({ switches: server.switches.length, unexpected: server.unexpected }).toEqual({ switches: 1, unexpected: [] });
    }, timers, true);
  });

it("reads only its Chapter when a save finds that another tab saved it and set another Chapter current", async () => {
  const server = fakeServer();
  const { timers, fire } = manualTimers();
  await withView(server, async (root) => {
    await type(root, "无人");
    await expect.poll(() => unsettled(root)).toBe("1");
    server.writeInOtherTab();
    server.setCurrentInOtherTab(CHAPTER_B);
    server.requests.length = 0;

    fire();

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", editable: true,
      save: "saving", failure: "", stored: NEW_SESSION });
    expect({ chapterReads: count(server, CHAPTER_READ), otherReads: count(server, CHAPTER_B_READ),
      switches: server.switches.length, unexpected: server.unexpected }).toEqual({ chapterReads: 1, otherReads: 0,
      switches: 1, unexpected: [] });
  }, timers);
});

it("opens the Chapter that another tab set current at a load when the unsent text of the tab has no change",
  async () => {
    const server = fakeServer();
    await withView(server, async (root) => {
      await type(root, "无");
      await expect.poll(() => unsettled(root)).toBe("1");
      await applyTrustedInput({ operation: "backspace" });
      await expect.poll(() => manuscriptBody(manuscriptEditor(root, window))).toBe("雨夜。\n渡口。");
      await expect.poll(() => unsettled(root)).toBe("2");
    });
    server.setCurrentInOtherTab(CHAPTER_B);
    server.requests.length = 0;

    await withView(server, async (root) => {
      await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "乙章。", editable: true,
        save: "saved", failure: "", stored: NEW_SESSION });
      expect({ switches: server.switches, unexpected: server.unexpected }).toEqual({ switches: [], unexpected: [] });
    }, heldTimers, true);
  });

it("opens the Chapter that another tab set current at a load when the tab has no text to send", async () => {
  const server = fakeServer();
  await leaveTextAndSetB(server, "");

  await withView(server, async (root) => {
    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "乙章。", editable: true, save: "saved",
      failure: "", stored: NEW_SESSION });
    expect({ switches: server.switches, unexpected: server.unexpected }).toEqual({ switches: [], unexpected: [] });
  }, heldTimers, true);
});

/**
 * Types "无人" in Chapter A and starts its save. `held` holds the Command Challenge of the save, or else the save, and
 * the page closes. Then another tab sets Chapter B current.
 */
async function closeDuringSaveAndSetB(server: ReturnType<typeof fakeServer>, held: "challenge" | "edit") {
  const { timers, fire } = manualTimers();
  await withView(server, async (root) => {
    await type(root, "无人");
    await expect.poll(() => unsettled(root)).toBe("1");
    const challenges = count(server, CHALLENGE);
    if (held === "challenge") server.gates.challenge = new Promise(() => {});
    fire();
    await expect.poll(() => held === "challenge" ? count(server, CHALLENGE) : count(server, AUTHOR_EDIT))
      .toBe(held === "challenge" ? challenges + 1 : 1);
  }, timers);
  delete server.gates.challenge;
  server.setCurrentInOtherTab(CHAPTER_B);
  server.requests.length = 0;
}

it("sets its Chapter current again at a load and sends the group that never went to the Server when the page closed "
  + "before the Challenge answered", async () => {
  const server = fakeServer(revision(APPLIED_REVISION, "雨夜。", "渡口。无人"));
  await closeDuringSaveAndSetB(server, "challenge");
  expect(count(server, AUTHOR_EDIT)).toBe(0);

  const { timers, fire } = manualTimers();
  await withView(server, async (root) => {
    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜。\n渡口。无人", editable: true,
      save: "saving", failure: "", stored: SESSION });
    fire();
    await expect.poll(() => root.querySelector("[data-save-state]")?.getAttribute("data-authoritative-revision-id"))
      .toBe(APPLIED_REVISION);
  }, timers, true);
  expect({ edits: server.edits.length, switches: server.switches, unexpected: server.unexpected }).toEqual({ edits: 1,
    switches: [{ chapter_id: CHAPTER, expected_current_chapter_id: CHAPTER_B, expected_target_revision_id: REVISION,
      editor_session_id: SESSION }], unexpected: [] });

  // A new load shows the stored text.
  server.requests.length = 0;
  await withView(server, async (root) => {
    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜。\n渡口。无人", editable: true,
      save: "saved", failure: "", stored: SESSION });
  }, heldTimers, true);
  expect({ edits: server.edits.length, switches: server.switches.length, unexpected: server.unexpected })
    .toEqual({ edits: 1, switches: 1, unexpected: [] });
});

it("puts the text of the group that never went to the Server on the later Revision of its Chapter at a load", async () => {
  const server = fakeServer();
  await closeDuringSaveAndSetB(server, "challenge");
  server.writeInOtherTab();

  const { timers, fire } = manualTimers();
  await withView(server, async (root) => {
    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", editable: true,
      save: "saving", failure: "", stored: NEW_SESSION });
    // The save of the text goes out one time, in the new Editor Session on the later Revision.
    fire();
    await expect.poll(() => count(server, AUTHOR_EDIT)).toBe(1);
  }, timers, true);
  expect({ editors: server.edits.map((edit) => [edit.editor_session_id, edit.expected_authoritative_revision_id]),
    switches: server.switches.length, unexpected: server.unexpected }).toEqual({
    editors: [[NEW_SESSION, LATER_REVISION]], switches: 1, unexpected: [] });
});

it("does not send again a group with an unknown result when the page closed during its save and another tab set "
  + "another Chapter current", async () => {
  const server = fakeServer();
  await closeDuringSaveAndSetB(server, "edit");

  // The load keeps the recovery of the stored Editor Session, so the editor takes no input.
  const { databases, openSpy } = trackOpens();
  try {
    const loaded = await load(server);
    const view = mountStage1View(loaded.root, { ...loaded, writingTimers: heldTimers });
    try {
      await expect.poll(() => shown(loaded.root), { timeout: 10_000 }).toMatchObject({ editable: false,
        save: "needs_attention", stored: SESSION });
    } finally {
      view.unmount();
    }
  } finally {
    closeTrackedDatabases(databases);
    openSpy.mockRestore();
  }
  expect({ edits: server.edits.length, switches: server.switches, unexpected: server.unexpected })
    .toEqual({ edits: 1, switches: [], unexpected: [] });
});
