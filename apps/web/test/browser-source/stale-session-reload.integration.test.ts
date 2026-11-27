import { afterEach, expect, it } from "vitest";

import type { GetChapterResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { openEditorWorkspace, submitOnePendingAuthorEdit } from "../../src/editor-session.ts";
import type { EditorReadyState } from "../../src/editor-types.ts";
import { appendAuthorEdit } from "../../src/local-edit-journal.ts";
import {
  OWNER, PROJECT, REVISION, SESSION, chapterRevision, closeTrackedDatabases, createAppliedAuthorEditResponse,
  createBrowserScenario, deleteJournal, jsonResponse, requestHeaders, requestResult, requireEditorReady,
  requireRequestBody, trackDatabase,
} from "./scenario.ts";

// ADR 0107: another Editor Session can write, so the stored Editor Session of a tab can have an earlier base.
const LATER_REVISION = "018f0000-0000-7001-8000-000000000a41";
const NEW_SESSION = "018f0000-0000-7001-8000-000000000a42";
const NEW_SNAPSHOT = "018f0000-0000-7001-8000-000000000a43";
const LATER_BODY = "Base, and the text of another tab";
const EDIT = { from: 4, to: 4, text: "!", resultingBody: "Base!" };
const UNDO_GROUP = "018f0000-0000-7001-8000-000000000a44";
// Two inputs of one group that end on the base text, so the Server gives `no_effect`.
const NO_CHANGE = [{ from: 4, to: 4, text: "x", resultingBody: "Basex", inputOrigin: "typing" as const,
  undoGroupId: UNDO_GROUP, createdAt: "2026-08-15T08:00:00.000Z" }, { from: 4, to: 5, text: "",
  resultingBody: "Base", inputOrigin: "typing" as const, undoGroupId: UNDO_GROUP, createdAt: "2026-08-15T08:00:00.001Z" }];
const ACTIVE_KEY = `active_session:${OWNER}:${PROJECT}`;
const SESSION_READ = `GET /api/v1/projects/${PROJECT}/editor-sessions/${SESSION}`;
const CHALLENGE = `POST /api/v1/projects/${PROJECT}/anti-forgery-challenges`;
const CREATION = `POST /api/v1/projects/${PROJECT}/editor-sessions`;
const AUTHOR_EDIT = `POST /api/v1/projects/${PROJECT}/manuscript/author-edits`;

const databases = new Set<IDBDatabase>();
const scenario = createBrowserScenario();

afterEach(async () => {
  closeTrackedDatabases(databases);
  sessionStorage.removeItem(ACTIVE_KEY);
  await deleteJournal(scenario.journalName);
});

async function sha256(text: string): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * A fake Server whose Chapter has a later Revision than the base of the stored Editor Session. It serves only the read
 * of the stored Session, a Challenge, one creation, whose new Session has the later Revision as its base, and, with
 * `edit`, one Author Edit with that zero-authority result. It records each request and throws on each other request.
 */
async function fakeServer(edit?: "no_effect" | "conflicted") {
  const requests: string[] = [];
  const created = { ...scenario.session,
    editor_session: { ...scenario.session.editor_session, editor_session_id: NEW_SESSION },
    base_snapshot: { ...scenario.session.base_snapshot, snapshot_id: NEW_SNAPSHOT, project_activity_position: "1",
      authoritative_head_revision_id: LATER_REVISION, materialized_revision: chapterRevision(LATER_REVISION, LATER_BODY),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: await sha256(LATER_BODY) } } };
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    requests.push(name);
    if (name === SESSION_READ) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v2" });
    }
    if (name === CHALLENGE) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (name === CREATION) return jsonResponse(created);
    if (name === AUTHOR_EDIT && edit !== undefined) {
      const request = requireRequestBody(init);
      const answer = createAppliedAuthorEditResponse({ request, commandDigest: await digestApplyAuthorEdit(request as never),
        idempotencyKey: requestHeaders(init).get("idempotency-key")! });
      const heads = edit === "no_effect" ? [REVISION] : [LATER_REVISION];
      return jsonResponse({ ...answer, effect: edit === "no_effect" ? { kind: "no_effect", reason: "content_unchanged" }
        : { kind: "conflicted", reason: "stale_authoritative_head", current_authoritative_revision_id: LATER_REVISION },
      receipt: { ...answer.receipt, result: edit, prior_heads: heads, resulting_heads: heads, proposal_revision_ids: [],
        authoritative_revision_ids: [], authoritative_commit_ids: [], author_action_sequence: null } });
    }
    throw new Error(`unexpected request: ${name}`);
  };
  return { fetchImpl, requests };
}

