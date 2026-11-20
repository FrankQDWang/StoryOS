import { expect, it } from "vitest";

import { HISTORICAL_ACKNOWLEDGEMENT_MESSAGE } from "../../src/historical-acknowledgement.ts";
import {
  closeOpenedChapter,
  historicalProblem,
  mountApp,
  openedChapterRoute,
  recordingFetch,
} from "./historical-acknowledgement-fixture.ts";
import {
  CHAPTER,
  PROJECT,
  chapterRevision,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  trackDatabase,
} from "./scenario.ts";

const CHAPTER_B = "018f0000-0000-7001-8000-000000000803";
const REVISION_B = "018f0000-0000-7001-8000-000000000805";

it("explains a historical Set Current Chapter acknowledgement and does not retry it", async () => {
  const scenario = createBrowserScenario();
  const chapterB = {
    schema_id: "storyos.query.chapter.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000806",
    project_scope: scenario.project.project_scope,
    project_activity_position: "0",
    chapter: {
      chapter_id: CHAPTER_B,
      title: "Chapter B",
      current_revision: chapterRevision(REVISION_B, ""),
    },
  };
  const chapters = [
    { chapter_id: CHAPTER, title: "Chapter A", order: "1" },
    { chapter_id: CHAPTER_B, title: "Chapter B", order: "2" },
  ];
  const { fetchImpl, methods } = recordingFetch(openedChapterRoute(scenario, "3", chapters, (method, path) => {
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER_B}`) {
      return jsonResponse(chapterB);
    }
    if (path === `/api/v1/projects/${PROJECT}/current-chapter` && method === "PUT") {
      return historicalProblem("The original Set Current Chapter acknowledgement cannot be recovered.");
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
      loaded.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)
        !== null
    ).toBe(true);
    loaded.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)
      ?.click();
    await expect.poll(() =>
      loaded.root.querySelector('[role="alert"]')?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(loaded.root.querySelector("h2")?.textContent).toBe("Chapter A");
    expect(methods.filter((entry) => entry === `PUT /api/v1/projects/${PROJECT}/current-chapter`))
      .toEqual([`PUT /api/v1/projects/${PROJECT}/current-chapter`]);
  } finally {
    await closeOpenedChapter(scenario, openDatabases);
  }
});
