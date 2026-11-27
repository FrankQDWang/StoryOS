// ADR 0107, section 2: a tab whose save finds the later Revision of another tab catches up on it, with its own input.
import { afterEach, expect, it, vi } from "vitest";

import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
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
import { applyTrustedInput } from "../support/browser-command-client.ts";
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
const ACTIVE_KEY = `active_session:${OWNER}:${PROJECT}`;
const AUTHOR_EDIT = `POST /api/v1/projects/${PROJECT}/manuscript/author-edits`;
const CHAPTER_READ = `GET /api/v1/projects/${PROJECT}/chapters/${CHAPTER}`;
const PROJECT_READ = `GET /api/v1/projects/${PROJECT}`;
const scenario = createBrowserScenario();

type Revision = ReturnType<typeof revision>;
const revision = (revisionId: string, first: string, second = "渡口。") => ({ revision_id: revisionId,
  body: `${first}\n${second}`, blocks: [{ manuscript_block_id: FIRST, block_kind: "paragraph" as const, text: first },
    { manuscript_block_id: SECOND, block_kind: "paragraph" as const, text: second }] });
const BASE = revision(REVISION, "雨夜。");
const LATER = revision(LATER_REVISION, "雨夜，风起。");

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
 * Revision of another tab. An Author Edit on an earlier Revision gets `stale_authoritative_head` when `gates.edit`
 * opens; an Author Edit on the current Revision gets no response, so the test sees that it went out. The first creation
 * gives the Editor Session on the base, the second one a new Editor Session on the current Revision. A Project read
 * waits for `gates.project` when the test gives it. It records each request, and it records and throws on each request
 * that it does not serve.
 */