const laterChapter: GetChapterResponse = { ...scenario.chapter, project_activity_position: "1",
  chapter: { ...scenario.chapter.chapter, current_revision: chapterRevision(LATER_REVISION, LATER_BODY) } };

async function openStored(server: Awaited<ReturnType<typeof fakeServer>>) {
  await deleteJournal(scenario.journalName);
  sessionStorage.setItem(ACTIVE_KEY, SESSION);
  const first = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: scenario.chapter, profile: scenario.profile, fetchImpl: server.fetchImpl });
  requireEditorReady(first);
  return first;
}

async function submit(workspace: EditorReadyState, server: Awaited<ReturnType<typeof fakeServer>>) {
  await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl: server.fetchImpl,
    cryptoImpl: crypto });
}

// The partition of the stored Editor Session has no work that the Server does not have.
it.each([
  ["an empty Journal", undefined, async () => {}],
  ["an Author Edit with no effect", "no_effect", async (workspace: EditorReadyState,
    server: Awaited<ReturnType<typeof fakeServer>>) => {
    for (const edit of NO_CHANGE) await appendAuthorEdit(workspace, edit);
    await submit(workspace, server);
  }],
] as const)("opens a new Editor Session at a load when the stored Session has an earlier base and %s", async (_, edit,
  leave) => {
  const server = await fakeServer(edit);
  const first = await openStored(server);
  await leave(first, server);
  first.database.close();
  server.requests.length = 0;

  const state = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: laterChapter, profile: scenario.profile, fetchImpl: server.fetchImpl });

  requireEditorReady(state);
  trackDatabase(state.database, databases);
  expect({ session: state.session.editor_session.editor_session_id,
    revision: state.session.base_snapshot.authoritative_head_revision_id, stored: sessionStorage.getItem(ACTIVE_KEY),
    requests: server.requests }).toEqual({ session: NEW_SESSION, revision: LATER_REVISION, stored: NEW_SESSION,
    requests: [SESSION_READ, CHALLENGE, CREATION] });
});

// Each case leaves work in the Journal of the stored Session that the Server does not have.
it.each([
  ["a record that is not settled", undefined, async (workspace: EditorReadyState) => {
    await appendAuthorEdit(workspace, EDIT);
  }],
  ["a conflicted Author Edit", "conflicted", async (workspace: EditorReadyState,
    server: Awaited<ReturnType<typeof fakeServer>>) => {
    await appendAuthorEdit(workspace, EDIT);
    await submit(workspace, server);
  }],
  ["an unresolved Withdrawal", undefined, async (workspace: EditorReadyState) => {
    await requestResult(workspace.database.transaction("metadata", "readwrite").objectStore("metadata").put({
      key: `withdraw:${workspace.partition.journal_partition_id}:${LATER_REVISION}:${NEW_SNAPSHOT}`,
      settlement: "unresolved" }));
  }],
] as const)("keeps the stored Editor Session and its input at a load when its Journal has %s", async (_, edit,
  leave) => {
  const server = await fakeServer(edit);
  const first = await openStored(server);
  await leave(first, server);
  first.database.close();
  server.requests.length = 0;

  const state = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: laterChapter, profile: scenario.profile, fetchImpl: server.fetchImpl });

  if (state.kind === "editor-ready") trackDatabase(state.database, databases);
  // The interim state of ADR 0107 stays until a tab with an earlier base catches up (#1945).
  expect({ kind: state.kind, stored: sessionStorage.getItem(ACTIVE_KEY), requests: server.requests })
    .toEqual({ kind: "editor-read-only-recovery", stored: SESSION, requests: [SESSION_READ] });
});
