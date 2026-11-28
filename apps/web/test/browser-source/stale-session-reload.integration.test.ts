import { afterEach, expect, it } from "vitest";

import type { GetChapterResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { freezeOneIntentSubmission, openEditorWorkspace, submitOnePendingAuthorEdit }
  from "../../src/editor-session.ts";
import type { EditorReadyState } from "../../src/editor-types.ts";
import { appendAuthorEdit } from "../../src/local-edit-journal.ts";
import { captureManuscriptChange } from "../../src/manuscript-doc.ts";
import { commitProtectedTransportSend } from "../../src/protected-transport-capsule.ts";
import {
  BLOCK, OWNER, PROJECT, REVISION, SESSION, chapterRevision, closeTrackedDatabases, createAppliedAuthorEditResponse,
  createBrowserScenario, deleteJournal, jsonResponse, requestHeaders, requestResult, requireEditorReady,
  requireRequestBody, trackDatabase,
} from "./scenario.ts";

// ADR 0107: another Editor Session can write, so the stored Editor Session of a tab can have an earlier base.
const LATER_REVISION = "018f0000-0000-7001-8000-000000000a41";
const NEW_SESSION = "018f0000-0000-7001-8000-000000000a42";
const NEW_SNAPSHOT = "018f0000-0000-7001-8000-000000000a43";
const MOVED_SNAPSHOT = "018f0000-0000-7001-8000-000000000a45";
const LATER_BODY = "Base, and the text of another tab";
const THIRD_REVISION = "018f0000-0000-7001-8000-000000000a48";
type Revision = { revision_id: string; body: string;
  blocks: { manuscript_block_id: string; block_kind: "paragraph" | "heading"; text: string }[] };
const REVISIONS: { base: Revision; later: Revision; created?: Revision } = { base: chapterRevision(REVISION, "Base"),
  later: chapterRevision(LATER_REVISION, LATER_BODY) };
const EDIT = { from: 4, to: 4, text: "!", resultingBody: "Base!" };
const [PROPOSAL, OPERATION, PROPOSAL_REVISION] = ["018f0000-0000-7001-8000-000000000a49",
  "018f0000-0000-7001-8000-000000000a4a", "018f0000-0000-7001-8000-000000000a4b"];
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
 * of the stored Session, a Challenge, one creation, whose new Session has the later Revision as its base (or `created`
 * at a later position), and, with `edit`, one Author Edit with that zero-authority result. After `moveBase("deletion")`,
 * the read gives the base that a deletion moved: a later Snapshot of the same Revision. After `moveBase("chapter")`, it
 * gives the base that a Set Current Chapter with the stored Session moved: a later Snapshot of the later Revision. It
 * records each request and throws on each other request.
 */
type Result = keyof typeof RESULTS;
/** The command that moved the Server base of the stored Session after the Journal stored its base. */
type Moved = false | "deletion" | "chapter";
// Each zero-authority result of an Author Edit on the base: its effect and the head of its receipt.
const RESULTS = {
  no_effect: { effect: { kind: "no_effect", reason: "content_unchanged" }, head: REVISION },
  conflicted: { effect: { kind: "conflicted", reason: "stale_authoritative_head",
    current_authoritative_revision_id: LATER_REVISION }, head: LATER_REVISION },
  proposal_head_present: { effect: { kind: "conflicted", reason: "proposal_head_present",
    current_authoritative_revision_id: REVISION }, head: REVISION },
} as const;

async function fakeServer(edit?: Result, { base, later, created: at } = REVISIONS) {
  const requests: string[] = [];
  const snapshot = async (revision: Revision) => ({ authoritative_head_revision_id: revision.revision_id,
    materialized_revision: revision, materialized_payload_digest: {
      ...scenario.session.base_snapshot.materialized_payload_digest, value_hex_lowercase: await sha256(revision.body) } });
  const first = { ...scenario.session, base_snapshot: { ...scenario.session.base_snapshot, ...await snapshot(base) } };
  let stored = first;
  const created = { ...scenario.session,
    editor_session: { ...scenario.session.editor_session, editor_session_id: NEW_SESSION },
    base_snapshot: { ...scenario.session.base_snapshot, snapshot_id: NEW_SNAPSHOT,
      project_activity_position: at === undefined ? "1" : "2", ...await snapshot(at ?? later) } };
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    requests.push(name);
    if (name === SESSION_READ) {
      return jsonResponse({ ...stored, schema_id: "storyos.query.editor-session.response.v2" });
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
      const { effect, head } = RESULTS[edit];
      return jsonResponse({ ...answer, effect, receipt: { ...answer.receipt, result: effect.kind, prior_heads: [head],
        resulting_heads: [head], proposal_revision_ids: [],
        authoritative_revision_ids: [], authoritative_commit_ids: [], author_action_sequence: null } });
    }
    throw new Error(`unexpected request: ${name}`);
  };
  const moveBase = async (by: Moved) => {
    stored = { ...first, base_snapshot: { ...first.base_snapshot, snapshot_id: MOVED_SNAPSHOT,
      project_activity_position: "1", ...by === "chapter" ? await snapshot(later) : {} } };
  };
  return { fetchImpl, requests, moveBase, revisions: { base, later } };
}

