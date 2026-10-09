import { expect, it, vi } from "vitest";

import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
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
  createAppliedAuthorEditResponse,
  createBrowserScenario,
  deleteJournal,
  emptyActivityStream,
  jsonResponse,
  requestHeaders,
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

it("selects a tree Chapter through getChapter and keeps pending on the current Chapter", async () => {
  const scenario = createBrowserScenario();
  const chapterB = chapterBResponse(scenario);
  const requests: string[] = [];
  const fetchImpl = navigationFetch(scenario, chapterB, requests, async () => undefined);
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
    expect(manuscriptBody(manuscriptEditor(root, window))).toBe("Base");
    expect(requests.some((entry) => entry.includes(CHAPTER_B))).toBe(false);

    const editor = manuscriptEditor(root, window);
    focusManuscriptEnd(editor, window);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await expect.poll(() => manuscriptBody(editor)).toBe("Base!");

    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)?.click();
    await expect.poll(() => root.querySelector("h2")?.textContent).toBe("Chapter B");
    expect(manuscriptBody(manuscriptEditor(root, window))).toBe("");
    expect(manuscriptIsEditable(manuscriptEditor(root, window))).toBe(false);
    expect(requests).toContain(`GET /api/v1/projects/${PROJECT}/chapters/${CHAPTER_B}`);
    expect(root.querySelector('[role="alert"]')).toBeNull();

    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER}"]`)?.click();
    await expect.poll(() => root.querySelector("h2")?.textContent).toBe("Chapter A");
    expect(manuscriptBody(manuscriptEditor(root, window))).toBe("Base!");
    expect(root.querySelector("[data-save-state]")?.getAttribute("data-save-state"))
      .toBe("saving");
    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_C}"]`)?.click();
    await expect.poll(() => root.querySelector('[role="alert"]') !== null).toBe(true);
    expect(root.querySelector("h2")?.textContent).toBe("Chapter A");
    expect(manuscriptBody(manuscriptEditor(root, window))).toBe("Base!");
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

it("shows a save that completes while the author returns to the current Chapter", async () => {
  const scenario = createBrowserScenario();
  let canonical = { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" };
  const gate = () => {
    let open!: () => void;
    const opened = new Promise<void>((resolve) => { open = resolve; });
    return { open, opened };
  };
  const returnStarted = gate(), returnReleased = gate();
  let chapterReads = 0;
  const requests: string[] = [];
  const fetchImpl = navigationFetch(scenario, chapterBResponse(scenario), requests, async (path, init) => {
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(canonical);
    if (path === `/api/v1/projects/${PROJECT}/chapters/${CHAPTER}` && (chapterReads += 1) > 1) {
      returnStarted.open();
      await returnReleased.opened;
      return jsonResponse(scenario.chapter);
    }
    if (!path.endsWith("/manuscript/author-edits")) return undefined;
    const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
    const response = createAppliedAuthorEditResponse({ request, body: "Base!",
      commandDigest: await digestApplyAuthorEdit(request as never), idempotencyKey: requestHeaders(init).get("idempotency-key")! });
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode("Base!"));
    canonical = { ...canonical, base_snapshot: { ...canonical.base_snapshot,
      snapshot_id: "018f0000-0000-7001-8000-000000000080", project_activity_position: "1",
      authoritative_head_revision_id: response.effect.kind === "authoritative_applied"
        ? response.effect.authoritative_revision.revision_id : "",
      materialized_revision: { ...canonical.base_snapshot.materialized_revision, revision_id: "018f0000-0000-7001-8000-000000000034",
        body: "Base!", blocks: canonical.base_snapshot.materialized_revision.blocks.map((block) => ({ ...block, text: "Base!" })) },
      materialized_payload_digest: { ...canonical.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("") } } };
    return jsonResponse(response);
  });
  const openDatabases = new Set<IDBDatabase>();
  let view: ReturnType<typeof mountStage1View> | undefined;
  await deleteJournal(scenario.journalName);
  try {
    document.body.innerHTML = '<main id="app"></main>';
    const loaded = await loadStoryOSWebState({ documentImpl: document,
      locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
      fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    const { state, root } = loaded;
    const collected = gate();
    let armed = false, fiveStoreTransactions = 0;
    if (state.kind === "project-ready" && state.editor.kind === "editor-ready") {
      const database = state.editor.database;
      trackDatabase(database, openDatabases);
      // After the settlement, the controller collects the payload in one transaction, and then installs the projection.
      state.editor.database = new Proxy(database, { get(target, key) {
        if (key !== "transaction") return Reflect.get(target, key, target);
        return (...args: Parameters<IDBDatabase["transaction"]>) => {
          const transaction = target.transaction(...args);
          if (armed && args[1] === "readwrite" && JSON.stringify([args[0]].flat().sort())
            === JSON.stringify(["intents", "metadata", "partitions", "payload_chains", "submission_groups"])) {
            // The first transaction with these stores freezes the group. The second collects it after the settlement.
            if ((fiveStoreTransactions += 1) === 2) {
              transaction.addEventListener("complete", () => { collected.open(); }, { once: true });
            }
          }
          return transaction;
        };
      } });
    }
    view = mountStage1View(root, loaded);
    await expect.poll(() => chapterTitles(root).join("\n")).toBe("Chapter A\nChapter B\nChapter C");
    // The Author Edit idle pause starts only when the test advances it, after the return to Chapter A is quiet.
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    focusManuscriptEnd(manuscriptEditor(root, window), window);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)?.click();
    await expect.poll(() => root.querySelector("h2")?.textContent).toBe("Chapter B");
    root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER}"]`)?.click();
    await returnStarted.opened;
    armed = true;
    vi.advanceTimersByTime(250);
    vi.useRealTimers();
    await collected.opened;
    // One task lets the controller install the saved projection while the view still shows Chapter B.
    await new Promise((resolve) => { setTimeout(resolve, 0); });
    // Record the save state of each render after the return, so a short stale state also shows.
    const shown: (string | null)[] = [];
    const observer = new MutationObserver(() => {
      const state = root.querySelector("[data-save-state]")?.getAttribute("data-save-state");
      if (root.querySelector("h2")?.textContent === "Chapter A" && state !== undefined && shown.at(-1) !== state) {
        shown.push(state);
      }
    });
    observer.observe(root, { subtree: true, childList: true, characterData: true, attributes: true });
    returnReleased.open();
    await expect.poll(() => shown.at(-1)).toBe("saved");
    observer.disconnect();
    expect({ shown, body: manuscriptBody(manuscriptEditor(root, window)) }).toEqual({ shown: ["saved"], body: "Base!" });
  } finally {
    vi.useRealTimers();
    returnReleased.open();
    // The unmount closes the controller and stops the Activity synchronization of the view.
    view?.unmount();
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});
