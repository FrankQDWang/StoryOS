import { expect, it, vi } from "vitest";

import exportedFixture from "../../../../generated/golden-wire/storyos-public-release-1/export-human-readable-manuscript.json";
import readyExportFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-human-readable-manuscript-export.json";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { appendAuthorEdit, openEditorWorkspace } from "../../src/editor-session.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { localUnsentTextFile } from "../../src/local-unsent-text-panel.tsx";
import { createJournalUuid, rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import { FIRST_APPEND_EDIT, openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { restoreJournalVersion } from "./journal-version-fixture.ts";
import {
  BLOCK,
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
const PROPOSAL = "018f0000-0000-7001-8000-000000000c01";
const OPERATION = "018f0000-0000-7001-8000-000000000c02";
const CANDIDATE_REVISION = "018f0000-0000-7001-8000-000000000c03";
const FIXTURE_PROJECT = exportedFixture.project_scope.project_id;
const forProject = <T,>(value: T): T => JSON.parse(JSON.stringify(value).replaceAll(FIXTURE_PROJECT, PROJECT)) as T;

/** Each upgrade to version 5 aborts until the returned function restores the store. */
function failVersionFiveUpgrade(): () => void {
  const put = IDBObjectStore.prototype.put;
  const spy = vi.spyOn(IDBObjectStore.prototype, "put").mockImplementation(function (
    this: IDBObjectStore, value: unknown, key?: IDBValidKey,
  ) {
    const request = key === undefined ? put.call(this, value) : put.call(this, value, key);
    if (this.name === "metadata" && value !== null && typeof value === "object"
      && Reflect.get(value, "key") === "schema" && Reflect.get(value, "version") === 5) this.transaction.abort();
    return request;
  });
  return () => spy.mockRestore();
}

async function journalRows(name: string, openDatabases: Set<IDBDatabase>) {
  const kept = await requestResult(indexedDB.open(name));
  trackDatabase(kept, openDatabases);
  const transaction = kept.transaction([...kept.objectStoreNames]);
  return { version: kept.version, rows: Object.fromEntries(await Promise.all([...kept.objectStoreNames].map(async (store) =>
    [store, await requestResult(transaction.objectStore(store).getAll())]))) };
}

it("keeps a version 4 Journal that cannot upgrade, shows local recovery, and exports the Server text and the local unsent text", async () => {
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

    const restoreUpgrade = failVersionFiveUpgrade();
    document.body.innerHTML = '<main id="app"></main>';
    let loaded: Awaited<ReturnType<typeof loadStoryOSWebState>>;
    try {
      loaded = await loadStoryOSWebState({ documentImpl: document,
        locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
        fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
    } finally { restoreUpgrade(); }
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

    // The author downloads the unsent local text as a file (contract 3.2).
    await expect.poll(() => loaded.root.querySelector("[data-local-unsent-text] button") !== null).toBe(true);
    const blobs: Blob[] = [];
    const names: string[] = [];
    const createUrl = vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
      blobs.push(blob as Blob);
      return "blob:local-unsent-text";
    });
    const revokeUrl = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
      names.push(this.download);
    });
    try {
      loaded.root.querySelector<HTMLButtonElement>("[data-local-unsent-text] button")?.click();
    } finally {
      createUrl.mockRestore();
      revokeUrl.mockRestore();
      click.mockRestore();
    }
    expect({ names, text: await blobs[0]?.text() }).toEqual({ names: ["storyos-local-unsent-text.txt"],
      text: "StoryOS 本机未发送的文字（未保存到服务器）\n\n章节：Chapter A\nBase!" });

    // The reads did not upgrade or change the version 4 Journal.
    expect(await journalRows(scenario.journalName, openDatabases)).toEqual({ version: 4, rows: versionFour });
  } finally {
    closeTrackedDatabases(openDatabases);
    document.body.replaceChildren();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

/** Writes one edit, rewrites the Journal as `version`, and opens it while each upgrade to version 5 aborts. */
async function openAfterFailedUpgrade(edit: Parameters<typeof appendAuthorEdit>[1], version: 3 | 4) {
  const scenario = createBrowserScenario();
  const openDatabases = new Set<IDBDatabase>();
  const test = await openJournalAppendTestWorkspace();
  try {
    await appendAuthorEdit(test.workspace, edit);
    const rows = await restoreJournalVersion(test.workspace.database, version);
    const restoreUpgrade = failVersionFiveUpgrade();
    try {
      const state = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
        chapter: scenario.chapter, profile: scenario.profile, fetchImpl: async () => { throw new Error("no request"); },
        indexedDBImpl: indexedDB, cryptoImpl: crypto });
      return { state, kept: (await journalRows(scenario.journalName, openDatabases)), rows };
    } finally { restoreUpgrade(); }
  } finally {
    closeTrackedDatabases(openDatabases);
    await test.close();
  }
}

it("tells the author that a version 3 Journal that cannot upgrade has local text that the editor cannot read", async () => {
  const { state, kept, rows } = await openAfterFailedUpgrade(FIRST_APPEND_EDIT, 3);
  expect(state).toMatchObject({ kind: "editor-read-only-recovery", code: "local_journal_unavailable",
    localUnsentText: { readable: false, chapters: [], proposals: [] } });
  expect(kept).toEqual({ version: 3, rows });
});

it("gives the unsent candidate text of a version 4 Journal that cannot upgrade after the Chapter text", async () => {
  const { state, kept, rows } = await openAfterFailedUpgrade({ kind: "candidate_selection",
    target: { proposal_id: PROPOSAL, operation_id: OPERATION, revision_id: CANDIDATE_REVISION, manuscript_block_id: BLOCK },
    expectedProposalHeads: [CANDIDATE_REVISION], priorText: "Candidate", from: 9, to: 9, text: "!",
    resultingBody: "Candidate!" }, 4);
  if (state.kind !== "editor-read-only-recovery" || state.localUnsentText === undefined) throw new Error("no local text");
  // The Chapter text is the Server text, so the file has only the candidate text.
  expect({ text: state.localUnsentText, file: localUnsentTextFile(state.localUnsentText, new Map()) }).toEqual({
    text: { readable: true, chapters: [], proposals: [{ proposal_id: PROPOSAL, text: "Candidate!" }] },
    file: `StoryOS 本机未发送的文字（未保存到服务器）\n\n提案：${PROPOSAL}\nCandidate!` });
  expect(kept).toEqual({ version: 4, rows });
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
