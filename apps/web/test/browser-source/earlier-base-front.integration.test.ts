// ADR 0107, section 2: a tab that comes to the front shows the later Revision of another tab, with its own input on it.
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

const revision = (revisionId: string, first: string) => ({ revision_id: revisionId, body: `${first}\n渡口。`,
  blocks: [{ manuscript_block_id: FIRST, block_kind: "paragraph" as const, text: first },
    { manuscript_block_id: SECOND, block_kind: "paragraph" as const, text: "渡口。" }] });
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
 * A fake Server of one Project. Before `writeInOtherTab`, the Chapter has the base Revision; after it, the later
 * Revision of another tab, and an Author Edit on the base gets `stale_authoritative_head`. The first creation gives the
 * Editor Session on the base, the second one a new Editor Session on the later Revision. An Author Edit on the later
 * Revision gets no response. After `writeInOtherTab`, a
 * Chapter read waits for `gates.chapter` and a Project read waits for `gates.project`, when the test gives them. It records
 * each request, and it records and throws on each request that it does not serve.
 */
function fakeServer() {
  const requests: string[] = [];
  const unexpected: string[] = [];
  const gates: { chapter?: Promise<void>; project?: Promise<void> } = {};
  let later = false;
  const session = async (editorSessionId: string, at: typeof BASE, position: string) => ({ ...scenario.session,
    editor_session: { ...scenario.session.editor_session, editor_session_id: editorSessionId },
    base_snapshot: { ...scenario.session.base_snapshot, snapshot_id: crypto.randomUUID(),
      project_activity_position: position, authoritative_head_revision_id: at.revision_id, materialized_revision: at,
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: await sha256(at.body) } } });
  const stored = session(SESSION, BASE, "0");
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
      if (later) await gates.project;
      return jsonResponse(scenario.project);
    }
    if (route === CHAPTER_READ) {
      if (later) await gates.chapter;
      return jsonResponse({ ...scenario.chapter, project_activity_position: later ? "1" : "0",
        chapter: { ...scenario.chapter.chapter, current_revision: later ? LATER : BASE } });
    }
    if (route === `POST ${project}/anti-forgery-challenges`) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (route === `POST ${project}/editor-sessions`) {
      creations += 1;
      return jsonResponse(creations === 1 ? await stored : await session(NEW_SESSION, LATER, "1"));
    }
    if (route === `GET ${project}/editor-sessions/${SESSION}`) {
      return jsonResponse({ ...await stored, schema_id: "storyos.query.editor-session.response.v2" });
    }
    if (route === `GET ${project}/activity`) return emptyActivityStream();
    // The page also reads the statistics, the open Proposals, the assistance settings, the Skills, and the tree.
    const scope = scenario.project.project_scope;
    if (route === `GET ${project}/manuscript/statistics`) {
      const position = later ? "1" : "0";
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
    if (route === AUTHOR_EDIT && later) {
      const request = requireRequestBody(init);
      // An Author Edit on the later Revision waits, so the test sees that it went out.
      if (request.expected_authoritative_revision_id === LATER_REVISION) return new Promise<Response>(() => {});
      const answer = createAppliedAuthorEditResponse({ request, commandDigest: await digestApplyAuthorEdit(request as never),
        idempotencyKey: requestHeaders(init).get("idempotency-key")! });
      return jsonResponse({ ...answer, effect: { kind: "conflicted", reason: "stale_authoritative_head",
        current_authoritative_revision_id: LATER_REVISION }, receipt: { ...answer.receipt, result: "conflicted",
        prior_heads: [LATER_REVISION], resulting_heads: [LATER_REVISION], proposal_revision_ids: [],
        authoritative_revision_ids: [], authoritative_commit_ids: [], author_action_sequence: null } });
    }
    unexpected.push(route);
    throw new Error(`unexpected request: ${route}`);
  };
  return { fetchImpl, requests, unexpected, gates, writeInOtherTab: () => { later = true; } };
}

/** Timers that never fire, so the input of the tab stays unsent until the tab comes to the front. */
const heldTimers: ChallengeAdmissionTimers = { setTimeoutImpl: () => 0, clearTimeoutImpl: () => {} };

/**
 * Mounts the view on the base Revision, runs `act`, and unmounts the view. A reopen opens the Journal again in the view,
 * so the cleanup closes each connection, and the deletion of the Journal is not blocked.
 */
async function withView(server: ReturnType<typeof fakeServer>, act: (root: HTMLElement) => Promise<void>) {
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
    const view = mountStage1View(loaded.root, { ...loaded, writingTimers: heldTimers });
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

it("puts input during the Chapter read at the front on the later Revision", async () => {
  const server = fakeServer();
  const chapter = gate();
  await withView(server, async (root) => {
    server.writeInOtherTab();
    server.gates.chapter = chapter.opened;
    server.requests.length = 0;
    document.dispatchEvent(new Event("visibilitychange"));
    await expect.poll(() => server.requests.includes(CHAPTER_READ)).toBe(true);

    await type(root, "无人");
    chapter.open();

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", editable: true,
      save: "saving", failure: "", stored: NEW_SESSION });
    expect({ edits: server.requests.filter((request) => request === AUTHOR_EDIT).length,
      unexpected: server.unexpected }).toEqual({ edits: 0, unexpected: [] });
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

    await type(root, "无人");
    expect(manuscriptBody(manuscriptEditor(root, window))).toBe("雨夜。\n渡口。");
    project.open();

    await expect.poll(() => shown(root), { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。", editable: true,
      save: "saved", failure: "", stored: NEW_SESSION });
    expect({ edits: server.requests.filter((request) => request === AUTHOR_EDIT).length,
      unexpected: server.unexpected }).toEqual({ edits: 0, unexpected: [] });
  });
});

it("catches up and then runs the Chapter selection again when the save of a Chapter switch finds the later Revision",
  async () => {
    const server = fakeServer();
    await withView(server, async (root) => {
      await type(root, "无人");
      await expect.poll(() => root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"))
        .toBe("1");
      server.writeInOtherTab();

      root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();

      // The selection runs again on the new editor part, which holds input while its quiet step sends the text.
      await expect.poll(() => { const { text, failure, stored } = shown(root); return { text, failure, stored }; },
        { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", failure: "", stored: NEW_SESSION });
      await expect.poll(() => server.requests.filter((request) => request === AUTHOR_EDIT).length).toBe(2);
      expect({ alert: root.querySelector('[role="alert"]')?.textContent ?? "", unexpected: server.unexpected })
        .toEqual({ alert: "", unexpected: [] });
    });
  });
