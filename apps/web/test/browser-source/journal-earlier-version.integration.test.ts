import { expect, it } from "vitest";

import exportedFixture from "../../../../generated/golden-wire/storyos-public-release-1/export-human-readable-manuscript.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import openProposalsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-chapter-open-proposals.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import statisticsFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-statistics.json";
import routeCatalog from "../../../../generated/route-catalog/versioned-protocol-release-1-route-catalog.json";
import readyExportFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-human-readable-manuscript-export.json";
import { digestExportHumanReadableManuscript, type ExportHumanReadableManuscriptRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { appendAuthorEdit, openEditorWorkspace } from "../../src/editor-session.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { FIRST_APPEND_EDIT } from "./local-edit-journal-append-fixture.ts";
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

// A Journal of version 3 or 4 has no upgrade path (ADR 0099). The page opens the Server text read-only, and the
// Journal stays as it is. A version 5 Journal upgrades to version 6 and keeps each row (ADR 0107).
const VOLUME = "018f0000-0000-7001-8000-000000000816";
const FIXTURE_PROJECT = exportedFixture.project_scope.project_id;
const forProject = <T,>(value: T): T => JSON.parse(JSON.stringify(value).replaceAll(FIXTURE_PROJECT, PROJECT)) as T;
const EXPORT_ROUTE = "/api/v1/projects/{project_id}/manuscript/exports";
const { operations } = routeCatalog as { operations: {
  kind: string; method: string; path: string; schemas: { request: string } }[] };
/** The rows of the route catalog that are the Project command targets of the Server. */
const PROJECT_COMMANDS = operations.filter(({ kind, path }) =>
  kind === "command" && path.startsWith("/api/v1/projects/{project_id}"));
const NONCE = "a".repeat(64);
type Challenge = { method: string; route_template: string; command_schema: string; idempotency_key: string;
  canonical_command_digest: unknown };
const EXPORT_TEXT = "# Volume A\n\n## Chapter A\n\nBase\n";
const EXPORT_SHA256 = [...new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(EXPORT_TEXT)))]
  .map((byte) => byte.toString(16).padStart(2, "0")).join("");
const problem = (code: string, message: string, status: number) =>
  jsonResponse({ schema_id: "storyos.problem.v1", code, message }, status);

async function journalRows(name: string, openDatabases: Set<IDBDatabase>) {
  const kept = await requestResult(indexedDB.open(name));
  trackDatabase(kept, openDatabases);
  const transaction = kept.transaction([...kept.objectStoreNames]);
  return { version: kept.version, rows: Object.fromEntries(await Promise.all([...kept.objectStoreNames].map(async (store) =>
    [store, await requestResult(transaction.objectStore(store).getAll())]))) };
}

