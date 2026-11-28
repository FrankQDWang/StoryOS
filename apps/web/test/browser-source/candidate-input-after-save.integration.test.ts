// #1944: input in a candidate stays in the candidate after an earlier saved Chapter text edit of the same open.
import type { Editor } from "@tiptap/core";
import { afterEach, beforeAll, expect, it, vi } from "vitest";

import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import statisticsFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-statistics.json";
import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import type { ChallengeAdmissionTimers } from "../../src/challenge-admission-wait.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { applyTrustedInput } from "../support/browser-command-client.ts";
import { manuscriptEditor, manuscriptIsEditable } from "../support/manuscript-surface.ts";
import {
  BLOCK, CHAPTER, OWNER, PROJECT, REVISION, SESSION, closeTrackedDatabases,
  createAppliedAuthorEditResponse, createBrowserScenario, deleteJournal, emptyActivityStream, jsonResponse,
  requestHeaders, requireRequestBody, trackDatabase,
} from "./scenario.ts";

const P = `/api/v1/projects/${PROJECT}`;
const AUTHOR_EDIT = `POST ${P}/manuscript/author-edits`;
const ACTIVE_KEY = `active_session:${OWNER}:${PROJECT}`;
const SAVED_REVISION = "018f0000-0000-7001-8000-000000000034";
// The first block of the Chapter. The open Proposal targets the second block, `BLOCK`.
const FIRST = "018f0000-0000-7001-8000-0000000019a1";
const revision = (revisionId: string, first: string) => ({ revision_id: revisionId, body: `${first}\nBase`,
  blocks: [{ manuscript_block_id: FIRST, block_kind: "paragraph" as const, text: first },
    { manuscript_block_id: BLOCK, block_kind: "paragraph" as const, text: "Base" }] });
const BASE = revision(REVISION, "雨夜。");
const SAVED = revision(SAVED_REVISION, "雨夜。!");
const scenario = createBrowserScenario();
const PROPOSAL = { ...proposalFixture.proposal, chapter_id: CHAPTER, manuscript_block_id: BLOCK,
  base_authoritative_revision_id: REVISION, operations: proposalFixture.proposal.operations
    .map((operation) => ({ ...operation, manuscript_block_id: BLOCK })) };

// The editor needs the white space rule of the product style, else Chrome drops a line feed at the end of a block.
beforeAll(async () => {
  const styles = Object.assign(document.createElement("link"), { rel: "stylesheet", href: "/src/writing-workspace.css" });
  const loaded = new Promise((resolve, reject) => { styles.onload = resolve; styles.onerror = reject; });
  document.head.append(styles);
  await loaded;
});

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
 * A fake Server of one Project whose Chapter has two blocks and one open Proposal for the second block. The first
 * Author Edit, of the first block with no Proposal head, applies as on the Server and moves the Editor Session base.
 * Each later Author Edit gets no response, so the test sees that it went out. It records each Author Edit request, and
 * it records and throws on each request that it does not serve.
 */
async function fakeServer() {
  const edits: Record<string, unknown>[] = [];
  const unexpected: string[] = [];
  const base = (at: typeof BASE, snapshotId: string, position: string, digest: string) => ({
    ...scenario.session.base_snapshot, snapshot_id: snapshotId, project_activity_position: position,
    authoritative_head_revision_id: at.revision_id, materialized_revision: at,
    materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
      value_hex_lowercase: digest } });
  const created = { ...scenario.session,
    base_snapshot: base(BASE, scenario.session.base_snapshot.snapshot_id, "0", await sha256(BASE.body)) };
  let session = { ...created, schema_id: "storyos.query.editor-session.response.v2" };
  const fetchImpl: typeof fetch = async (input, init) => {
    const route = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    const scope = scenario.project.project_scope;
    if (route === "GET /api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (route === "GET /api/v1/projects") {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: "Project A", revision: "1",
          open: { kind: "current_chapter", current_chapter_id: CHAPTER } }] });
    }
    if (route === `GET ${P}`) return jsonResponse(scenario.project);
    if (route === `GET ${P}/chapters/${CHAPTER}`) {
      return jsonResponse({ ...scenario.chapter, chapter: { ...scenario.chapter.chapter, current_revision: BASE } });
    }
    if (route === `POST ${P}/anti-forgery-challenges`) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (route === `POST ${P}/editor-sessions`) return jsonResponse(created);
    if (route === `GET ${P}/editor-sessions/${SESSION}`) return jsonResponse(session);
    if (route === `GET ${P}/chapters/${CHAPTER}/open-proposals`) {
      return jsonResponse({ schema_id: "storyos.query.chapter-open-proposals.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000c71", project_scope: scope, chapter_id: CHAPTER,
        completeness: "complete", proposals: [{ proposal_id: PROPOSAL.proposal_id, revision_id: PROPOSAL.revision_id,
          kind: PROPOSAL.kind, source: PROPOSAL.source }] });
    }
    if (route === `GET ${P}/proposals/${PROPOSAL.proposal_id}`) {
      return jsonResponse({ ...proposalFixture, project_scope: scope, proposal: PROPOSAL });
    }
    if (route === `GET ${P}/activity`) return emptyActivityStream();
    if (route === `GET ${P}/manuscript/statistics`) {
      const count = { word_count: "1", character_count: "4" };
      return jsonResponse({ ...statisticsFixture, project_scope: scope, current_chapter: { chapter_id: CHAPTER, ...count } });
    }
    if (route === `GET ${P}/assistance`) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (route === `GET ${P}/skills`) return jsonResponse({ ...skillsFixture, project_scope: scope, skills: [] });
    if (route === `GET ${P}/manuscript/tree`) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope, tree_revision: "1",
        snapshot: { snapshot_id: "018f0000-0000-7001-8000-000000000032", project_scope: scope },
        volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000815", title: "Volume A", order: "1",
          chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" }] }] });
    }
    if (route === AUTHOR_EDIT) {
      const request = requireRequestBody(init);
      edits.push(request);
      if (edits.length > 1) return new Promise<Response>(() => {});
      const response = createAppliedAuthorEditResponse({ request, authoritativeRevisionId: SAVED_REVISION,
        commandDigest: await digestApplyAuthorEdit(request as never),
        idempotencyKey: requestHeaders(init).get("idempotency-key")! });
      session = { ...session,
        base_snapshot: base(SAVED, "018f0000-0000-7001-8000-000000000080", "1", await sha256(SAVED.body)) };
      return jsonResponse({ ...response, effect: { ...response.effect, authoritative_revision: SAVED } });
    }
    unexpected.push(route);
    throw new Error(`unexpected request: ${route}`);
  };
  return { fetchImpl, edits, unexpected };
}

