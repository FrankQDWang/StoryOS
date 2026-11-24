import { afterEach, beforeEach, expect, it } from "vitest";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import rejectionFixture from "../../../../generated/golden-wire/storyos-public-release-1/reject-proposal-operations.json";
import setCurrentChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-current-chapter.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import takeoverFixture from "../../../../generated/golden-wire/storyos-public-release-1/take-over-project-writer.json";
import { digestAcceptProposal, digestRejectProposalOperations }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, GetEditorSessionResponse, RejectProposalOperationsRequest, SnapshotDescriptor }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { openControlledProject } from "../../src/boot.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import {
  BLOCK, CHAPTER, EDITOR_SNAPSHOT, OWNER, PROJECT, REVISION, SESSION, chapterRevision, closeTrackedDatabases,
  createBrowserScenario, deleteJournal, emptyActivityStream, jsonResponse, trackDatabase,
} from "./scenario.ts";

// A Project opens, and opens again after a command, with fewer requests in series (#1875).
const P = `/api/v1/projects/${PROJECT}`;
const CHAPTER_B = setCurrentChapterFixture.project.open.current_chapter_id;
const REVISION_B = "018f0000-0000-7001-8000-000000000805";
const SWITCH_SNAPSHOT = "018f0000-0000-7001-8000-000000000831";
const TAKEOVER_SNAPSHOT = "018f0000-0000-7001-8000-000000000832";
const PRIOR_SESSION = "018f0000-0000-7001-8000-000000000833";
const SECOND_SESSION = "018f0000-0000-7001-8000-000000000836";
const ACCEPTANCE_SNAPSHOT = "018f0000-0000-7001-8000-000000000834";
const ACCEPTED_REVISION = "018f0000-0000-7001-8000-000000000806";
const ACCEPTANCE_COMMIT = "018f0000-0000-7001-8000-000000000835";
const EMPTY_BODY_DIGEST = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
// The SHA-256 of the candidate text of the Proposal, which an applied Acceptance makes the body of Chapter A.
const ACCEPTED_BODY_DIGEST = "c4cd3b16acbe6d2358948ccb2004fc1eec27d078096d06fed3fbaa9c16fa22e6";
const PROTOCOL = "GET /api/v1/protocol";
const LIST = "GET /api/v1/projects";
const PROJECT_READ = `GET ${P}`;
const chapterRead = (chapterId: string) => `GET ${P}/chapters/${chapterId}`;
const SESSION_READ = `GET ${P}/editor-sessions/${SESSION}`;
const SESSION_CREATE = `POST ${P}/editor-sessions`;
const CHALLENGE = `POST ${P}/anti-forgery-challenges`;
const TREE = `GET ${P}/manuscript/tree`;
const STATISTICS = `GET ${P}/manuscript/statistics`;
const SWITCH = `PUT ${P}/current-chapter`;
const TAKEOVER = `POST ${P}/editor-sessions/${SESSION}/takeovers`;
const PROPOSAL = { ...proposalFixture.proposal, chapter_id: CHAPTER, manuscript_block_id: BLOCK,
  base_authoritative_revision_id: REVISION, operations: proposalFixture.proposal.operations
    .map((operation) => ({ ...operation, manuscript_block_id: BLOCK })) };
const PROPOSAL_READ = `GET ${P}/proposals/${PROPOSAL.proposal_id}`;
const snapshotRead = (snapshotId: string) => `GET ${P}/snapshots/${snapshotId}`;
const openProposalsRead = (chapterId: string) => `GET ${P}/chapters/${chapterId}/open-proposals`;
const ASSISTANCE = `GET ${P}/assistance`;
const SKILLS = `GET ${P}/skills`;
const PANEL_READS = [TREE, `GET ${P}/activity`, STATISTICS, ASSISTANCE, SKILLS];
const UNAVAILABLE_SENTENCE = "写作助手当前不可用。你仍可以直接写作。";
/** The reads of the open of Chapter A and of its panels. */
const OPEN_READS = [PROTOCOL, LIST, PROJECT_READ, chapterRead(CHAPTER), openProposalsRead(CHAPTER), ...PANEL_READS];
const ACCEPTANCE = `POST ${P}/proposals/${PROPOSAL.proposal_id}/acceptances`;
const REJECTION = `POST ${P}/proposals/${PROPOSAL.proposal_id}/rejections`;

type Writer = GetEditorSessionResponse["writer"];

/** The command of each route template that a Challenge names. */
const CHALLENGE_ROUTES: Record<string, string> = {
  "POST /api/v1/projects/{project_id}/editor-sessions": SESSION_CREATE,
  "PUT /api/v1/projects/{project_id}/current-chapter": SWITCH,
  "POST /api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers": TAKEOVER,
  "POST /api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances": ACCEPTANCE,
  "POST /api/v1/projects/{project_id}/proposals/{proposal_id}/rejections": REJECTION,
};

