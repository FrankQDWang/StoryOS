import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  manuscriptIsEditable,
} from "../support/manuscript-surface.ts";
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
const CHAPTER_C = "018f0000-0000-7001-8000-000000000804";
const REVISION_B = "018f0000-0000-7001-8000-000000000805";
const VOLUME = "018f0000-0000-7001-8000-000000000815";

function chapterTitles(root: HTMLElement): string[] {
  return [...root.querySelectorAll<HTMLButtonElement>(
    'nav[aria-label="稿件目录"] button[data-chapter-id]',
  )].map((button) => button.textContent ?? "");
}

/** The Server of the chapter navigation tests. `extra` answers a request first when it returns a response. */
function navigationFetch(scenario: ReturnType<typeof createBrowserScenario>, chapterB: unknown, requests: string[],
  extra: (path: string, init: RequestInit | undefined) => Promise<Response | undefined>): typeof fetch {
  return async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    requests.push(`${init?.method ?? "GET"} ${path}`);
    const answer = await extra(path, init);
    if (answer !== undefined) return answer;
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
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER_C}`) {
      return jsonResponse({
        schema_id: "storyos.problem.v1",
        code: "snapshot_expired",
        message: "The Snapshot is no longer available.",
      }, 409);
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
            { chapter_id: CHAPTER_C, title: "Chapter C", order: "3" },
          ],
        }],
      });
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
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
    throw new Error(`unexpected request: ${path}`);
  };
}

function chapterBResponse(scenario: ReturnType<typeof createBrowserScenario>) {
  return {
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
}

it("refuses a Chapter switch that cannot open the Chapter or settle the input, and keeps the input", async () => {
  const scenario = createBrowserScenario();
  const requests: string[] = [];
  const fetchImpl = navigationFetch(scenario, chapterBResponse(scenario), requests, async (path) => {
    if (path.endsWith("/manuscript/author-edits")) throw new TypeError("Failed to fetch");
    return undefined;
  });
  const openDatabases = new Set<IDBDatabase>();
  let view: ReturnType<typeof mountStage1View> | undefined;
  await deleteJournal(scenario.journalName);
  try {
    document.body.innerHTML = '<main id="app"></main>';
    const loaded = await loadStoryOSWebState({ documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    view = mountStage1View(loaded.root, loaded);
    const { state, root } = loaded;
    if (state.kind === "project-ready" && state.editor.kind === "editor-ready") {
      trackDatabase(state.editor.database, openDatabases);
    }
    await expect.poll(() => chapterTitles(root).join("\n")).toBe("Chapter A\nChapter B\nChapter C");
    const shown = () => ({
      title: root.querySelector("h2")?.textContent,
      body: manuscriptBody(manuscriptEditor(root, window)),
      alert: root.querySelector('[role="alert"]')?.textContent,
    });

    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_C}"]`)?.click();
    await expect.poll(shown).toEqual({ title: "Chapter A", body: "Base", alert: "无法打开这一章：快照已过期。" });
    expect(manuscriptIsEditable(manuscriptEditor(root, window))).toBe(true);

    focusManuscriptEnd(manuscriptEditor(root, window), window);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)?.click();
    await expect.poll(shown).toEqual({ title: "Chapter A", body: "Base!", alert: "无法切换章节：请先完成当前输入。" });
    expect(root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count")).toBe("1");
    expect(requests.filter((entry) => entry.includes(CHAPTER_B) || entry.endsWith("/current-chapter"))).toEqual([]);
  } finally {
    view?.unmount();
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
