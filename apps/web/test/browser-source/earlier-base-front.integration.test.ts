// ADR 0107, section 2: a tab that comes to the front shows the later Revision of another tab, with its own input on it.
import { afterEach, expect, it } from "vitest";

import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import type { ChallengeAdmissionTimers } from "../../src/challenge-admission-wait.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import { focusManuscriptEnd, manuscriptBody, manuscriptEditor, manuscriptIsEditable }
  from "../support/manuscript-surface.ts";
import {
  CHAPTER, OWNER, PROJECT, REVISION, SESSION, createAppliedAuthorEditResponse, createBrowserScenario, deleteJournal,
  emptyActivityStream, jsonResponse, requestHeaders, requireRequestBody,
} from "./scenario.ts";

const LATER_REVISION = "018f0000-0000-7001-8000-000000000a61";
const NEW_SESSION = "018f0000-0000-7001-8000-000000000a62";
const [FIRST, SECOND] = ["018f0000-0000-7001-8000-000000000a63", "018f0000-0000-7001-8000-000000000a64"];
const ACTIVE_KEY = `active_session:${OWNER}:${PROJECT}`;
const AUTHOR_EDIT = `POST /api/v1/projects/${PROJECT}/manuscript/author-edits`;
const scenario = createBrowserScenario();

const revision = (revisionId: string, first: string) => ({ revision_id: revisionId, body: `${first}\n渡口。`,
  blocks: [{ manuscript_block_id: FIRST, block_kind: "paragraph" as const, text: first },
    { manuscript_block_id: SECOND, block_kind: "paragraph" as const, text: "渡口。" }] });
const BASE = revision(REVISION, "雨夜。");
const LATER = revision(LATER_REVISION, "雨夜，风起。");

afterEach(async () => {
  document.body.replaceChildren();
  sessionStorage.removeItem(ACTIVE_KEY);
  await deleteJournal(scenario.journalName);
});

async function sha256(text: string): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)));
  return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * A fake Server of one Project. Before `writeInOtherTab`, the Chapter has the base Revision; after it, the later
 * Revision of another tab, and an Author Edit on the base gets `stale_authoritative_head`. The first creation gives the
 * Editor Session on the base, the second one a new Editor Session on the later Revision. It records each request and
 * throws on each request that it does not serve.
 */
function fakeServer() {
  const requests: string[] = [];
  let later = false;
  const session = async (editorSessionId: string, at: typeof BASE, position: string) => ({ ...scenario.session,
    editor_session: { ...scenario.session.editor_session, editor_session_id: editorSessionId },
    base_snapshot: { ...scenario.session.base_snapshot, snapshot_id: crypto.randomUUID(),
      project_activity_position: position, authoritative_head_revision_id: at.revision_id, materialized_revision: at,
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: await sha256(at.body) } } });
  const stored = session(SESSION, BASE, "0");
  let creations = 0;
  const fetchImpl: typeof fetch = async (input, init) => {
    const route = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    requests.push(route);
    const project = `/api/v1/projects/${PROJECT}`;
    if (route === "GET /api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (route === "GET /api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: { owner_user_id: OWNER, project_id: PROJECT }, title: "Project A", revision: "1",
          open: { kind: "current_chapter", current_chapter_id: CHAPTER } }] });
    }
    if (route === `GET ${project}`) return jsonResponse(scenario.project);
    if (route === `GET ${project}/chapters/${CHAPTER}`) {
      return jsonResponse({ ...scenario.chapter, project_activity_position: later ? "1" : "0",
        chapter: { ...scenario.chapter.chapter, current_revision: later ? LATER : BASE } });
    }
    if (route === `POST ${project}/anti-forgery-challenges`) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (route === `POST ${project}/editor-sessions`) {
      creations += 1;
      return jsonResponse(creations === 1 ? await stored : await session(NEW_SESSION, LATER, "1"));
    }
    if (route === `GET ${project}/editor-sessions/${SESSION}`) {
      return jsonResponse({ ...await stored, schema_id: "storyos.query.editor-session.response.v2" });
    }
    if (route === `GET ${project}/activity`) return emptyActivityStream();
    if (route === AUTHOR_EDIT && later) {
      const request = requireRequestBody(init);
      const answer = createAppliedAuthorEditResponse({ request, commandDigest: await digestApplyAuthorEdit(request as never),
        idempotencyKey: requestHeaders(init).get("idempotency-key")! });
      return jsonResponse({ ...answer, effect: { kind: "conflicted", reason: "stale_authoritative_head",
        current_authoritative_revision_id: LATER_REVISION }, receipt: { ...answer.receipt, result: "conflicted",
        prior_heads: [LATER_REVISION], resulting_heads: [LATER_REVISION], proposal_revision_ids: [],
        authoritative_revision_ids: [], authoritative_commit_ids: [], author_action_sequence: null } });
    }
    throw new Error(`unexpected request: ${route}`);
  };
  return { fetchImpl, requests, writeInOtherTab: () => { later = true; } };
}

/** Timers that never fire, so the input of the tab stays unsent until the tab comes to the front. */
const heldTimers: ChallengeAdmissionTimers = { setTimeoutImpl: () => 0, clearTimeoutImpl: () => {} };

it("shows the later Revision with the unsent input of the tab when the tab comes to the front", async () => {
  sessionStorage.removeItem(ACTIVE_KEY);
  const server = fakeServer();
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const view = mountStage1View(loaded.root, { ...loaded, writingTimers: heldTimers });
  try {
    await expect.poll(() => loaded.root.querySelector("[contenteditable='true']") !== null).toBe(true);
    focusManuscriptEnd(manuscriptEditor(loaded.root, window), window);
    await applyTrustedInput({ operation: "insert_text", text: "无人" });
    await expect.poll(() => loaded.root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"))
      .toBe("1");
    server.writeInOtherTab();

    document.dispatchEvent(new Event("visibilitychange"));

    const shown = () => {
      const editor = manuscriptEditor(loaded.root, window);
      const status = loaded.root.querySelector("[data-save-state]");
      return { text: manuscriptBody(editor), editable: manuscriptIsEditable(editor),
        save: status?.getAttribute("data-save-state"), failure: status?.getAttribute("data-editor-failure"),
        stored: sessionStorage.getItem(ACTIVE_KEY) };
    };
    await expect.poll(shown, { timeout: 10_000 }).toEqual({ text: "雨夜，风起。\n渡口。无人", editable: true,
      save: "saving", failure: "", stored: NEW_SESSION });
    expect(server.requests.filter((request) => request === AUTHOR_EDIT)).toHaveLength(1);
  } finally {
    view.unmount();
    if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
      loaded.state.editor.database.close();
    }
  }
});