// The fake Server serves the exact requests of the test: the method and the path with the Project of the test.
// It records each other request and throws, so that request fails the test.
function journalServer(scenario: ReturnType<typeof createBrowserScenario>) {
  const unserved: string[] = [];
  const issued = new Map<string, { challenge: Challenge; expiresAt: number }>();
  let admitted = false;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    const route = `${method} ${path}`;
    if (route === "GET /api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (route === "GET /api/v1/projects") {
      return jsonResponse({
        schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000817",
        owner_user_id: OWNER,
        projects: [{
          project_scope: { owner_user_id: OWNER, project_id: PROJECT },
          title: "Project A",
          revision: "1",
          open: { kind: "current_chapter", current_chapter_id: CHAPTER },
        }],
      });
    }
    if (route === `GET /api/v1/projects/${PROJECT}`) return jsonResponse(scenario.project);
    if (route === `GET /api/v1/projects/${PROJECT}/chapters/${CHAPTER}`) return jsonResponse(scenario.chapter);
    if (route === `GET /api/v1/projects/${PROJECT}/manuscript/tree`) {
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
    if (route === `GET /api/v1/projects/${PROJECT}/manuscript/statistics`) {
      // The Ready answer of the Server (`crates/storyos-server/src/manuscript_statistics.rs:50`) for the one Chapter of
      // the Project, with the Server text "Base", at the activity position "0" of the scenario. The watermark and the
      // generation are that position, and page_bytes is the byte length of the JSON totals.
      const fixture = forProject(statisticsFixture);
      const manuscript = { chapter_count: "1", word_count: "1", character_count: "4" };
      return jsonResponse({ ...fixture, manuscript, page_bytes: String(JSON.stringify(manuscript).length),
        current_chapter: { chapter_id: CHAPTER, word_count: "1", character_count: "4" },
        projection_generation: "0", projection_watermark: "0",
        source_snapshot: { ...fixture.source_snapshot, project_activity_position: "0" } });
    }
    if (route === `POST /api/v1/projects/${PROJECT}/anti-forgery-challenges`) {
      const challenge = JSON.parse(String(init?.body)) as Challenge;
      // `crates/storyos-server/src/lib.rs:908`: a target that is not a Project command of the route catalog.
      if (!PROJECT_COMMANDS.some((operation) => operation.method === challenge.method
        && operation.path === challenge.route_template && operation.schemas.request === challenge.command_schema)) {
        return problem("command_target_refused", "The command target is not in the closed Release 1 contract.", 422);
      }
      // The Server issues a nonce that is valid for five minutes (`project_command_challenge.rs:142`).
      const expiresAt = Date.now() + 300_000;
      issued.set(challenge.idempotency_key, { challenge, expiresAt });
      return jsonResponse({ nonce: NONCE, expires_at: new Date(expiresAt).toISOString(),
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (route === `POST /api/v1/projects/${PROJECT}/editor-sessions`) return jsonResponse(scenario.session);
    if (route === `GET /api/v1/projects/${PROJECT}/editor-sessions/${SESSION}`) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v2" });
    }
    if (route === `GET /api/v1/projects/${PROJECT}/activity`) return emptyActivityStream();
    // The page reads the open Proposals of the Chapter and the assistance settings. This Project has none of them.
    const scope = { owner_user_id: OWNER, project_id: PROJECT };
    if (route === `GET /api/v1/projects/${PROJECT}/chapters/${CHAPTER}/open-proposals`) {
      return jsonResponse({ ...openProposalsFixture, project_scope: scope, chapter_id: CHAPTER, proposals: [] });
    }
    if (route === `GET /api/v1/projects/${PROJECT}/assistance`) {
      return jsonResponse({ ...assistanceFixture, project_scope: scope });
    }
    if (route === `GET /api/v1/projects/${PROJECT}/skills`) {
      return jsonResponse({ ...skillsFixture, project_scope: scope, skills: [] });
    }
    if (route === `POST /api/v1/projects/${PROJECT}/manuscript/exports`) {
      // `crates/storyos-server/src/lib.rs:931`: a command with no valid Challenge gets `challenge_invalid`.
      // The command must match its Challenge: nonce, key, method, route, schema, digest, and expiry
      // (`crates/storyos-adapter-postgres/src/lib.rs:470`).
      const headers = new Headers(init?.headers);
      const key = headers.get("idempotency-key") ?? "";
      const binding = issued.get(key)?.challenge;
      const request = JSON.parse(String(init?.body)) as ExportHumanReadableManuscriptRequest;
      if (headers.get("x-storyos-anti-forgery") !== NONCE || binding === undefined
        || binding.idempotency_key !== key || binding.method !== method || binding.route_template !== EXPORT_ROUTE
        || binding.command_schema !== request.command_schema || Date.now() >= issued.get(key)!.expiresAt
        || JSON.stringify(binding.canonical_command_digest)
          !== JSON.stringify(await digestExportHumanReadableManuscript(request, crypto))) {
        return problem("challenge_invalid", "The command challenge is invalid or expired.", 422);
      }
      admitted = true;
      return jsonResponse(forProject(exportedFixture));
    }
    if (route === `GET /api/v1/projects/${PROJECT}/manuscript/exports/${readyExportFixture.export_id}`) {
      // `readable_export.rs:109`: no export exists before the Server admits the export command (`lib.rs:876`).
      if (!admitted) return problem("resource_unavailable", "The requested resource is unavailable.", 404);
      // The export of this Project has the Server text of its one Chapter.
      return jsonResponse({ ...forProject(readyExportFixture), manuscript_utf8: EXPORT_TEXT,
        content_sha256: EXPORT_SHA256 });
    }
    unserved.push(route);
    throw new Error(`The fake Server does not serve ${route}`);
  };
  return { fetchImpl, unserved };
}

it.each([3, 4] as const)("shows the Server text read-only for a version %i Journal, exports it, and keeps the Journal",
  async (version) => {
    const scenario = createBrowserScenario();
    const server = journalServer(scenario);
    const { fetchImpl } = server;

    const openDatabases = new Set<IDBDatabase>();
    await deleteJournal(scenario.journalName);
    try {
      const writing = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
        chapter: scenario.chapter, profile: scenario.profile, fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
      requireEditorReady(writing);
      trackDatabase(writing.database, openDatabases);
      await appendAuthorEdit(writing, FIRST_APPEND_EDIT);
      const rows = await restoreJournalVersion(writing.database, version);
      expect(rows.intents).toHaveLength(1);

      document.body.innerHTML = '<main id="app"></main>';
      const loaded = await loadStoryOSWebState({ documentImpl: document,
        locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
        fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
      if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
        trackDatabase(loaded.state.editor.database, openDatabases);
      }
      expect(loaded.state).toMatchObject({ kind: "project-ready",
        editor: { kind: "editor-read-only-recovery", code: "local_journal_unavailable" } });
      mountStage1View(loaded.root, loaded);
      await expect.poll(() => loaded.root.querySelector("[data-save-state]")?.getAttribute("data-save-state"))
        .toBe("needs_attention");
      // The editor shows the Server text and not the text of the Journal ("Base!").
      const editor = loaded.root.querySelector("[data-manuscript-editor]");
      expect({ editable: editor?.getAttribute("contenteditable"), text: editor?.textContent })
        .toEqual({ editable: "false", text: "Base" });

      await expect.poll(() => loaded.root.querySelector("[data-readable-export] button") !== null).toBe(true);
      loaded.root.querySelector<HTMLButtonElement>("[data-readable-export] button")?.click();
      await expect.poll(() => loaded.root.querySelector("[data-readable-export-bytes]")?.textContent)
        .toBe(EXPORT_TEXT);

      // The open did not upgrade or change the Journal.
      expect(await journalRows(scenario.journalName, openDatabases)).toEqual({ version, rows });
      // The page sent only the requests that the fake Server serves.
      expect(server.unserved).toEqual([]);
    } finally {
      closeTrackedDatabases(openDatabases);
      document.body.replaceChildren();
      sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
      await deleteJournal(scenario.journalName);
    }
  });

it("upgrades a version 5 Journal, opens the editor, and keeps the earlier rows unread and unsent", async () => {
  const scenario = createBrowserScenario();
  const server = journalServer(scenario);
  const options = { baseUrl: location.origin, project: scenario.project, chapter: scenario.chapter,
    profile: scenario.profile, fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto };
  const openDatabases = new Set<IDBDatabase>();
  await deleteJournal(scenario.journalName);
  try {
    const writing = await openEditorWorkspace(options);
    requireEditorReady(writing);
    trackDatabase(writing.database, openDatabases);
    await appendAuthorEdit(writing, FIRST_APPEND_EDIT);
    const rows = await restoreJournalVersion(writing.database, 5);

    const upgraded = await openEditorWorkspace(options);
    requireEditorReady(upgraded);
    trackDatabase(upgraded.database, openDatabases);
    // The editor shows the Server text and not the text of the earlier partition ("Base!").
    expect({ body: upgraded.openedProjection.body, unsettled: upgraded.openedProjection.unsettled_intent_count })
      .toEqual({ body: "Base", unsettled: 0 });
    upgraded.database.close();
    const kept = await journalRows(scenario.journalName, openDatabases);
    expect({ version: kept.version, intents: kept.rows.intents }).toEqual({ version: 6, intents: rows.intents });
    expect(server.unserved).toEqual([]);
  } finally {
    closeTrackedDatabases(openDatabases);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
  }
});

it("enters local recovery while a page of version 5 keeps the Journal open, and upgrades after that page closes",
  async () => {
    const scenario = createBrowserScenario();
    const server = journalServer(scenario);
    const options = { baseUrl: location.origin, project: scenario.project, chapter: scenario.chapter,
      profile: scenario.profile, fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto };
    const openDatabases = new Set<IDBDatabase>();
    await deleteJournal(scenario.journalName);
    try {
      const writing = await openEditorWorkspace(options);
      requireEditorReady(writing);
      trackDatabase(writing.database, openDatabases);
      await restoreJournalVersion(writing.database, 5);
      // The page of version 5 keeps its connection and does not close it at a version change.
      const earlier = await requestResult(indexedDB.open(scenario.journalName, 5));
      trackDatabase(earlier, openDatabases);

      expect(await openEditorWorkspace(options)).toMatchObject({ kind: "editor-read-only-recovery",
        code: "local_journal_unavailable" });
      earlier.close();
      await expect.poll(async () => (await indexedDB.databases())
        .find((database) => database.name === scenario.journalName)?.version).toBe(6);
      const upgraded = await openEditorWorkspace(options);
      requireEditorReady(upgraded);
      trackDatabase(upgraded.database, openDatabases);
      expect(server.unserved).toEqual([]);
    } finally {
      closeTrackedDatabases(openDatabases);
      sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
      await deleteJournal(scenario.journalName);
    }
  });

it("does not serve a request with another method, on another Project, or for another target", async () => {
  const server = journalServer(createBrowserScenario());
  const otherProject = "018f0000-0000-7001-8000-000000000819";
  const otherChapter = "018f0000-0000-7001-8000-000000000820";
  const requests: [string, RequestInit][] = [
    [`/api/v1/projects/${PROJECT}/anti-forgery-challenges`, { method: "GET" }],
    [`/api/v1/projects/${otherProject}/anti-forgery-challenges`, { method: "POST", body: "{}" }],
    [`/api/v1/projects/${PROJECT}/chapters/${otherChapter}`, { method: "GET" }],
  ];
  for (const [path, init] of requests) {
    await expect(server.fetchImpl(`${location.origin}${path}`, init)).rejects.toThrow("The fake Server does not serve");
  }
  expect(server.unserved).toEqual([
    `GET /api/v1/projects/${PROJECT}/anti-forgery-challenges`,
    `POST /api/v1/projects/${otherProject}/anti-forgery-challenges`,
    `GET /api/v1/projects/${PROJECT}/chapters/${otherChapter}`,
  ]);
  server.unserved.length = 0;
});