const chapterAt = (revision: Revision, position: string): GetChapterResponse => ({ ...scenario.chapter,
  project_activity_position: position, chapter: { ...scenario.chapter.chapter, current_revision: revision } });

async function openStored(server: Awaited<ReturnType<typeof fakeServer>>) {
  await deleteJournal(scenario.journalName);
  sessionStorage.setItem(ACTIVE_KEY, SESSION);
  const first = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: chapterAt(server.revisions.base, "0"), profile: scenario.profile, fetchImpl: server.fetchImpl });
  requireEditorReady(first);
  return first;
}

/** Opens the Chapter of the later Revision at a new load. */
async function load(server: Awaited<ReturnType<typeof fakeServer>>) {
  const state = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: chapterAt(server.revisions.later, "1"), profile: scenario.profile, fetchImpl: server.fetchImpl });
  if (state.kind === "editor-ready") trackDatabase(state.database, databases);
  return state;
}

async function submit(workspace: EditorReadyState, server: Awaited<ReturnType<typeof fakeServer>>) {
  await submitOnePendingAuthorEdit({ workspace, baseUrl: location.origin, fetchImpl: server.fetchImpl,
    cryptoImpl: crypto });
}

async function journalRows(store: string): Promise<unknown[]> {
  const database = await requestResult(indexedDB.open(scenario.journalName));
  try {
    return await requestResult(database.transaction(store).objectStore(store).getAll());
  } finally {
    database.close();
  }
}

type Leave = (workspace: EditorReadyState, server: Awaited<ReturnType<typeof fakeServer>>) => Promise<void>;

/**
 * Opens the stored Editor Session on the Chapter of its base, leaves the Journal work of `leave`, and proves that the
 * Author Edit of `edit` settled with that result. With `moved`, the Server then gives the base that a deletion moved.
 */
async function prepare(edit: Result | undefined, moved: Moved, leave: Leave,
  revisions = REVISIONS) {
  const server = await fakeServer(edit, revisions);
  const first = await openStored(server);
  await leave(first, server);
  first.database.close();
  const settlements = (await journalRows("submission_groups") as { settlement: { kind: string;
    effect?: { kind: string } } }[]).filter(({ settlement }) => settlement.kind !== "unsettled")
    .map(({ settlement }) => [settlement.kind, settlement.effect?.kind]);
  expect({ edits: server.requests.filter((request) => request === AUTHOR_EDIT).length, settlements })
    .toEqual(edit === undefined ? { edits: 0, settlements: [] }
      : { edits: 1, settlements: [["zero_authority_receipt_settled", RESULTS[edit].effect.kind]] });
  if (moved !== false) await server.moveBase(moved);
  server.requests.length = 0;
  return server;
}

// The partition of the stored Editor Session has no work that the Server does not have.
it.each([
  ["an empty Journal", undefined, false, async () => {}],
  ["an Author Edit with no effect", "no_effect", false, async (workspace: EditorReadyState,
    server: Awaited<ReturnType<typeof fakeServer>>) => {
    for (const edit of NO_CHANGE) await appendAuthorEdit(workspace, edit);
    await submit(workspace, server);
  }],
  ["an empty Journal, after a deletion moved its Server base", undefined, "deletion", async () => {}],
] as const)("opens a new Editor Session at a load when the stored Session has an earlier base and %s", async (_, edit,
  moved, leave) => {
  const server = await prepare(edit, moved, leave);

  const state = await load(server);

  requireEditorReady(state);
  expect({ session: state.session.editor_session.editor_session_id,
    revision: state.session.base_snapshot.authoritative_head_revision_id, stored: sessionStorage.getItem(ACTIVE_KEY),
    requests: server.requests }).toEqual({ session: NEW_SESSION, revision: LATER_REVISION, stored: NEW_SESSION,
    requests: [SESSION_READ, CHALLENGE, CREATION] });
});

