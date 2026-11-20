import { expect, it } from "vitest";

import { HISTORICAL_ACKNOWLEDGEMENT_MESSAGE } from "../../src/historical-acknowledgement.ts";
import {
  closeOpenedChapter,
  historicalProblem,
  mountApp,
  openedChapterRoute,
  recordingFetch,
} from "./historical-acknowledgement-fixture.ts";
import { CHAPTER, OWNER, PROJECT, createBrowserScenario, deleteJournal, trackDatabase } from "./scenario.ts";

const ACTIVE_SESSION_KEY = `active_session:${OWNER}:${PROJECT}`;

it("explains a historical Undo Latest Author Action acknowledgement and does not retry it", async () => {
  const scenario = createBrowserScenario();
  const chapters = [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }];
  const { fetchImpl, methods } = recordingFetch(openedChapterRoute(scenario, "2", chapters, (method, path) => {
    if (path.endsWith("/author-actions/undo") && method === "POST") {
      return historicalProblem("The original Undo Latest Author Action acknowledgement cannot be recovered.");
    }
    return undefined;
  }, { author_undo_frontier_sequence: "1" }));

  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  // All browser test files share one sessionStorage. An earlier file can keep an
  // active Editor Session key, and then the editor reads that Session and does not
  // create one. This test needs the create path and its Challenge.
  sessionStorage.removeItem(ACTIVE_SESSION_KEY);
  try {
    const loaded = await mountApp(fetchImpl, `/projects/${PROJECT}`, indexedDB);
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
    expect(methods.filter((entry) => entry.endsWith("/editor-sessions"))).toEqual([
      `POST /api/v1/projects/${PROJECT}/editor-sessions`,
    ]);
    expect(methods.filter((entry) => entry.includes("anti-forgery-challenges"))).toHaveLength(2);
  } finally {
    await closeOpenedChapter(scenario, openDatabases);
  }
});