/**
 * A fake Server of one Project with Chapters A (current) and B. It serves only the `reads` and the `commands` of the
 * test (the creation of an Editor Session too), and a Challenge for one of these commands. It records each other
 * request and throws. It records the start and the end of each request. A switch, a takeover, and an applied Acceptance
 * settle a new Snapshot, which the Editor Session base, the Chapter read, and the tree name. `acceptance` gives the
 * effect of an Acceptance. `takeoverAfterAcceptance` makes another tab take over the writer and switch to Chapter B
 * after the Acceptance settles: the Editor Session of this page keeps its base. A Rejection removes the suggestion from
 * the open suggestions. `assistant` gives the availability of the assistance read and serves the Skills; with no
 * `assistant`, both reads answer a 503 problem. As on the Server, only the first created Editor Session holds the
 * writer; a later creation gives a read-only Session. `holdAfter` holds the next request of each given name after the
 * trigger request ends, until `release`. `unavailable` answers the given requests with a 503 problem. `lost` applies
 * the given requests and then loses their response.
 */
function fakeServer({ reads, writer = { kind: "current_writer", writer_generation: "1" }, commands = [],
  proposals = false, acceptance = "refused", takeoverAfterAcceptance = false, assistant, unavailable = [], lost = [] }: {
  reads: string[]; writer?: Writer; commands?: string[]; proposals?: boolean; acceptance?: "refused" | "applied";
  takeoverAfterAcceptance?: boolean; assistant?: "available" | "unavailable"; unavailable?: string[];
  lost?: string[];
}) {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  const state = { current: CHAPTER, writer, snapshot: { id: EDITOR_SNAPSHOT, position: "0" }, accepted: false,
    rejected: false, created: 0, superseded: undefined as GetEditorSessionResponse | undefined };
  const project = () => ({ ...scenario.project.project, open: open() });
  const revisionA = () => state.accepted ? chapterRevision(ACCEPTED_REVISION, PROPOSAL.candidate_text)
    : chapterRevision(REVISION, "Base");
  const log = { started: [] as string[], unexpected: [] as string[], active: new Map<string, number>() };
  let armed: { trigger: string; names: string[] } | undefined;
  let holding = new Set<string>();
  const held: (() => void)[] = [];
  const open = () => ({ kind: "current_chapter" as const, current_chapter_id: state.current });
  const session = (): GetEditorSessionResponse => {
    if (state.superseded !== undefined) return state.superseded;
    const base = state.current === CHAPTER ? { ...scenario.session.base_snapshot,
      authoritative_head_revision_id: revisionA().revision_id, materialized_revision: revisionA(),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: state.accepted ? ACCEPTED_BODY_DIGEST
          : scenario.session.base_snapshot.materialized_payload_digest.value_hex_lowercase } }
      : { ...scenario.session.base_snapshot,
      chapter_id: CHAPTER_B, authoritative_head_revision_id: REVISION_B, target_refs: [`manuscript:${CHAPTER_B}`],
      materialized_revision: chapterRevision(REVISION_B, ""),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: EMPTY_BODY_DIGEST } };
    return { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1", writer: state.writer,
      base_snapshot: { ...base, snapshot_id: state.snapshot.id, project_activity_position: state.snapshot.position } };
  };
  const chapter = (chapterId: string) => chapterId === CHAPTER
    ? { ...scenario.chapter, project_activity_position: state.snapshot.position,
      chapter: { ...scenario.chapter.chapter, current_revision: revisionA() } }
    : { ...scenario.chapter, project_activity_position: state.snapshot.position,
      chapter: { chapter_id: CHAPTER_B, title: "Chapter B", current_revision: chapterRevision(REVISION_B, "") } };
  const canonical = (): SnapshotDescriptor => ({ snapshot_id: state.snapshot.id, project_scope: scope,
    snapshot_kind: "canonical", project_activity_position: state.snapshot.position, source_watermarks: {},
    projection_generations: {}, redaction_profile: "storyos.author.v1", schema_profile: "storyos.public.release.1",
    replay_generation: "2", created_at: "2026-08-20T04:00:00.000Z", expires_at: null });
  const settle = (snapshotId: string) => {
    state.snapshot = { id: snapshotId, position: String(Number(state.snapshot.position) + 1) };
  };
  const answer = async (name: string, init: RequestInit | undefined): Promise<Response> => {
    if (name !== CHALLENGE && !reads.includes(name) && !commands.includes(name)) {
      log.unexpected.push(name);
      throw new Error(`unexpected request: ${name}`);
    }
    if (unavailable.includes(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "service_unavailable", message: "Unavailable." }, 503);
    }
    if (name === PROTOCOL) return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (name === LIST) {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: "Project A", revision: "1", open: open() }] });
    }
    if (name === PROJECT_READ) return jsonResponse({ ...scenario.project, project: project() });
    if (name === chapterRead(CHAPTER)) return jsonResponse(chapter(CHAPTER));
    if (name === chapterRead(CHAPTER_B)) return jsonResponse(chapter(CHAPTER_B));
    if (name === CHALLENGE) {
      const route = JSON.parse(String(init?.body)) as { method?: string; route_template?: string };
      const command = CHALLENGE_ROUTES[`${route.method} ${route.route_template}`];
      if (command === undefined || !commands.includes(command)) {
        log.unexpected.push(`${name} for ${route.method} ${route.route_template}`);
        throw new Error(`unexpected Challenge: ${route.method} ${route.route_template}`);
      }
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (name === SESSION_CREATE) {
      state.created += 1;
      return jsonResponse(state.created === 1 ? { ...session(), schema_id: scenario.session.schema_id }
        : { ...session(), schema_id: scenario.session.schema_id,
          editor_session: { ...scenario.session.editor_session, editor_session_id: SECOND_SESSION },
          writer: { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" } });
    }
    if (name === SESSION_READ) return jsonResponse(session());
    if (name === TREE) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope, tree_revision: "3",
        snapshot: canonical(), volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000815", title: "Volume A",
          order: "1", chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" },
            { chapter_id: CHAPTER_B, title: "Chapter B", order: "2" }] }] });
    }
    if (name === snapshotRead(state.snapshot.id)) {
      return jsonResponse({ schema_id: "storyos.query.snapshot.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000015", project_scope: scope, snapshot: canonical() });
    }
    if (name === `GET ${P}/activity`) return emptyActivityStream();
    if (name === ASSISTANCE && assistant !== undefined) {
      return jsonResponse({ ...assistanceFixture, project_scope: scope,
        assistance: { ...assistanceFixture.assistance, availability: assistant } });
    }
    if (name === SKILLS && assistant !== undefined) return jsonResponse({ ...skillsFixture, project_scope: scope });
    // A test that is not about these panels shows that their read is unavailable.
    if ([STATISTICS, ASSISTANCE, SKILLS].includes(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "service_unavailable", message: "Unavailable." }, 503);
    }
    if (name === `GET ${P}/chapters/${state.current}/open-proposals`) {
      return jsonResponse({ schema_id: "storyos.query.chapter-open-proposals.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000c71", project_scope: scope, chapter_id: state.current,
        completeness: "complete", proposals: (proposals && !state.accepted && !state.rejected ? [PROPOSAL] : []).map(({ proposal_id, revision_id, kind,
          source }) => ({ proposal_id, revision_id, kind, source })) });
    }
    if (name === PROPOSAL_READ) {
      const resolution = state.accepted ? "applied" : state.rejected ? "rejected" : undefined;
      return jsonResponse({ ...proposalFixture, project_scope: scope, proposal: resolution === undefined ? PROPOSAL
        : { ...PROPOSAL, operation_resolution: resolution, operations: PROPOSAL.operations
          .map((operation) => ({ ...operation, resolution })) } });
    }
    if (name === SWITCH) {
      state.current = (JSON.parse(String(init?.body)) as { set_current_chapter_input: { chapter_id: string } })
        .set_current_chapter_input.chapter_id;
      settle(SWITCH_SNAPSHOT);
      return jsonResponse({ ...setCurrentChapterFixture, project: project(),
        effect: { ...setCurrentChapterFixture.effect, base_snapshot_id: SWITCH_SNAPSHOT,
          current_chapter_id: state.current, project_activity_position: state.snapshot.position },
        receipt: { ...setCurrentChapterFixture.receipt, expected_heads: [REVISION_B], prior_heads: [REVISION_B],
          resulting_heads: [REVISION_B] } });
    }
    if (name === TAKEOVER) {
      state.writer = { kind: "current_writer", writer_generation: "2" };
      settle(TAKEOVER_SNAPSHOT);
      return jsonResponse({ ...takeoverFixture,
        receipt: { ...takeoverFixture.receipt, expected_heads: [REVISION], prior_heads: [REVISION],
          resulting_heads: [REVISION] },
        result: { ...takeoverFixture.result, prior_editor_session_id: PRIOR_SESSION,
          resulting_editor_session_id: SESSION, resulting_heads: [REVISION], resulting_snapshot_id: TAKEOVER_SNAPSHOT,
          resulting_snapshot_activity_position: state.snapshot.position } });
    }
    if (name === ACCEPTANCE) {
      // A refused Acceptance changes no head, so the Server keeps its Snapshot. An applied one settles a Snapshot.
      const request = JSON.parse(String(init?.body)) as AcceptProposalRequest;
      const input = request.accept_proposal_input;
      const applied = acceptance === "applied";
      if (applied) {
        state.accepted = true;
        settle(ACCEPTANCE_SNAPSHOT);
      }
      const settled = project();
      if (takeoverAfterAcceptance) {
        // The Server keeps the base of the Editor Session of this page, and the Session is read-only now.
        state.superseded = { ...session(), writer: { kind: "read_only", observed_writer_generation: "2",
          reason: "superseded_by_takeover" } };
        state.current = CHAPTER_B;
        settle(TAKEOVER_SNAPSHOT);
      }
      return jsonResponse({ schema_id: "storyos.command.accept-proposal.response.v1",
        correlation_id: input.correlation_id, project_scope: scope, command_id: "018f0000-0000-7001-8000-000000000e05",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000e06",
        receipt: { receipt_id: "018f0000-0000-7001-8000-000000000e07", project_scope: scope,
          command_digest: await digestAcceptProposal(request, crypto),
          idempotency_key: new Headers(init?.headers).get("idempotency-key") ?? "",
          author_command_admission_id: "018f0000-0000-7001-8000-000000000e06", proposal_id: PROPOSAL.proposal_id,
          proposal_revision_id: input.proposal_revision_id, validation_receipt_id: input.validation_receipt_id,
          selected_operation_ids: input.selected_operation_ids,
          prior_authoritative_revision_ids: [input.expected_authoritative_revision_id],
          resulting_authoritative_revision_ids: [applied ? ACCEPTED_REVISION : input.expected_authoritative_revision_id],
          authoritative_commit_ids: applied ? [ACCEPTANCE_COMMIT] : [], condition_refs: [],
          result: applied ? "applied" : "refused", created_at: "2026-10-18T08:00:00.000Z" },
        project: settled,
        effect: applied ? { kind: "applied", author_action_sequence: "1", authoritative_commit_id: ACCEPTANCE_COMMIT,
          authoritative_revision: revisionA(), project_activity_position: state.snapshot.position,
          target_resolution: "exact_base" } : { kind: "refused", reason: "not_eligible" } });
    }
    const request = JSON.parse(String(init?.body)) as RejectProposalOperationsRequest;
    const input = request.reject_proposal_operations_input;
    state.rejected = true;
    return jsonResponse({ ...rejectionFixture, correlation_id: input.correlation_id, project_scope: scope,
      project: project(),
      effect: { ...rejectionFixture.effect, operation_ids: input.selected_pending_operation_ids },
      receipt: { ...rejectionFixture.receipt, project_scope: scope,
        command_digest: await digestRejectProposalOperations(request, crypto),
        idempotency_key: new Headers(init?.headers).get("idempotency-key") ?? "", proposal_id: PROPOSAL.proposal_id,
        proposal_revision_id: input.proposal_revision_id,
        selected_pending_operation_ids: input.selected_pending_operation_ids,
        expected_target_revisions: input.expected_target_revisions } });
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    log.started.push(name);
    log.active.set(name, (log.active.get(name) ?? 0) + 1);
    try {
      if (holding.delete(name)) await new Promise<void>((resolve) => { held.push(resolve); });
      const response = await answer(name, init);
      if (lost.includes(name)) throw new TypeError("Failed to fetch");
      return response;
    } finally {
      log.active.set(name, log.active.get(name)! - 1);
      if (armed?.trigger === name) {
        holding = new Set(armed.names);
        armed = undefined;
      }
    }
  };
  return {
    scenario, log, fetchImpl,
    holdAfter(trigger: string, names: string[]) { armed = { trigger, names }; },
    hold(names: string[]) { holding = new Set(names); },
    release() { for (const resolve of held.splice(0)) resolve(); },
    inFlight: () => [...log.active].filter(([, count]) => count > 0).map(([name]) => name).sort(),
    count: (name: string) => log.started.filter((entry) => entry === name).length,
  };
}

