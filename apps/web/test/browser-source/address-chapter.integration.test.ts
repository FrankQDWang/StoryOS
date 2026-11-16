import { afterEach, expect, it } from "vitest";

import type { CreateEditorSessionResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { HISTORICAL_ACKNOWLEDGEMENT_MESSAGE } from "../../src/historical-acknowledgement.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import {
  CHAPTER, OWNER, PROJECT, SESSION, chapterRevision, closeTrackedDatabases, createBrowserScenario, deleteJournal,
  emptyActivityStream, jsonResponse, trackDatabase,
} from "./scenario.ts";

// The workspace address `/projects/{project_id}?chapter={chapter_id}` (ADR 0094 section 9, S7-D2c).
const CHAPTER_B = "018f0000-0000-7001-8000-000000000803";
const REVISION_B = "018f0000-0000-7001-8000-000000000805";
const VOLUME = "018f0000-0000-7001-8000-000000000815";
// A Chapter of another Project: the manuscript tree of this Project does not have it.
const FOREIGN_CHAPTER = "018f0000-0000-7001-8000-000000000890";
const CONVERSATION = "018f0000-0000-7001-8000-000000000891";
const NOT_FOUND = "找不到这个章节。";
const CURRENT_CHAPTER_PATH = `/api/v1/projects/${PROJECT}/current-chapter`;

type Writer = CreateEditorSessionResponse["writer"];

/**
 * A fake Server with Chapter A (current) and Chapter B in one Volume. It refuses each `setCurrentChapter` with the
 * historical acknowledgement problem, so the workspace stays at Chapter A. Each other request gets 404.
 */
function fakeServer(writer: Writer) {
  const scenario = createBrowserScenario();
  const session = { ...scenario.session, writer };
  const requests: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    requests.push(`${method} ${path}`);
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scenario.project.project_scope, title: "Project A", revision: "1",
          open: { kind: "current_chapter", current_chapter_id: CHAPTER } }] });
    }
    if (path === `/api/v1/projects/${PROJECT}`) return jsonResponse(scenario.project);
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}`) return jsonResponse(scenario.chapter);
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER_B}`) {
      return jsonResponse({ ...scenario.chapter, chapter: { chapter_id: CHAPTER_B, title: "Chapter B",
        current_revision: chapterRevision(REVISION_B, "") } });
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scenario.project.project_scope,
        tree_revision: "3", snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032",
          project_scope: scenario.project.project_scope },
        volumes: [{ volume_id: VOLUME, title: "Volume A", order: "1", chapters: [
          { chapter_id: CHAPTER, title: "Chapter A", order: "1" },
          { chapter_id: CHAPTER_B, title: "Chapter B", order: "2" },
        ] }] });
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-09-12T00:00:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({ ...session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (path.endsWith("/activity")) return emptyActivityStream();
    if (path === CURRENT_CHAPTER_PATH && method === "PUT") {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "historical_acknowledgement_unavailable",
        message: "The original Set Current Chapter acknowledgement cannot be recovered." }, 409);
    }
    return jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404);
  };
  return { scenario, requests, fetchImpl };
}

const openDatabases = new Set<IDBDatabase>();
const unmounts: (() => void)[] = [];
const startAddress = location.href;

/** Opens the workspace at the address with the query `search`, and waits for the manuscript tree. */
async function openWorkspace(search: string, writer: Writer = { kind: "current_writer", writer_generation: "1" }) {
  const server = fakeServer(writer);
  await deleteJournal(server.scenario.journalName);
  history.replaceState(null, "", `${location.pathname}${search}`);
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const view = mountStage1View(loaded.root, loaded);
  unmounts.push(() => view.unmount());
  if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
    trackDatabase(loaded.state.editor.database, openDatabases);
  }
  await expect.poll(() => loaded.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
  const shown = () => ({ title: loaded.root.querySelector("h2")?.textContent,
    alert: loaded.root.querySelector('[role="alert"]')?.textContent, search: location.search });
  const chapterSteps = () => server.requests.filter((entry) => entry.includes(`/chapters/${CHAPTER_B}`)
    || entry.endsWith("/current-chapter"));
  return { server, loaded, shown, chapterSteps };
}

afterEach(async () => {
  for (const unmount of unmounts.splice(0)) unmount();
  closeTrackedDatabases(openDatabases);
  document.body.replaceChildren();
  sessionStorage.clear();
  history.replaceState(null, "", startAddress);
  await deleteJournal(createBrowserScenario().journalName);
});

it("makes the Chapter of the address current with the steps of a click in the manuscript tree", async () => {
  const { shown, chapterSteps, server } = await openWorkspace(`?chapter=${CHAPTER_B}`);
  await expect.poll(shown).toEqual({ title: "Chapter A", alert: HISTORICAL_ACKNOWLEDGEMENT_MESSAGE, search: "" });
  expect(chapterSteps()).toEqual([`GET /api/v1/projects/${PROJECT}/chapters/${CHAPTER_B}`,
    `PUT ${CURRENT_CHAPTER_PATH}`]);
  // The parameter does not change the Conversation of the panel.
  expect(server.requests.filter((entry) => entry.includes("/conversations/"))).toEqual([]);
});

it("sends no command for the Current Chapter", async () => {
  const { shown, chapterSteps } = await openWorkspace(`?chapter=${CHAPTER}`);
  await expect.poll(() => shown().search).toBe("");
  expect([shown(), chapterSteps()]).toEqual([{ title: "Chapter A", alert: undefined, search: "" }, []]);
});

it.each([
  ["a value that is not a UUID", "?chapter=not-a-chapter", undefined],
  ["a Chapter of another Project", `?chapter=${FOREIGN_CHAPTER}`, undefined],
  ["a workspace that is not the current writer", `?chapter=${CHAPTER_B}`,
    { kind: "read_only", observed_writer_generation: "2", reason: "superseded_by_takeover" } as const],
])("opens nothing for %s and shows one sentence", async (_, search, writer) => {
  const { loaded, shown, chapterSteps } = await openWorkspace(search, writer);
  await expect.poll(shown).toEqual({ title: "Chapter A", alert: NOT_FOUND, search: "" });
  expect(chapterSteps()).toEqual([]);
  // The workspace stays usable: a click in the tree still opens a Chapter.
  loaded.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)?.click();
  await expect.poll(() => chapterSteps()[0]).toBe(`GET /api/v1/projects/${PROJECT}/chapters/${CHAPTER_B}`);
});

it("opens the Conversation of the address after the Chapter step of the same address", async () => {
  const { shown, server } = await openWorkspace(`?conversation=${CONVERSATION}&chapter=${CHAPTER_B}`);
  const conversationRead = `GET /api/v1/projects/${PROJECT}/conversations/${CONVERSATION}`;
  await expect.poll(() => server.requests.includes(conversationRead)).toBe(true);
  expect(server.requests.indexOf(conversationRead)).toBeGreaterThan(server.requests.indexOf(`PUT ${CURRENT_CHAPTER_PATH}`));
  await expect.poll(() => shown().search).toBe("");
});