// Each case leaves Author Edit text in the Journal of the stored Session that the Server does not have.
it.each([
  ["a record that is not settled", undefined, false, async (workspace: EditorReadyState) => {
    await appendAuthorEdit(workspace, EDIT);
  }],
  ["a record that is not settled, after a deletion moved its Server base", undefined, "deletion",
    async (workspace: EditorReadyState) => {
      await appendAuthorEdit(workspace, EDIT);
    }],
  // The page closed after Set Current Chapter moved the Server base and before the copy of the text (#1953).
  ["a record that is not settled, after Set Current Chapter moved its Server base to the current Revision", undefined,
    "chapter", async (workspace: EditorReadyState) => {
      await appendAuthorEdit(workspace, EDIT);
    }],
  ["a conflicted Author Edit", "conflicted", false, async (workspace: EditorReadyState,
    server: Awaited<ReturnType<typeof fakeServer>>) => {
    await appendAuthorEdit(workspace, EDIT);
    await submit(workspace, server);
  }],
] as const)("puts the text on the current Chapter in a new Editor Session at a load when its Journal has %s",
  async (_, edit, moved, leave) => {
    const server = await prepare(edit, moved, leave);
    const records = await journalRows("intents");

    const state = await load(server);

    requireEditorReady(state);
    // The later edit of the block wins. The new partition has it, and the Session changes after it is there.
    expect({ session: state.session.editor_session.editor_session_id, stored: sessionStorage.getItem(ACTIVE_KEY),
      requests: server.requests, text: state.openedProjection.body, save: state.openedProjection.save_state,
      records: (await journalRows("intents")).length - records.length }).toEqual({ session: NEW_SESSION,
      stored: NEW_SESSION, requests: [SESSION_READ, CHALLENGE, CREATION], text: "Base!", save: "saving", records: 1 });
  });

it("keeps a changed block that the current Chapter does not have as a paragraph at its place", async () => {
  const [first, second] = ["018f0000-0000-7001-8000-000000000a46", "018f0000-0000-7001-8000-000000000a47"];
  const revisions = { base: { revision_id: REVISION, body: "雨夜。\n渡口。", blocks: [
    { manuscript_block_id: first, block_kind: "paragraph" as const, text: "雨夜。" },
    { manuscript_block_id: second, block_kind: "heading" as const, text: "渡口。" }] },
  later: chapterRevision(LATER_REVISION, "雨夜，风起。", first) };
  const server = await prepare(undefined, false, async (workspace) => {
    const edit = captureManuscriptChange(revisions.base.blocks, [
      { manuscript_block_id: first, block_kind: "paragraph", text: "雨夜。" },
      { manuscript_block_id: second, block_kind: "heading", text: "渡口无人。" }]);
    await appendAuthorEdit(workspace, { ...edit!, inputOrigin: "typing", undoGroupId: UNDO_GROUP,
      createdAt: "2026-08-15T08:00:00.000Z" });
  }, revisions);

  const state = await load(server);

  requireEditorReady(state);
  // The steps of the catch-up go out as one Author Edit.
  const group = await freezeOneIntentSubmission(state);
  expect(group.ordered_coverage.length).toBe(state.openedProjection.unsettled_intent_count);
  const blocks = state.openedProjection.blocks;
  expect({ blocks: blocks.map(({ block_kind, text }) => [block_kind, text]), kept: blocks[0]?.manuscript_block_id,
    added: [first, second].includes(blocks[1]?.manuscript_block_id ?? first), requests: server.requests })
    .toEqual({ blocks: [["paragraph", "雨夜，风起。"], ["paragraph", "渡口无人。"]], kept: first, added: false,
      requests: [SESSION_READ, CHALLENGE, CREATION] });
});

it("puts the text on the base of the new Editor Session when another tab saves before the creation", async () => {
  const server = await prepare(undefined, false, async (workspace) => { await appendAuthorEdit(workspace, EDIT); },
    { ...REVISIONS, created: chapterRevision(THIRD_REVISION, "Base, and the text of two other tabs") });

  const state = await load(server);

  requireEditorReady(state);
  expect({ revision: state.session.base_snapshot.authoritative_head_revision_id, text: state.openedProjection.body,
    stored: sessionStorage.getItem(ACTIVE_KEY), requests: server.requests }).toEqual({ revision: THIRD_REVISION,
    text: "Base!", stored: NEW_SESSION, requests: [SESSION_READ, CHALLENGE, CREATION] });
});