const openDatabases = new Set<IDBDatabase>();
const unmounts: (() => void)[] = [];
// The view opens the Journal again at each reopen, so the test closes each connection before it deletes the Journal.
const openJournal = indexedDB.open.bind(indexedDB);

beforeEach(() => {
  Object.defineProperty(indexedDB, "open", { configurable: true, value: (name: string, version?: number) => {
    const request = openJournal(name, version);
    request.addEventListener("success", () => { trackDatabase(request.result, openDatabases); });
    return request;
  } });
});

afterEach(async () => {
  for (const unmount of unmounts.splice(0)) unmount();
  Reflect.deleteProperty(indexedDB, "open");
  closeTrackedDatabases(openDatabases);
  document.body.replaceChildren();
  sessionStorage.clear();
  await deleteJournal(createBrowserScenario().journalName);
});

async function openWorkspace(server: ReturnType<typeof fakeServer>) {
  await deleteJournal(server.scenario.journalName);
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const view = mountStage1View(loaded.root, loaded);
  unmounts.push(() => view.unmount());
  // A writable workspace shows the saved state. A read-only recovery shows "needs_attention".
  const shown = () => ({ title: loaded.root.querySelector(".editor-panel h2")?.textContent,
    save: loaded.root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
    failure: loaded.root.querySelector("[data-editor-failure]")?.getAttribute("data-editor-failure") });
  await expect.poll(() => shown().title).toBe("Chapter A");
  return { root: loaded.root, shown };
}

