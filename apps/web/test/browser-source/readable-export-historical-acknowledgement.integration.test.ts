import { expect, it } from "vitest";

import { HISTORICAL_ACKNOWLEDGEMENT_MESSAGE } from "../../src/historical-acknowledgement.ts";
import {
  closeOpenedChapter,
  historicalProblem,
  mountApp,
  openedChapterRoute,
  recordingFetch,
} from "./historical-acknowledgement-fixture.ts";
import { CHAPTER, PROJECT, createBrowserScenario, deleteJournal, jsonResponse, trackDatabase } from "./scenario.ts";

it("explains a historical human-readable export acknowledgement and does not retry it", async () => {
  const scenario = createBrowserScenario();
  const chapters = [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }];
  const { fetchImpl, methods } = recordingFetch(openedChapterRoute(scenario, "2", chapters, (method, path) => {
    if (method === "GET" && path === `/api/v1/projects/${PROJECT}/manuscript/statistics`) {
      return jsonResponse({
        schema_id: "storyos.problem.v1",
        code: "projection_not_ready",
        message: "The statistics projection is not ready.",
      }, 409);
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/exports` && method === "POST") {
      return historicalProblem("The original human-readable export acknowledgement cannot be recovered.");
    }
    return undefined;
  }));

  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  try {
    const loaded = await mountApp(fetchImpl, `/projects/${PROJECT}`, indexedDB);
    if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
      trackDatabase(loaded.state.editor.database, openDatabases);
    }
    await expect.poll(() =>
      loaded.root.querySelector<HTMLButtonElement>("[data-readable-export] button")
        !== null
    ).toBe(true);
    loaded.root.querySelector<HTMLButtonElement>("[data-readable-export] button")?.click();
    await expect.poll(() =>
      loaded.root.querySelector("[data-readable-export-error]")?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(loaded.root.querySelector("h2")?.textContent).toBe("Chapter A");
    expect(methods.filter((entry) =>
      entry === `POST /api/v1/projects/${PROJECT}/manuscript/exports`
    )).toEqual([
      `POST /api/v1/projects/${PROJECT}/manuscript/exports`,
    ]);
  } finally {
    await closeOpenedChapter(scenario, openDatabases);
  }
});
