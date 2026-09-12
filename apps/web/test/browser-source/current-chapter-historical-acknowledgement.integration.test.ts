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
  chapterRevision,
  closeTrackedDatabases,
  createBrowserScenario,
  deleteJournal,
  emptyActivityStream,
  jsonResponse,
  trackDatabase,
} from "./scenario.ts";

const CHAPTER_B = "018f0000-0000-7001-8000-000000000803";
const REVISION_B = "018f0000-0000-7001-8000-000000000805";
const VOLUME = "018f0000-0000-7001-8000-000000000815";

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
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER_B}`) {
      return jsonResponse(chapterB);
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse({
        schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014",
        project_scope: { owner_user_id: OWNER, project_id: PROJECT },
        tree_revision: "3",
        snapshot: {
          snapshot_id: "018f0000-0000-7001-8000-000000000032",
          project_scope: { owner_user_id: OWNER, project_id: PROJECT },
        },
        volumes: [{
          volume_id: VOLUME,
          title: "Volume A",
          order: "1",
          chapters: [
            { chapter_id: CHAPTER, title: "Chapter A", order: "1" },
            { chapter_id: CHAPTER_B, title: "Chapter B", order: "2" },
          ],
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
      });
    }
    if (path.endsWith("/activity")) return emptyActivityStream();
    if (path === `/api/v1/projects/${PROJECT}/current-chapter` && method === "PUT") {
      return jsonResponse({
        schema_id: "storyos.problem.v1",
        code: "historical_acknowledgement_unavailable",
        message: "The original Set Current Chapter acknowledgement cannot be recovered.",
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
    }
    await expect.poll(() =>
      loaded.root.querySelector<HTMLButtonElement>(`[data-make-current-chapter="${CHAPTER_B}"]`)
        !== null
    ).toBe(true);
    loaded.root.querySelector<HTMLButtonElement>(`[data-make-current-chapter="${CHAPTER_B}"]`)
      ?.click();
    await expect.poll(() =>
      loaded.root.querySelector('[role="alert"]')?.textContent
    ).toBe(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
    expect(loaded.root.querySelector("h2")?.textContent).toBe("Chapter A");
    expect(methods.filter((entry) => entry === `PUT /api/v1/projects/${PROJECT}/current-chapter`))
      .toEqual([`PUT /api/v1/projects/${PROJECT}/current-chapter`]);
  } finally {
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