/** The reads of the open that a reopen after a command sends again (the Project and the list), and the protocol read. */
const openReads = (requests: string[]) => requests.filter((name) => [PROTOCOL, LIST, PROJECT_READ].includes(name));

/**
 * Proves the two steps of a reopen after `command`: the Project read and the list read in flight together, then the
 * Chapter read and the Editor Session read in flight together. It releases each step.
 */
async function expectReopenSteps(server: ReturnType<typeof fakeServer>, command: string, chapterId: string) {
  server.holdAfter(command, [PROJECT_READ, LIST]);
  return async () => {
    await expect.poll(server.inFlight).toEqual([LIST, PROJECT_READ].sort());
    server.holdAfter(LIST, [chapterRead(chapterId), SESSION_READ]);
    server.release();
    await expect.poll(server.inFlight).toEqual([chapterRead(chapterId), SESSION_READ].sort());
    server.release();
  };
}

it.each([
  { tab: "with its stored Editor Session", stored: true, session: SESSION_READ },
  { tab: "in a new tab", stored: false, session: SESSION_CREATE },
])("opens a Project $tab with the independent reads in flight at the same time", async ({ stored, session }) => {
  const server = fakeServer({ reads: [PROTOCOL, LIST, PROJECT_READ, chapterRead(CHAPTER), ...(stored ? [SESSION_READ] : [])],
    commands: stored ? [] : [SESSION_CREATE] });
  await deleteJournal(server.scenario.journalName);
  if (stored) sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  server.hold([PROTOCOL, PROJECT_READ, LIST]);
  const opening = openControlledProject({ baseUrl: location.origin, projectId: PROJECT,
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  await expect.poll(server.inFlight).toEqual([LIST, PROJECT_READ, PROTOCOL].sort());
  // In a new tab the Challenge ends first, and the creation of the Session is in flight with the Chapter read.
  server.hold([chapterRead(CHAPTER), session]);
  server.release();
  await expect.poll(server.inFlight).toEqual([chapterRead(CHAPTER), session].sort());
  server.release();
  const opened = await opening;
  expect({ kind: opened.kind, editor: opened.kind === "project-ready" ? opened.editor.kind : undefined,
    unexpected: server.log.unexpected }).toEqual({ kind: "project-ready", editor: "editor-ready", unexpected: [] });
});

it("reopens after a Chapter switch with the Project read and the list read, then the Chapter and the Editor Session",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, chapterRead(CHAPTER_B), openProposalsRead(CHAPTER_B),
      SESSION_READ], commands: [SESSION_CREATE, SWITCH] });
    const workspace = await openWorkspace(server);
    await expect.poll(() => workspace.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
    const before = server.log.started.length;
    const steps = await expectReopenSteps(server, SWITCH, CHAPTER_B);

    workspace.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();
    await steps();

    await expect.poll(workspace.shown).toEqual({ title: "Chapter B", save: "saved", failure: "" });
    const later = server.log.started.slice(server.log.started.indexOf(SWITCH, before));
    expect({ open: openReads(later).sort(), unexpected: server.log.unexpected })
      .toEqual({ open: [LIST, PROJECT_READ].sort(), unexpected: [] });
  });

