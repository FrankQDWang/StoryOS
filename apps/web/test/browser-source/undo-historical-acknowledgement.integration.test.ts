import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { HISTORICAL_ACKNOWLEDGEMENT_MESSAGE } from "../../src/historical-acknowledgement.ts";
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

it("explains a historical Undo Latest Author Action acknowledgement and does not retry it", async () => {
  const scenario = createBrowserScenario();
  const methods: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    methods.push(`${method} ${path}`);
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
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}`) {
      return jsonResponse(scenario.chapter);
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse({
        schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014",
        project_scope: scenario.project.project_scope,
        tree_revision: "2",
        snapshot: {
          snapshot_id: "018f0000-0000-7001-8000-000000000032",
          project_scope: scenario.project.project_scope,
        },
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
        schema_id: "storyos.problem.v1",
        code: "historical_acknowledgement_unavailable",
        message: "The original Undo Latest Author Action acknowledgement cannot be recovered.",
      }, 409);
    }
    throw new Error(`unexpected request: ${method} ${path}`);
  };

  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  try {
    document.body.innerHTML = '<main id="app"></main>';
    const loaded = await loadStoryOSWebState({
      documentImpl: document,
      locationImpl: {
        origin: location.origin,
        pathname: `/projects/${PROJECT}`,
      },
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
    await expect.poll(() =>
      loaded.root.querySelector("[contenteditable='true']") !== null
    ).toBe(true);
    const surface = loaded.root.querySelector<HTMLElement>("[contenteditable='true']");
    if (surface === null) {
      throw new Error("the manuscript editor is missing");
    }
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
    await expect.poll(() =>
      loaded.root.querySelector('[role="alert"]')?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(loaded.root.querySelector("[data-save-state]")?.getAttribute("data-save-state")).toBe("saved");
    expect(loaded.root.querySelector("[data-editor-failure]")?.getAttribute("data-editor-failure")).toBe("");
    expect(methods.filter((entry) => entry.endsWith("/author-actions/undo"))).toEqual([
      `POST /api/v1/projects/${PROJECT}/author-actions/undo`,
    ]);
    expect(methods.filter((entry) => entry.includes("anti-forgery-challenges"))).toHaveLength(2);
  } finally {
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