it("keeps the stored Editor Session with the base of its Journal at a load when the text after Set Current Chapter "
  + "does not fit in one Author Edit", async () => {
  const id = (index: number) => `018f0000-0000-7001-8000-0000000b${index.toString(16).padStart(4, "0")}`;
  const block = (index: number, text: string) => ({ manuscript_block_id: id(index), block_kind: "paragraph" as const,
    text });
  // A, 100 blocks, and D. The tab changes A and pastes 140 blocks after it, then joins the 100 blocks into the last
  // pasted block. The catch-up needs one replacement with 100 joins, 140 splits, and its text: more primitives than
  // one Author Edit can have.
  const base = [block(0, "甲。"), ...Array.from({ length: 100 }, (_, index) => block(1 + index, `第${index}段。`)),
    block(101, "丁。")];
  const pasted = Array.from({ length: 140 }, (_, index) => block(0x200 + index, `新${index}。`));
  const revision = (revisionId: string) => ({ revision_id: revisionId, body: base.map(({ text }) => text).join("\n"),
    blocks: base });
  const server = await prepare(undefined, "chapter", async (workspace) => {
    const steps = [base, [block(0, "甲，雨夜。"), ...pasted, ...base.slice(1)],
      [block(0, "甲，雨夜。"), ...pasted.slice(0, -1), { ...pasted.at(-1)!, text: "新，渡口。" }, base.at(-1)!]];
    for (const [index, next] of steps.slice(1).entries()) {
      await appendAuthorEdit(workspace, { ...captureManuscriptChange(steps[index]!, next)!, inputOrigin: "selection_replacement",
        undoGroupId: UNDO_GROUP, createdAt: "2026-08-15T08:00:00.000Z" });
    }
  }, { base: revision(REVISION), later: revision(LATER_REVISION) });
  const bases = async () => (await journalRows("metadata") as { key: string; value: { snapshot_id?: string } }[])
    .filter(({ key }) => key.startsWith("active_base:")).map(({ value }) => value.snapshot_id);
  const [records, before] = [await journalRows("intents"), await bases()];

  const state = await load(server);

  expect({ kind: state.kind, stored: sessionStorage.getItem(ACTIVE_KEY), requests: server.requests,
    bases: await bases() }).toEqual({ kind: "editor-read-only-recovery", stored: SESSION, requests: [SESSION_READ],
    bases: before });
  if (state.kind === "editor-ready") state.database.close();
  expect(await journalRows("intents")).toEqual(records);
});

const submitted = (workspace: EditorReadyState, server: Awaited<ReturnType<typeof fakeServer>>) =>
  appendAuthorEdit(workspace, EDIT).then(() => submit(workspace, server));

// The capsule and its transport attempt commit before the send, so the group can have gone out, and its result is
// unknown (#1964). The load opens the stored Session on its own base, as a tab that stayed open does, so the text of the
// group is on the screen (#1972).
it("keeps the stored Editor Session and its input on the base of the Session at a load when its Journal has a group "
  + "that went out with no result", async () => {
  const server = await prepare(undefined, false, async (workspace: EditorReadyState) => {
    await appendAuthorEdit(workspace, EDIT);
    await commitProtectedTransportSend(workspace, await freezeOneIntentSubmission(workspace),
      { nonce: "a".repeat(64), expires_at: "2026-08-15T09:00:00.000Z" });
  });
  const records = await journalRows("intents");

  const state = await load(server);

  expect({ kind: state.kind, stored: sessionStorage.getItem(ACTIVE_KEY), requests: server.requests,
    base: state.kind === "editor-ready" ? state.session.base_snapshot.materialized_revision.revision_id : undefined })
    .toEqual({ kind: "editor-ready", stored: SESSION, requests: [SESSION_READ], base: REVISION });
  if (state.kind === "editor-ready") state.database.close();
  expect(await journalRows("intents")).toEqual(records);
});

// Each case leaves work in the Journal of the stored Session that the catch-up cannot put on the current Chapter.
it.each([
  ["an Author Edit that conflicted for another reason", "proposal_head_present", submitted],
  ["Proposal candidate input that no group revised", undefined, async (workspace: EditorReadyState) => {
    await appendAuthorEdit(workspace, { kind: "candidate_selection", target: { proposal_id: PROPOSAL,
      operation_id: OPERATION, revision_id: PROPOSAL_REVISION, manuscript_block_id: BLOCK },
    expectedProposalHeads: [PROPOSAL_REVISION], priorText: "Candidate", from: 9, to: 9, text: "!",
    resultingBody: "Candidate!" });
  }],
  ["an unresolved Withdrawal", undefined, async (workspace: EditorReadyState) => {
    await requestResult(workspace.database.transaction("metadata", "readwrite").objectStore("metadata").put({
      key: `withdraw:${workspace.partition.journal_partition_id}:${LATER_REVISION}:${NEW_SNAPSHOT}`,
      settlement: "unresolved" }));
  }],
] as const)("keeps the stored Editor Session and its input at a load when its Journal has %s", async (_, edit,
  leave) => {
  const server = await prepare(edit, false, leave);
  const records = await journalRows("intents");

  const state = await load(server);

  expect({ kind: state.kind, stored: sessionStorage.getItem(ACTIVE_KEY), requests: server.requests })
    .toEqual({ kind: "editor-read-only-recovery", stored: SESSION, requests: [SESSION_READ] });
  // The load does not remove or change the input of the stored Session.
  if (state.kind === "editor-ready") state.database.close();
  expect(await journalRows("intents")).toEqual(records);
});
