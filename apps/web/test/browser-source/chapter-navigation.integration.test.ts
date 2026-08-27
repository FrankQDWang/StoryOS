import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import {
  CHAPTER,
  OWNER,
  PROJECT,
  SESSION,
  closeTrackedDatabases,
  createBrowserScenario,
  deleteJournal,
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

it("selects a tree Chapter through getChapter and keeps pending on the current Chapter", async () => {
  const scenario = createBrowserScenario();
  const chapterB = {
    schema_id: "storyos.query.chapter.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000806",
    project_scope: scenario.project.project_scope,
    project_activity_position: "0",
    chapter: {
      chapter_id: CHAPTER_B,
      title: "Chapter B",
      current_revision: { revision_id: REVISION_B, body: "" },
    },
  };
  const requests: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    requests.push(`${init?.method ?? "GET"} ${path}`);
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
    throw new Error(`unexpected request: ${path}`);
  };
  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  localStorage.setItem("current_chapter", CHAPTER_B);
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
    const { state, root } = loaded;
    if (state.kind === "project-ready" && state.editor.kind === "editor-ready") {
      trackDatabase(state.editor.database, openDatabases);
    }
    await expect.poll(() => chapterTitles(root).join("\n"))
      .toBe("Chapter A\nChapter B\nChapter C");
    expect(root.querySelector("h2")?.textContent).toBe("Chapter A");
    expect(root.querySelector("textarea")?.value).toBe("Base");
    expect(requests.some((entry) => entry.includes(CHAPTER_B))).toBe(false);

    const editor = root.querySelector("textarea");
    if (!(editor instanceof HTMLTextAreaElement)) throw new Error("the editor is missing");
    editor.focus();
    editor.setSelectionRange(editor.value.length, editor.value.length);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await expect.poll(() => editor.value).toBe("Base!");

    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)?.click();
    await expect.poll(() => root.querySelector("h2")?.textContent).toBe("Chapter B");
    expect(root.querySelector("textarea")?.value).toBe("");
    expect(root.querySelector("textarea")?.hasAttribute("readonly")).toBe(true);
    expect(requests).toContain(`GET /api/v1/projects/${PROJECT}/chapters/${CHAPTER_B}`);
    expect(root.querySelector('[role="alert"]')).toBeNull();

    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER}"]`)?.click();
    await expect.poll(() => root.querySelector("h2")?.textContent).toBe("Chapter A");
    expect(root.querySelector("textarea")?.value).toBe("Base!");
    expect(root.querySelector("[data-save-state]")?.getAttribute("data-save-state"))
      .toBe("saving");
    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_C}"]`)?.click();
    await expect.poll(() => root.querySelector('[role="alert"]') !== null).toBe(true);
    expect(root.querySelector("h2")?.textContent).toBe("Chapter A");
    expect(root.querySelector("textarea")?.value).toBe("Base!");
    expect(root.textContent).not.toContain("snapshot_expired");
    expect(root.textContent).not.toContain("The Snapshot is no longer available.");
  } finally {
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    localStorage.removeItem("current_chapter");
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