it.each([
  { decision: "a refused acceptance", button: "button[data-proposal-accept]", command: ACCEPTANCE,
    message: "此次接受已被拒绝，候选文字仍保留。" },
])("reopens after $decision with the Project read and the list read, then the Chapter and the Editor Session",
  async ({ button, command, message }) => {
    const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ],
      commands: [SESSION_CREATE, command], proposals: true });
    const workspace = await openWorkspace(server);
    const card = () => workspace.root.querySelector(`.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"]`);
    const control = () => card()?.querySelector<HTMLButtonElement>(button);
    await expect.poll(() => card()?.getAttribute("data-proposal-eligibility")).toBe("eligible");
    await expect.poll(() => control()?.disabled).toBe(false);
    const before = server.log.started.length;
    const steps = await expectReopenSteps(server, command, CHAPTER);

    control()!.click();
    await steps();

    // The refused Acceptance keeps its card and shows its status.
    const status = () => workspace.root.querySelector(`[data-proposal-decision="${PROPOSAL.proposal_id}"]`)?.textContent;
    await expect.poll(() => ({ ...workspace.shown(), status: status() }))
      .toEqual({ title: "Chapter A", save: "saved", failure: "", status: message });
    const later = server.log.started.slice(server.log.started.indexOf(command, before));
    expect({ decisions: server.count(command), open: openReads(later).sort(), unexpected: server.log.unexpected })
      .toEqual({ decisions: 1, open: [LIST, PROJECT_READ].sort(), unexpected: [] });
  });

it("reopens after an applied acceptance with the Project read and the list read, then the Chapter and the Editor Session",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ],
      commands: [SESSION_CREATE, ACCEPTANCE], proposals: true, acceptance: "applied" });
    const workspace = await openWorkspace(server);
    const control = () => workspace.root.querySelector<HTMLButtonElement>(
      `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-accept]`);
    await expect.poll(() => control()?.disabled).toBe(false);
    const before = server.log.started.length;
    const steps = await expectReopenSteps(server, ACCEPTANCE, CHAPTER);

    control()!.click();
    await steps();

    // The reads from the command to the new view: the poll takes the record when the new revision shows.
    const revision = () => workspace.root.querySelector("[data-authoritative-revision-id]")
      ?.getAttribute("data-authoritative-revision-id");
    let untilView: string[] = [];
    await expect.poll(() => {
      untilView = server.log.started.slice(server.log.started.indexOf(ACCEPTANCE, before));
      return { ...workspace.shown(), revision: revision() };
    }).toEqual({ title: "Chapter A", save: "saved", failure: "", revision: ACCEPTED_REVISION });
    expect({ decisions: server.count(ACCEPTANCE), open: openReads(untilView).sort(), unexpected: server.log.unexpected })
      .toEqual({ decisions: 1, open: [LIST, PROJECT_READ].sort(), unexpected: [] });
  });

it("reopens after a takeover with the Project read and the list read, then the Chapter and the Editor Session",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT)],
      writer: { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" },
      commands: [SESSION_CREATE, TAKEOVER] });
    const workspace = await openWorkspace(server);
    const takeOver = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
    await expect.poll(takeOver).not.toBeNull();
    const before = server.log.started.length;
    const steps = await expectReopenSteps(server, TAKEOVER, CHAPTER);

    takeOver()!.click();
    await steps();

    await expect.poll(() => ({ ...workspace.shown(), takeOver: takeOver() }))
      .toEqual({ title: "Chapter A", save: "saved", failure: "", takeOver: null });
    const later = server.log.started.slice(server.log.started.indexOf(TAKEOVER, before));
    // The takeover check of the Journal keeps its three reads in their order, and the view uses its tree.
    const reads = later.filter((name) => ![CHALLENGE, STATISTICS, `GET ${P}/activity`, `GET ${P}/assistance`,
      `GET ${P}/skills`, openProposalsRead(CHAPTER)].includes(name));
    expect({ reads: [reads[0], reads.slice(1, 3).sort(), reads.slice(3, 5).sort(), ...reads.slice(5)],
      unexpected: server.log.unexpected }).toEqual({
      reads: [TAKEOVER, [LIST, PROJECT_READ].sort(), [chapterRead(CHAPTER), SESSION_READ].sort(), TREE,
        snapshotRead(TAKEOVER_SNAPSHOT), SESSION_READ],
      unexpected: [],
    });
  });

