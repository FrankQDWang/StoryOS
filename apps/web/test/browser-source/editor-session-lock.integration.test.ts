import { afterEach, expect, it } from "vitest";

import { openEditorWorkspace } from "../../src/editor-session.ts";
import {
  OWNER, PROJECT, SESSION, closeTrackedDatabases, createBrowserScenario, deleteJournal, jsonResponse,
  requireEditorReady, trackDatabase,
} from "./scenario.ts";

// The browser command "Duplicate tab" copies `sessionStorage`, so a new page can find the Editor Session of another
// page. Two pages of one Editor Session would freeze and settle the Journal records of each other.
const NEW_SESSION = "018f0000-0000-7001-8000-000000000a51";
const ACTIVE_KEY = `active_session:${OWNER}:${PROJECT}`;
const CHALLENGE = `POST /api/v1/projects/${PROJECT}/anti-forgery-challenges`;
const CREATION = `POST /api/v1/projects/${PROJECT}/editor-sessions`;

const databases = new Set<IDBDatabase>();
const scenario = createBrowserScenario();

afterEach(async () => {
  closeTrackedDatabases(databases);
  sessionStorage.removeItem(ACTIVE_KEY);
  await deleteJournal(scenario.journalName);
});

/** A fake Server that serves only a Challenge and one creation of a new Editor Session. It throws on each other request. */
function fakeServer() {
  const requests: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    requests.push(name);
    if (name === CHALLENGE) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (name === CREATION) {
      return jsonResponse({ ...scenario.session,
        editor_session: { ...scenario.session.editor_session, editor_session_id: NEW_SESSION } });
    }
    throw new Error(`unexpected request: ${name}`);
  };
  return { fetchImpl, requests };
}

it("creates a new Editor Session when another page writes with the stored one", async () => {
  sessionStorage.setItem(ACTIVE_KEY, SESSION);
  const server = fakeServer();
  let endOtherPage = () => {};
  const otherPage = new Promise<void>((resolve) => { endOtherPage = resolve; });
  const held = new Promise<void>((taken) => {
    void navigator.locks.request(`storyos-editor-session:${SESSION}`, () => {
      taken();
      return otherPage;
    });
  });
  await held;
  try {
    const state = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
      chapter: scenario.chapter, profile: scenario.profile, fetchImpl: server.fetchImpl });

    requireEditorReady(state);
    trackDatabase(state.database, databases);
    expect({ session: state.session.editor_session.editor_session_id, stored: sessionStorage.getItem(ACTIVE_KEY),
      requests: server.requests }).toEqual({ session: NEW_SESSION, stored: NEW_SESSION,
      requests: [CHALLENGE, CREATION] });
  } finally {
    endOtherPage();
  }
});