function fakeServer() {
  const requests: string[] = [];
  const unexpected: string[] = [];
  const gates: { project?: Promise<void>; edit?: Promise<void> } = {};
  const world = { head: BASE, position: 0 };
  // One Editor Session response for each Editor Session and base, so a read gives the response of the creation.
  const sessions = new Map<string, Promise<unknown>>();
  const session = (editorSessionId: string, at: Revision, position: number) => {
    const key = `${editorSessionId}:${at.revision_id}`;
    if (!sessions.has(key)) {
      sessions.set(key, sha256(at.body).then((digest) => ({ ...scenario.session,
        editor_session: { ...scenario.session.editor_session, editor_session_id: editorSessionId },
        base_snapshot: { ...scenario.session.base_snapshot, snapshot_id: crypto.randomUUID(),
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
          open: { kind: "current_chapter", current_chapter_id: CHAPTER } }] });
    }
    if (route === PROJECT_READ) {
      await gates.project;
      return jsonResponse(scenario.project);
    }
    if (route === CHAPTER_READ) {
      return jsonResponse({ ...scenario.chapter, project_activity_position: String(world.position),
        chapter: { ...scenario.chapter.chapter, current_revision: world.head } });
    }
    if (route === `POST ${project}/anti-forgery-challenges`) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (route === `POST ${project}/editor-sessions`) {
      creations += 1;
      return jsonResponse(creations === 1 ? await session(SESSION, BASE, 0)
        : await session(NEW_SESSION, world.head, world.position));
    }
    if (route === `GET ${project}/editor-sessions/${SESSION}`) {
      return jsonResponse({ ...await session(SESSION, BASE, 0), schema_id: "storyos.query.editor-session.response.v2" });
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
    if (route === `GET ${project}/chapters/${CHAPTER}/open-proposals`) {
      return jsonResponse({ ...openProposalsFixture, project_scope: scope, chapter_id: CHAPTER, proposals: [] });
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
      if (request.expected_authoritative_revision_id === world.head.revision_id) return new Promise<Response>(() => {});
      await gates.edit;
      const answer = createAppliedAuthorEditResponse({ request, commandDigest: await digestApplyAuthorEdit(request as never),
        idempotencyKey: requestHeaders(init).get("idempotency-key")!, priorRevisionId: REVISION,
        projectActivityPosition: String(world.position) });
      const head = world.head.revision_id;
      return jsonResponse({ ...answer, effect: { kind: "conflicted", reason: "stale_authoritative_head",
        current_authoritative_revision_id: head }, receipt: { ...answer.receipt, result: "conflicted",
        prior_heads: [head], resulting_heads: [head], proposal_revision_ids: [], authoritative_revision_ids: [],
        authoritative_commit_ids: [], author_action_sequence: null } });
    }
    unexpected.push(route);
    throw new Error(`unexpected request: ${route}`);
  };
  const writeInOtherTab = () => {
    world.head = LATER;
    world.position += 1;
  };
  return { fetchImpl, requests, unexpected, gates, writeInOtherTab };
}

/** Timers that never fire, so the input of the tab stays unsent until the test starts a Chapter switch. */
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

/**
 * Mounts the view on the base Revision, runs `act`, and unmounts the view. A reopen opens the Journal again in the view,
 * so the cleanup closes each connection, and the deletion of the Journal is not blocked.
 */
async function withView(server: ReturnType<typeof fakeServer>, act: (root: HTMLElement) => Promise<void>,
  writingTimers = heldTimers) {
  sessionStorage.removeItem(ACTIVE_KEY);
  const databases = new Set<IDBDatabase>();
  const open = IDBFactory.prototype.open;
  const openSpy = vi.spyOn(indexedDB, "open").mockImplementation((name, version) => {
    const request = open.call(indexedDB, name, version);
    request.addEventListener("success", () => trackDatabase(request.result, databases));
    return request;
  });
  document.body.innerHTML = '<main id="app"></main>';
  try {
    const loaded = await loadStoryOSWebState({ documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
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

it("puts its input and a keystroke behind the save on the later Revision when the save finds it", async () => {
  const server = fakeServer();
  const [edit, project] = [gate(), gate()];
  const { timers, fire } = manualTimers();
  await withView(server, async (root) => {
    server.writeInOtherTab();
    server.gates.edit = edit.opened;
    server.gates.project = project.opened;
    await type(root, "无");
    await expect.poll(() => unsettled(root)).toBe("1");
    fire();
    await expect.poll(() => server.requests.includes(AUTHOR_EDIT)).toBe(true);
    // The keystroke waits in the queue behind the save, and the editor shows it.
    await type(root, "人");
    await expect.poll(() => manuscriptBody(manuscriptEditor(root, window))).toBe("雨夜。\n渡口。无人");

    edit.open();
    // The catch-up started: the editor is read-only until the tab opens again on the later Revision.
    await expect.poll(() => manuscriptIsEditable(manuscriptEditor(root, window))).toBe(false);
    project.open();

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", editable: true,
      save: "saving", failure: "", stored: NEW_SESSION });
    expect({ edits: count(server, AUTHOR_EDIT), alert: root.querySelector('[role="alert"]')?.textContent ?? "",
      unexpected: server.unexpected }).toEqual({ edits: 1, alert: "", unexpected: [] });
  }, timers);
});

it("catches up and then runs the Chapter selection again when the save of a Chapter switch finds the later Revision",
  async () => {
    const server = fakeServer();
    await withView(server, async (root) => {
      await type(root, "无人");
      await expect.poll(() => unsettled(root)).toBe("1");
      server.writeInOtherTab();

      root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();

      // The selection runs again on the new editor part, which holds input while its quiet step sends the text.
      await expect.poll(() => { const { text, failure, stored } = shown(root); return { text, failure, stored }; },
        { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", failure: "", stored: NEW_SESSION });
      await expect.poll(() => count(server, AUTHOR_EDIT)).toBe(2);
      expect({ alert: root.querySelector('[role="alert"]')?.textContent ?? "", unexpected: server.unexpected })
        .toEqual({ alert: "", unexpected: [] });
    });
  });