it("opens the Current Chapter of the Project read after another tab took over the writer and switched the Chapter",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ, chapterRead(CHAPTER_B),
      openProposalsRead(CHAPTER_B)], commands: [SESSION_CREATE, ACCEPTANCE], proposals: true,
      takeoverAfterAcceptance: true });
    const workspace = await openWorkspace(server);
    const control = () => workspace.root.querySelector<HTMLButtonElement>(
      `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-accept]`);
    await expect.poll(() => control()?.disabled).toBe(false);
    const before = server.log.started.length;

    control()!.click();

    // The response of the refused Acceptance names Chapter A, but the Project read names Chapter B.
    await expect.poll(() => workspace.shown().title).toBe("Chapter B");
    const later = server.log.started.slice(server.log.started.indexOf(ACCEPTANCE, before));
    expect({ open: openReads(later).sort(), chapters: later.filter((name) => name.startsWith(`GET ${P}/chapters/`)
      && !name.endsWith("/open-proposals")), unexpected: server.log.unexpected }).toEqual({
      open: [LIST, PROJECT_READ].sort(), chapters: [chapterRead(CHAPTER_B)], unexpected: [] });
  });

it("reads the list of Projects, the tree, and the statistics one time for one open", async () => {
  const server = fakeServer({ reads: OPEN_READS, commands: [SESSION_CREATE] });
  server.hold([TREE]);
  const workspace = await openWorkspace(server);
  await expect.poll(server.inFlight).toContain(TREE);
  // The statistics wait for the tree revision, and the view uses the list of the open.
  expect({ list: server.count(LIST), statistics: server.count(STATISTICS) }).toEqual({ list: 1, statistics: 0 });
  server.release();

  await expect.poll(() => workspace.root.querySelector("[data-manuscript-statistics]")).not.toBeNull();
  expect({ ...workspace.shown(), list: server.count(LIST), tree: server.count(TREE),
    statistics: server.count(STATISTICS), unexpected: server.log.unexpected }).toEqual({
    title: "Chapter A", save: "saved", failure: "", list: 1, tree: 1, statistics: 1, unexpected: [] });
});

it("reads the tree one time for one open with a writer generation above 1", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(EDITOR_SNAPSHOT)],
    writer: { kind: "current_writer", writer_generation: "2" } });
  sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  const workspace = await openWorkspace(server);

  await expect.poll(() => server.count(STATISTICS)).toBe(1);
  await expect.poll(() => workspace.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
  expect({ ...workspace.shown(), list: server.count(LIST), tree: server.count(TREE),
    unexpected: server.log.unexpected }).toEqual({
    title: "Chapter A", save: "saved", failure: "", list: 1, tree: 1, unexpected: [] });
});

it("reads the statistics one time when the tree read fails", async () => {
  const server = fakeServer({ reads: OPEN_READS, commands: [SESSION_CREATE], unavailable: [TREE] });
  await openWorkspace(server);

  // The tree read tries three times, with waits of 0.5 s and 1 s.
  await expect.poll(() => server.count(STATISTICS), { timeout: 5_000 }).toBe(1);
  expect({ tree: server.count(TREE), unexpected: server.log.unexpected }).toEqual({ tree: 3, unexpected: [] });
});

it("opens the Project with each check of the first open after a decision that did not settle", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ],
    commands: [SESSION_CREATE, REJECTION], proposals: true, unavailable: [REJECTION] });
  const workspace = await openWorkspace(server);
  const card = () => workspace.root.querySelector(`.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"]`);
  const control = () => card()?.querySelector<HTMLButtonElement>("button[data-proposal-reject]");
  await expect.poll(() => control()?.disabled).toBe(false);
  const before = server.log.started.length;

  control()!.click();

  await expect.poll(() => openReads(server.log.started.slice(before)).sort())
    .toEqual([PROTOCOL, PROJECT_READ, LIST].sort());
  expect(server.log.unexpected).toEqual([]);
});

// The workspace stays on the screen through a command (#1876).

/** The assistant panel, its message box, and the Chapters of the contents list. */
function workspaceParts(root: HTMLElement) {
  return {
    panel: () => root.querySelector<HTMLElement>("[data-assistant-availability]"),
    message: () => root.querySelector<HTMLTextAreaElement>('textarea[name="assistant-message"]'),
    chapters: () => root.querySelectorAll(".tree-panel button[data-chapter-id]").length,
    sentence: () => root.textContent?.includes(UNAVAILABLE_SENTENCE) ?? false,
  };
}

/** Types `text` into the message box, as the author does. */
function typeMessage(box: HTMLTextAreaElement, text: string) {
  Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")!.set!.call(box, text);
  box.dispatchEvent(new Event("input", { bubbles: true }));
}

/**
 * Records each change of the page that empties the contents list or shows the sentence of an unavailable assistant.
 * The observer sees each step of the page, so a short empty list between two views is also a record.
 */
function watchWorkspace(root: HTMLElement) {
  const parts = workspaceParts(root);
  const gaps: { chapters: number; sentence: boolean }[] = [];
  const observer = new MutationObserver(() => {
    const seen = { chapters: parts.chapters(), sentence: parts.sentence() };
    if (seen.chapters === 0 || seen.sentence) gaps.push(seen);
  });
  observer.observe(root, { subtree: true, childList: true, characterData: true, attributes: true });
  return { gaps, stop: () => observer.disconnect() };
}

