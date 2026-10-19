import { expect, it, vi } from "vitest";

import exportedFixture from "../../../../generated/golden-wire/storyos-public-release-1/export-human-readable-manuscript.json";
import readyExportFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-human-readable-manuscript-export.json";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { appendAuthorEdit, openEditorWorkspace } from "../../src/editor-session.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { createJournalUuid, rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import { FIRST_APPEND_EDIT, openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { restoreJournalVersion } from "./journal-version-fixture.ts";
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
  requestResult,
  requireEditorReady,
  trackDatabase,
} from "./scenario.ts";

const VOLUME = "018f0000-0000-7001-8000-000000000816";
const FIXTURE_PROJECT = exportedFixture.project_scope.project_id;
const forProject = <T,>(value: T): T => JSON.parse(JSON.stringify(value).replaceAll(FIXTURE_PROJECT, PROJECT)) as T;

it("keeps a version 4 Journal that cannot upgrade, shows local recovery, and exports the Server text", async () => {
  const scenario = createBrowserScenario();
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({
        schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000817",
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
        correlation_id: "018f0000-0000-7001-8000-000000000818",
        project_scope: { owner_user_id: OWNER, project_id: PROJECT },
        tree_revision: "2",
        snapshot: {
          snapshot_id: "018f0000-0000-7001-8000-000000000032",
          project_scope: { owner_user_id: OWNER, project_id: PROJECT },
        },
        volumes: [{ volume_id: VOLUME, title: "Volume A", order: "1",
          chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }] }],
      });
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/statistics`) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "projection_not_ready",
        message: "The statistics projection is not ready." }, 409);
    }
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: "2026-09-12T00:00:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (path.endsWith("/activity")) return emptyActivityStream();
    if (path === `/api/v1/projects/${PROJECT}/manuscript/exports` && method === "POST") {
      return jsonResponse(forProject(exportedFixture));
    }
    if (path === `/api/v1/projects/${PROJECT}/manuscript/exports/${readyExportFixture.export_id}`) {
      return jsonResponse(forProject(readyExportFixture));
    }
    throw new Error(`unexpected request: ${method} ${path}`);
  };

  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  try {
    const writing = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
      chapter: scenario.chapter, profile: scenario.profile, fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    requireEditorReady(writing);
    trackDatabase(writing.database, openDatabases);
    await appendAuthorEdit(writing, FIRST_APPEND_EDIT);
    const versionFour = await restoreJournalVersion(writing.database, 4);
    expect(versionFour.intents).toHaveLength(1);

    const put = IDBObjectStore.prototype.put;
    const failedUpgrade = vi.spyOn(IDBObjectStore.prototype, "put").mockImplementation(function (
      this: IDBObjectStore, value: unknown, key?: IDBValidKey,
    ) {
      const request = key === undefined ? put.call(this, value) : put.call(this, value, key);
      if (this.name === "metadata" && value !== null && typeof value === "object"
        && Reflect.get(value, "key") === "schema" && Reflect.get(value, "version") === 5) this.transaction.abort();
      return request;
    });
    document.body.innerHTML = '<main id="app"></main>';
    let loaded: Awaited<ReturnType<typeof loadStoryOSWebState>>;
    try {
      loaded = await loadStoryOSWebState({ documentImpl: document,
        locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
        fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    } finally { failedUpgrade.mockRestore(); }
    if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
      trackDatabase(loaded.state.editor.database, openDatabases);
    }
    expect(loaded.state).toMatchObject({ kind: "project-ready",
      editor: { kind: "editor-read-only-recovery", code: "local_journal_unavailable" } });
    mountStage1View(loaded.root, loaded);
    await expect.poll(() => loaded.root.querySelector("[data-save-state]")?.getAttribute("data-save-state"))
      .toBe("needs_attention");
    expect(loaded.root.querySelector("[data-manuscript-editor]")?.getAttribute("contenteditable")).toBe("false");

    // The readable export contains the Server text only. The local text stays in the version 4 Journal.
    await expect.poll(() => loaded.root.querySelector("[data-readable-export] button") !== null).toBe(true);
    loaded.root.querySelector<HTMLButtonElement>("[data-readable-export] button")?.click();
    await expect.poll(() => loaded.root.querySelector("[data-readable-export-bytes]")?.textContent)
      .toBe(readyExportFixture.manuscript_utf8);

    const kept = await requestResult(indexedDB.open(scenario.journalName));
    trackDatabase(kept, openDatabases);
    expect(kept.version).toBe(4);
    const transaction = kept.transaction([...kept.objectStoreNames]);
    expect(Object.fromEntries(await Promise.all([...kept.objectStoreNames].map(async (name) =>
      [name, await requestResult(transaction.objectStore(name).getAll())])))).toEqual(versionFour);
  } finally {
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

// A record that follows a group must name an earlier frozen authoritative group (ADR 0065).
it("fails closed for a record that follows a group that the Journal does not have", async () => {
  const test = await openJournalAppendTestWorkspace();
  try {
    await appendAuthorEdit(test.workspace, FIRST_APPEND_EDIT);
    const intents = test.workspace.database.transaction("intents", "readwrite").objectStore("intents");
    const [record] = await requestResult(intents.getAll());
    await requestResult(intents.put({ ...record,
      follows_submission_group: { journal_submission_group_id: createJournalUuid() } }));
    await expect(rebuildPendingProjection(test.workspace)).rejects.toThrow("Local Edit Journal is corrupt");
    await expect(test.reopen()).rejects.toThrow("workspace unavailable: local_journal_unavailable");
  } finally { await test.close(); }
});