/** Timers that fire only when the test calls `fire`, so the test starts each save. */
function manualTimers() {
  const due = new Map<number, () => void>();
  let next = 0;
  const timers: ChallengeAdmissionTimers = {
    setTimeoutImpl: (callback) => { next += 1; due.set(next, callback); return next; },
    clearTimeoutImpl: (timer) => { due.delete(timer as number); },
  };
  const fire = () => {
    const callbacks = [...due.values()];
    due.clear();
    for (const callback of callbacks) callback();
  };
  return { timers, fire };
}

/** Puts the caret at the end of the first node of type `name` in the editor, as a click of the author does. */
function caretAtEnd(surface: HTMLElement, name: "paragraph" | "blockProposal") {
  const editor = (surface as HTMLElement & { editor: Editor }).editor;
  let end: number | undefined;
  editor.state.doc.forEach((node, offset) => {
    if (end === undefined && node.type.name === name) end = offset + 1 + node.content.size;
  });
  editor.commands.setTextSelection(end!);
  editor.view.focus();
}

it("keeps Enter at the end of a candidate and the typing after it in the candidate after a saved Chapter text edit",
  async () => {
    const server = await fakeServer();
    const { timers, fire } = manualTimers();
    const databases = new Set<IDBDatabase>();
    const open = IDBFactory.prototype.open;
    const openSpy = vi.spyOn(indexedDB, "open").mockImplementation((name, version) => {
      const request = open.call(indexedDB, name, version);
      request.addEventListener("success", () => trackDatabase(request.result, databases));
      return request;
    });
    document.body.innerHTML = '<main id="app"></main>';
    try {
      const loaded = await loadStoryOSWebState({ documentImpl: document,
        locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
        fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
      const view = mountStage1View(loaded.root, { ...loaded, writingTimers: timers });
      try {
        const root = loaded.root;
        const editor = () => manuscriptEditor(root, window);
        const candidate = () => root.querySelector(`.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"]`);
        const status = () => root.querySelector("[data-save-state]");
        await expect.poll(() => candidate()?.getAttribute("data-proposal-eligibility") === "eligible"
          && manuscriptIsEditable(editor())).toBe(true);

        caretAtEnd(editor(), "paragraph");
        await applyTrustedInput({ operation: "insert_text", text: "!" });
        // The save timer starts after the Journal append, so the test fires it until the save went out.
        await expect.poll(() => { fire(); return [status()?.getAttribute("data-save-state"),
          status()?.getAttribute("data-authoritative-revision-id")]; }).toEqual(["saved", SAVED_REVISION]);
        await expect.poll(() => candidate()?.getAttribute("data-proposal-eligibility")).toBe("eligible");

        caretAtEnd(editor(), "blockProposal");
        await applyTrustedInput({ operation: "enter" });
        await expect.poll(() => { fire(); return server.edits.length; }).toBe(2);
        await applyTrustedInput({ operation: "insert_text", text: "灯" });

        // The Server applies the first edit only with no Proposal head, because the edit does not name the target.
        const [first, second] = server.edits;
        expect({ blocks: [...editor().querySelectorAll(":scope > p")].map((block) => block.textContent),
          first: [first?.expected_proposal_head_revision_ids, first?.observed_ownership_partition],
          candidate: candidate()?.querySelector(".block-proposal-text")?.textContent,
          editable: manuscriptIsEditable(editor()), save: status()?.getAttribute("data-save-state"),
          failure: status()?.getAttribute("data-editor-failure"),
          target: (second?.proposal_target as { proposal_id?: string } | undefined)?.proposal_id,
          unexpected: server.unexpected })
          .toEqual({ blocks: ["雨夜。!", "Base"], first: [[], "authoritative"], candidate: `${PROPOSAL.candidate_text}\n灯`, editable: true, save: "saving",
            failure: "", target: PROPOSAL.proposal_id, unexpected: [] });
      } finally {
        view.unmount();
      }
    } finally {
      closeTrackedDatabases(databases);
      openSpy.mockRestore();
    }
  });