it.each([
  { command: "a Chapter switch", name: SWITCH, chapter: CHAPTER_B, revision: REVISION_B },
  { command: "an applied acceptance", name: ACCEPTANCE, chapter: CHAPTER, revision: ACCEPTED_REVISION },
  { command: "a rejection", name: REJECTION, chapter: CHAPTER, revision: REVISION },
  { command: "a takeover", name: TAKEOVER, chapter: CHAPTER, revision: REVISION },
])("keeps the contents list and the assistant panel on the screen through $command", async ({ name, chapter,
  revision }) => {
  const takeover = name === TAKEOVER;
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, PROPOSAL_READ, chapterRead(CHAPTER_B),
    openProposalsRead(CHAPTER_B), snapshotRead(TAKEOVER_SNAPSHOT)], commands: [SESSION_CREATE, name],
  proposals: name === ACCEPTANCE || name === REJECTION, acceptance: "applied", assistant: "available",
  ...(takeover ? { writer: { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" } }
    : {}) });
  const workspace = await openWorkspace(server);
  const parts = workspaceParts(workspace.root);
  await expect.poll(() => ({ availability: parts.panel()?.getAttribute("data-assistant-availability"),
    chapters: parts.chapters() })).toEqual({ availability: "available", chapters: 2 });
  const control = () => workspace.root.querySelector<HTMLButtonElement>(name === SWITCH
    ? `button[data-chapter-id="${CHAPTER_B}"]` : takeover ? "button[data-take-over-writer]"
    : `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[${name === ACCEPTANCE
      ? "data-proposal-accept" : "data-proposal-reject"}]`);
  await expect.poll(() => control()?.disabled).toBe(false);
  const panel = parts.panel();
  typeMessage(parts.message()!, "Keep the narrator voice.");
  const watch = watchWorkspace(workspace.root);
  // The fake holds each step of the reopen, so the observer sees the page between the steps.
  const steps = name === REJECTION ? async () => {} : await expectReopenSteps(server, name, chapter);

  control()!.click();
  await steps();

  const shownRevision = () => workspace.root.querySelector("[data-authoritative-revision-id]")
    ?.getAttribute("data-authoritative-revision-id");
  await expect.poll(() => ({ ...workspace.shown(), revision: shownRevision(),
    card: workspace.root.querySelector(".block-proposal"),
    takeOver: workspace.root.querySelector("button[data-take-over-writer]") })).toEqual({
    title: chapter === CHAPTER ? "Chapter A" : "Chapter B", save: "saved", failure: "", revision, card: null,
    takeOver: null });
  await expect.poll(server.inFlight).toEqual([]);
  watch.stop();

  expect({ gaps: watch.gaps, panel: parts.panel() === panel, message: parts.message()?.value,
    assistance: server.count(ASSISTANCE), skills: server.count(SKILLS), unexpected: server.log.unexpected }).toEqual({
    gaps: [], panel: true, message: "Keep the narrator voice.", assistance: 1, skills: 1, unexpected: [] });
});

it.each([
  { answer: "unavailable" as const, read: "the answer unavailable" },
  { answer: undefined, read: "a failed read" },
])("shows the sentence of an unavailable assistant only after $read", async ({ answer }) => {
  const server = fakeServer({ reads: OPEN_READS, commands: [SESSION_CREATE], ...(answer ? { assistant: answer } : {}) });
  server.hold([ASSISTANCE]);
  const workspace = await openWorkspace(server);
  const parts = workspaceParts(workspace.root);
  await expect.poll(server.inFlight).toEqual([ASSISTANCE]);

  const before = parts.sentence();
  server.release();

  await expect.poll(parts.sentence).toBe(true);
  expect({ before, unexpected: server.log.unexpected }).toEqual({ before: false, unexpected: [] });
});

it("shows that the takeover runs, and sends one takeover for two clicks", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT)],
    writer: { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" },
    commands: [SESSION_CREATE, TAKEOVER] });
  const workspace = await openWorkspace(server);
  const button = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
  await expect.poll(() => button()?.disabled).toBe(false);
  server.hold([TAKEOVER]);

  button()!.click();
  await expect.poll(() => ({ disabled: button()?.disabled, text: button()?.textContent }))
    .toEqual({ disabled: true, text: "正在接管写作…" });
  await expect.poll(server.inFlight).toEqual([TAKEOVER]);
  button()!.click();
  expect(server.count(TAKEOVER)).toBe(1);
  server.release();

  await expect.poll(() => ({ ...workspace.shown(), button: button() }))
    .toEqual({ title: "Chapter A", save: "saved", failure: "", button: null });
  expect({ takeovers: server.count(TAKEOVER), unexpected: server.log.unexpected })
    .toEqual({ takeovers: 1, unexpected: [] });
});

