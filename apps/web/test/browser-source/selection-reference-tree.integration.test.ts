import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { draftEntries, removeDraftReference } from "../../src/selection-reference-draft.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { beginInlineChapterCreation } from "../support/inline-chapter-creation.ts";
import { manuscriptEditor } from "../support/manuscript-surface.ts";
import {
  CHAPTER,
  OWNER,
  PROJECT,
  SESSION,
  closeTrackedDatabases,
  createBrowserScenario,
  deleteJournal,
  emptyActivityStream,
  jsonResponse,
  trackDatabase,
} from "./scenario.ts";

const VOLUME_A = "018f0000-0000-7001-8000-000000001175";
const VOLUME_Z = "018f0000-0000-7001-8000-000000001176";
const PROLOGUE = "018f0000-0000-7001-8000-000000001177";
type TreeChapter = { chapter_id: string; title: string; order: string };
type TreeVolume = { volume_id: string; title: string; order: string; chapters: TreeChapter[] };
type Answer = (path: string, method: string) => Promise<Response | undefined>;

function manuscriptTree(revision: string, volumes: TreeVolume[]) {
  return {
    schema_id: "storyos.query.manuscript-tree.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000001178",
    project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    tree_revision: revision,
    snapshot: {
      snapshot_id: "018f0000-0000-7001-8000-000000001179",
      project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    },
    volumes,
  };
}

const chapterA: TreeChapter = { chapter_id: CHAPTER, title: "Chapter A", order: "1" };

/** The Server of a workspace whose current Chapter is Chapter A. `answer` answers a request first when it returns a response. */
function workspaceFetch(scenario: ReturnType<typeof createBrowserScenario>, answer: Answer): typeof fetch {
  return async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    const answered = await answer(path, method);
    if (answered !== undefined) return answered;
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({
        schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013",
        owner_user_id: OWNER,
        projects: [{
          project_scope: { owner_user_id: OWNER, project_id: PROJECT },
          title: "Project A",
          revision: "1",
          open: { kind: "current_chapter", current_chapter_id: CHAPTER },
        }],
      });
    }
    if (path === `/api/v1/projects/${PROJECT}`) return jsonResponse(scenario.project);
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}`) return jsonResponse(scenario.chapter);
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (path.endsWith("/activity")) return emptyActivityStream();
    throw new Error(`unexpected request: ${method} ${path}`);
  };
}

/** Opens the workspace of Chapter A ("Base") and runs `body` with its root. */
async function withWorkspace(answer: Answer, body: (root: HTMLElement) => Promise<void>) {
  const scenario = createBrowserScenario();
  const openDatabases = new Set<IDBDatabase>();
  let view: ReturnType<typeof mountStage1View> | undefined;
  await deleteJournal(scenario.journalName);
  try {
    document.body.innerHTML = '<main id="app"></main>';
    const loaded = await loadStoryOSWebState({ documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl: workspaceFetch(scenario, answer), indexedDBImpl: indexedDB, cryptoImpl: crypto });
    view = mountStage1View(loaded.root, loaded);
    if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
      trackDatabase(loaded.state.editor.database, openDatabases);
    }
    await body(loaded.root);
  } finally {
    view?.unmount();
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    // The draft of the page outlives the session storage, so the next test starts with no references.
    const scope = { owner_user_id: OWNER, project_id: PROJECT };
    for (const entry of draftEntries(scope)) removeDraftReference(scope, entry.key);
    sessionStorage.clear();
    await deleteJournal(scenario.journalName);
  }
}

/** Selects the text between two offsets of the first editor paragraph, as a mouse selection does. */
function selectFirstParagraph(root: HTMLElement, from: number, to: number) {
  const editor = manuscriptEditor(root, window);
  editor.focus();
  const text = editor.querySelector("p")!.firstChild!;
  document.getSelection()!.setBaseAndExtent(text, from, text, to);
}

const composerLabels = (root: HTMLElement) => [...root.querySelectorAll("[data-selection-reference-label]")]
  .map((item) => item.getAttribute("data-selection-reference-label"));

it("shows the add control after a failed outline read, with no reload and no new selection", async () => {
  let treeReads = 0;
  let release!: () => void;
  const held = new Promise<void>((resolve) => { release = resolve; });
  await withWorkspace(async (path) => {
    if (path !== `/api/v1/projects/${PROJECT}/manuscript/tree`) return undefined;
    treeReads += 1;
    if (treeReads === 1) throw new TypeError("Failed to fetch");
    await held;
    return jsonResponse(manuscriptTree("3", [{ volume_id: VOLUME_A, title: "Volume A", order: "1", chapters: [chapterA] }]));
  }, async (root) => {
    await expect.poll(() => root.querySelector("[data-manuscript-editor]")?.textContent).toBe("Base");
    // The author selects text while the outline is not there, so the control has no label yet.
    selectFirstParagraph(root, 0, 2);
    await expect.poll(() => treeReads, { timeout: 5_000 }).toBe(2);
    expect(root.querySelector("[data-add-selection-reference]")).toBeNull();
    release();
    await expect.poll(() => root.querySelector("[data-add-selection-reference]")?.tagName).toBe("BUTTON");
    root.querySelector<HTMLButtonElement>("[data-add-selection-reference]")!.click();
    await expect.poll(() => composerLabels(root)).toEqual(["第 1 章 Chapter A（第 1 段）"]);
  });
});

it("gives an added reference the Chapter number of the outline after a Chapter is created before it", async () => {
  let created = false;
  await withWorkspace(async (path, method) => {
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse(manuscriptTree(created ? "4" : "3", [
        { volume_id: VOLUME_Z, title: "Volume Z", order: "1",
          chapters: created ? [{ chapter_id: PROLOGUE, title: "Prologue", order: "1" }] : [] },
        { volume_id: VOLUME_A, title: "Volume A", order: "2", chapters: [chapterA] },
      ]));
    }
    if (path === `/api/v1/projects/${PROJECT}/volumes/${VOLUME_Z}/chapters` && method === "POST") {
      created = true;
      return jsonResponse({ effect: { kind: "authoritative_applied", chapter_id: PROLOGUE, tree_revision: "4" } });
    }
    return undefined;
  }, async (root) => {
    await expect.poll(() => root.querySelector(`button[data-chapter-id="${CHAPTER}"]`)?.tagName).toBe("BUTTON");
    selectFirstParagraph(root, 0, 2);
    await expect.poll(() => root.querySelector("[data-add-selection-reference]")?.tagName).toBe("BUTTON");
    root.querySelector<HTMLButtonElement>("[data-add-selection-reference]")!.click();
    await expect.poll(() => composerLabels(root)).toEqual(["第 1 章 Chapter A（第 1 段）"]);

    await beginInlineChapterCreation(root, VOLUME_Z);
    const input = root.querySelector<HTMLInputElement>(`form[data-create-chapter="${VOLUME_Z}"] input[name="chapter-title"]`)!;
    input.value = "Prologue";
    input.form!.requestSubmit();
    await expect.poll(() => root.querySelector(`button[data-chapter-id="${PROLOGUE}"]`)?.tagName).toBe("BUTTON");
    await expect.poll(() => composerLabels(root)).toEqual(["第 2 章 Chapter A（第 1 段）"]);
  });
});
