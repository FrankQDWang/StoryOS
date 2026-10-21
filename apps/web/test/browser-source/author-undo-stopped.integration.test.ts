import { expect, it } from "vitest";

import undoFixture from "../../../../generated/golden-wire/storyos-public-release-1/undo-latest-author-action.json";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
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

const VOLUME = "018f0000-0000-7001-8000-000000000815";
const ACTIVE_SESSION_KEY = `active_session:${OWNER}:${PROJECT}`;

/** Presses Undo once on an editor whose Server gives `effect` for the Undo. */
async function undoWith(effect: Record<string, unknown>) {
  const scenario = createBrowserScenario();
  const requests: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    requests.push(`${method} ${path}`);
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({
        schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013",
        owner_user_id: OWNER,
        projects: [{
          project_scope: { owner_user_id: OWNER, project_id: PROJECT },
          title: "Project A",
          lifecycle: { kind: "active" },
          revision: "1",
          open: { kind: "current_chapter", current_chapter_id: CHAPTER },
        }],
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
        volumes: [{
          volume_id: VOLUME,
          title: "Volume A",
          order: "1",
          chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }],
        }],
      });
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-09-12T00:00:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({
        ...scenario.session,
        schema_id: "storyos.query.editor-session.response.v1",
        author_undo_frontier_sequence: "1",
      });
    }
    if (path.endsWith("/activity")) return emptyActivityStream();
    if (path.endsWith("/author-actions/undo") && method === "POST") {
      return jsonResponse({
        ...structuredClone(undoFixture),
        receipt: { ...undoFixture.receipt, result: "refused", author_action_sequence: null,
          authoritative_commit_ids: [], authoritative_revision_ids: [] },
        effect,
      });
    }
    throw new Error(`unexpected request: ${method} ${path}`);
  };

  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  // All browser test files share one sessionStorage, so an earlier file can keep an active Session key.
  sessionStorage.removeItem(ACTIVE_SESSION_KEY);
  try {
    document.body.innerHTML = '<main id="app"></main>';
    const loaded = await loadStoryOSWebState({
      documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    mountStage1View(loaded.root, loaded);
    if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
      trackDatabase(loaded.state.editor.database, openDatabases);
      loaded.state.editor.session = {
        ...loaded.state.editor.session,
        author_undo_frontier_sequence: "1",
      };
    }
    await expect.poll(() => loaded.root.querySelector("[contenteditable='true']") !== null).toBe(true);
    // The view loads the tree and the activity after the editor. A load during the Undo stops the Undo.
    await expect.poll(() => requests.some((request) => request.endsWith("/activity"))).toBe(true);
    const surface = loaded.root.querySelector<HTMLElement>("[contenteditable='true']")!;
    surface.focus();
    const mac = navigator.platform.includes("Mac");
    surface.dispatchEvent(new KeyboardEvent("keydown", {
      key: "z",
      code: "KeyZ",
      metaKey: mac,
      ctrlKey: !mac,
      bubbles: true,
      cancelable: true,
    }));
    const notice = () => loaded.root.querySelector('[role="alert"]')?.textContent ?? "";
    const failure = () => loaded.root.querySelector("[data-editor-failure]")?.getAttribute("data-editor-failure") ?? "";
    // Before ADR 0067 the editor fails and shows no notice, so wait for the first of the two.
    await expect.poll(() => notice() !== "" || failure() !== "").toBe(true);
    return {
      notice: notice(),
      save: loaded.root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
      failure: failure(),
      editable: loaded.root.querySelector("[data-manuscript-editor]")?.getAttribute("contenteditable"),
    };
  } finally {
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    sessionStorage.removeItem(ACTIVE_SESSION_KEY);
    await deleteJournal(scenario.journalName);
  }
}

it.each([
  ["barrier", "无法继续撤销：更早的操作不能撤销。"],
  ["no_frontier", "没有可以撤销的操作。"],
  ["source_unavailable", "无法撤销：这个操作的记录已不可用。"],
])("keeps the editor writable and shows the reason when Undo is unavailable with %s", async (reason, notice) => {
  expect(await undoWith({ kind: "unavailable", reason })).toEqual({
    notice,
    save: "saved",
    failure: "",
    editable: "true",
  });
});