it("shows the sentence of a refused takeover and makes the button usable again", async () => {
  const server = fakeServer({ reads: OPEN_READS,
    writer: { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" },
    commands: [SESSION_CREATE, TAKEOVER], unavailable: [TAKEOVER] });
  const workspace = await openWorkspace(server);
  const button = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
  await expect.poll(() => button()?.disabled).toBe(false);

  button()!.click();

  await expect.poll(() => ({ alert: workspace.root.querySelector('.editor-panel [role="alert"]')?.textContent,
    disabled: button()?.disabled, text: button()?.textContent }))
    .toEqual({ alert: "无法接管写作。", disabled: false, text: "接管写作" });
  expect(server.log.unexpected).toEqual([]);
});

it("reads only the suggestions after a rejection", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ], commands: [SESSION_CREATE, REJECTION],
    proposals: true });
  const workspace = await openWorkspace(server);
  const card = () => workspace.root.querySelector(`.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"]`);
  const reject = () => card()?.querySelector<HTMLButtonElement>("button[data-proposal-reject]");
  await expect.poll(() => reject()?.disabled).toBe(false);
  const before = server.log.started.length;

  reject()!.click();

  await expect.poll(card).toBeNull();
  await expect.poll(server.inFlight).toEqual([]);
  const later = server.log.started.slice(server.log.started.indexOf(REJECTION, before) + 1);
  // The Chapter text does not change, so the page reads the open suggestions and the Proposal, and no Project, list,
  // Chapter, Editor Session, or tree.
  expect({ ...workspace.shown(), later: [...new Set(later.filter((entry) => entry !== CHALLENGE
    && entry !== STATISTICS))].sort(), unexpected: server.log.unexpected }).toEqual({ title: "Chapter A",
    save: "saved", failure: "", later: [openProposalsRead(CHAPTER), PROPOSAL_READ].sort(), unexpected: [] });
});

/**
 * Waits until the new view read the Proposal and no request is in flight for 0.5 s. The view reads the Proposal after
 * the Editor Session read of the open, and a second open would start after that Proposal read.
 */
async function settledAfterProposalRead(server: ReturnType<typeof fakeServer>, before: number) {
  const opened = server.log.started.indexOf(SESSION_READ, before);
  await expect.poll(() => server.log.started.indexOf(PROPOSAL_READ, opened)).toBeGreaterThan(opened);
  await expect.poll(server.inFlight).toEqual([]);
  await new Promise((resolve) => { setTimeout(resolve, 500); });
  await expect.poll(server.inFlight).toEqual([]);
}

it("opens the Project again one time after an applied acceptance", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ], commands: [SESSION_CREATE, ACCEPTANCE],
    proposals: true, acceptance: "applied" });
  const workspace = await openWorkspace(server);
  const accept = () => workspace.root.querySelector<HTMLButtonElement>(
    `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-accept]`);
  await expect.poll(() => accept()?.disabled).toBe(false);
  const before = server.log.started.length;
  const steps = await expectReopenSteps(server, ACCEPTANCE, CHAPTER);

  accept()!.click();
  await steps();

  const revision = () => workspace.root.querySelector("[data-authoritative-revision-id]")
    ?.getAttribute("data-authoritative-revision-id");
  await expect.poll(revision).toBe(ACCEPTED_REVISION);
  await settledAfterProposalRead(server, before);
  const later = server.log.started.slice(before);
  expect({ open: openReads(later).sort(), revision: revision(), unexpected: server.log.unexpected })
    .toEqual({ open: [LIST, PROJECT_READ].sort(), revision: ACCEPTED_REVISION, unexpected: [] });
});

it("opens the Project one time after an applied acceptance whose response was lost", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ], commands: [SESSION_CREATE, ACCEPTANCE],
    proposals: true, acceptance: "applied", lost: [ACCEPTANCE] });
  const workspace = await openWorkspace(server);
  const accept = () => workspace.root.querySelector<HTMLButtonElement>(
    `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-accept]`);
  await expect.poll(() => accept()?.disabled).toBe(false);
  const before = server.log.started.length;

  accept()!.click();

  // The page cannot know the result, so it opens the Project with each check of the first open.
  const revision = () => workspace.root.querySelector("[data-authoritative-revision-id]")
    ?.getAttribute("data-authoritative-revision-id");
  await expect.poll(revision).toBe(ACCEPTED_REVISION);
  await settledAfterProposalRead(server, before);
  expect({ open: openReads(server.log.started.slice(before)).sort(), revision: revision(),
    unexpected: server.log.unexpected }).toEqual({ open: [PROTOCOL, LIST, PROJECT_READ].sort(),
    revision: ACCEPTED_REVISION, unexpected: [] });
});

it("opens the created Editor Session again after an open whose Chapter read failed", async () => {
  const unavailable = [chapterRead(CHAPTER)];
  const server = fakeServer({ reads: [PROTOCOL, LIST, PROJECT_READ, chapterRead(CHAPTER), SESSION_READ],
    commands: [SESSION_CREATE], unavailable });
  await deleteJournal(server.scenario.journalName);
  const open = () => openControlledProject({ baseUrl: location.origin, projectId: PROJECT,
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const failed = await open();
  await expect.poll(() => ({ created: server.count(SESSION_CREATE), inFlight: server.inFlight() }))
    .toEqual({ created: 1, inFlight: [] });
  unavailable.length = 0;

  const opened = await open();

  // The created Session holds the writer, so a second creation would open read-only.
  expect({ failed: failed.kind, kind: opened.kind,
    writer: opened.kind === "project-ready" && opened.editor.kind === "editor-ready"
      ? opened.editor.session.writer.kind : "none",
    created: server.count(SESSION_CREATE), unexpected: server.log.unexpected }).toEqual({
    failed: "project-blocked", kind: "project-ready", writer: "current_writer", created: 1, unexpected: [] });
});
