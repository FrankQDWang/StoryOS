import { afterEach, beforeEach, expect, it } from "vitest";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import rejectionFixture from "../../../../generated/golden-wire/storyos-public-release-1/reject-proposal-operations.json";
import setCurrentChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-current-chapter.json";
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
  BLOCK, CHAPTER, OWNER, PROJECT, REVISION, SESSION, chapterRevision, closeTrackedDatabases, createBrowserScenario,
  deleteJournal, emptyActivityStream, jsonResponse, trackDatabase,
} from "./scenario.ts";

// A Project opens, and opens again after a command, with fewer requests in series (#1875).
const P = `/api/v1/projects/${PROJECT}`;
const CHAPTER_B = setCurrentChapterFixture.project.open.current_chapter_id;
const REVISION_B = "018f0000-0000-7001-8000-000000000805";
const TAKEOVER_SNAPSHOT = "018f0000-0000-7001-8000-000000000831";
const CANONICAL_SNAPSHOT = "018f0000-0000-7001-8000-000000000832";
const EMPTY_BODY_DIGEST = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const PROTOCOL = "GET /api/v1/protocol";
const LIST = "GET /api/v1/projects";
const PROJECT_READ = `GET ${P}`;
const chapterRead = (chapterId: string) => `GET ${P}/chapters/${chapterId}`;
const SESSION_READ = `GET ${P}/editor-sessions/${SESSION}`;
const CHALLENGE = `POST ${P}/anti-forgery-challenges`;
const TREE = `GET ${P}/manuscript/tree`;
const STATISTICS = `GET ${P}/manuscript/statistics`;
const SWITCH = `PUT ${P}/current-chapter`;
const TAKEOVER = `POST ${P}/editor-sessions/${SESSION}/takeovers`;
const PROPOSAL = { ...proposalFixture.proposal, chapter_id: CHAPTER, manuscript_block_id: BLOCK,
  base_authoritative_revision_id: REVISION, operations: proposalFixture.proposal.operations
    .map((operation) => ({ ...operation, manuscript_block_id: BLOCK })) };
const PROPOSAL_READ = `GET ${P}/proposals/${PROPOSAL.proposal_id}`;
const ACCEPTANCE = `POST ${P}/proposals/${PROPOSAL.proposal_id}/acceptances`;
const REJECTION = `POST ${P}/proposals/${PROPOSAL.proposal_id}/rejections`;

type Writer = GetEditorSessionResponse["writer"];

/**
 * A fake Server of one Project with Chapters A (current) and B. It records the start and the end of each request.
 * `holdAfter` holds the next request of each given name after the trigger request ends, until `release`.
 */
function fakeServer(initialWriter: Writer = { kind: "current_writer", writer_generation: "1" }) {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  const state = { current: CHAPTER, writer: initialWriter, position: "0", proposals: [] as typeof PROPOSAL[] };
  const log = { started: [] as string[], unexpected: [] as string[], active: new Map<string, number>() };
  let armed: { trigger: string; names: string[] } | undefined;
  let holding = new Set<string>();
  const held: (() => void)[] = [];
  const open = () => ({ kind: "current_chapter" as const, current_chapter_id: state.current });
  const takenOver = () => state.writer.kind === "current_writer" && state.writer.writer_generation === "2";
  const session = (): GetEditorSessionResponse => {
    const base = state.current === CHAPTER ? scenario.session.base_snapshot : { ...scenario.session.base_snapshot,
      chapter_id: CHAPTER_B, authoritative_head_revision_id: REVISION_B, target_refs: [`manuscript:${CHAPTER_B}`],
      materialized_revision: chapterRevision(REVISION_B, ""),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: EMPTY_BODY_DIGEST } };
    return { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1", writer: state.writer,
      base_snapshot: takenOver() ? { ...base, snapshot_id: TAKEOVER_SNAPSHOT, project_activity_position: "1" } : base };
  };
  const chapter = (chapterId: string) => chapterId === CHAPTER
    ? { ...scenario.chapter, project_activity_position: state.position }
    : { ...scenario.chapter, project_activity_position: state.position,
      chapter: { chapter_id: CHAPTER_B, title: "Chapter B", current_revision: chapterRevision(REVISION_B, "") } };
  const canonical: SnapshotDescriptor = { snapshot_id: CANONICAL_SNAPSHOT, project_scope: scope,
    snapshot_kind: "canonical", project_activity_position: "1", source_watermarks: {}, projection_generations: {},
    redaction_profile: "storyos.author.v1", schema_profile: "storyos.public.release.1", replay_generation: "2",
    created_at: "2026-08-20T04:00:00.000Z", expires_at: null };
  const answer = async (name: string, init: RequestInit | undefined): Promise<Response> => {
    if (name === PROTOCOL) return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (name === LIST) {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: "Project A", revision: "1", open: open() }] });
    }
    if (name === PROJECT_READ) return jsonResponse({ ...scenario.project, project: { ...scenario.project.project, open: open() } });
    if (name === chapterRead(CHAPTER)) return jsonResponse(chapter(CHAPTER));
    if (name === chapterRead(CHAPTER_B)) return jsonResponse(chapter(CHAPTER_B));
    if (name === CHALLENGE) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (name === `POST ${P}/editor-sessions`) return jsonResponse({ ...session(), schema_id: scenario.session.schema_id });
    if (name === SESSION_READ) return jsonResponse(session());
    if (name === TREE) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope, tree_revision: "3",
        snapshot: canonical, volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000815", title: "Volume A",
          order: "1", chapters: [{ chapter_id: CHAPTER, title: "Chapter A", order: "1" },
            { chapter_id: CHAPTER_B, title: "Chapter B", order: "2" }] }] });
    }
    if (name === `GET ${P}/snapshots/${CANONICAL_SNAPSHOT}`) {
      return jsonResponse({ schema_id: "storyos.query.snapshot.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000015", project_scope: scope, snapshot: canonical });
    }
    if (name === `GET ${P}/activity`) return emptyActivityStream();
    // The test is not about these panels, so they show that their read is unavailable.
    if ([STATISTICS, `GET ${P}/assistance`, `GET ${P}/skills`].includes(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "service_unavailable", message: "Unavailable." }, 503);
    }
    if (name === `GET ${P}/chapters/${state.current}/open-proposals`) {
      return jsonResponse({ schema_id: "storyos.query.chapter-open-proposals.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000c71", project_scope: scope, chapter_id: state.current,
        completeness: "complete", proposals: state.proposals.map(({ proposal_id, revision_id, kind, source }) =>
          ({ proposal_id, revision_id, kind, source })) });
    }
    if (name === PROPOSAL_READ && state.proposals.length > 0) {
      return jsonResponse({ ...proposalFixture, project_scope: scope, proposal: PROPOSAL });
    }
    if (name === SWITCH) {
      state.current = (JSON.parse(String(init?.body)) as { set_current_chapter_input: { chapter_id: string } })
        .set_current_chapter_input.chapter_id;
      return jsonResponse(setCurrentChapterFixture);
    }
    if (name === TAKEOVER) {
      state.writer = { kind: "current_writer", writer_generation: "2" };
      state.position = "1";
      return jsonResponse(takeoverFixture);
    }
    if (name === ACCEPTANCE) {
      const request = JSON.parse(String(init?.body)) as AcceptProposalRequest;
      const input = request.accept_proposal_input;
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
          resulting_authoritative_revision_ids: [input.expected_authoritative_revision_id],
          authoritative_commit_ids: [], condition_refs: [], result: "conflicted", created_at: "2026-10-18T08:00:00.000Z" },
        project: { ...scenario.project.project, open: open() },
        effect: { kind: "conflicted", reason: "changed_head" } });
    }
    if (name === REJECTION) {
      const request = JSON.parse(String(init?.body)) as RejectProposalOperationsRequest;
      const input = request.reject_proposal_operations_input;
      return jsonResponse({ ...rejectionFixture, correlation_id: input.correlation_id, project_scope: scope,
        project: { ...scenario.project.project, open: open() },
        effect: { ...rejectionFixture.effect, operation_ids: input.selected_pending_operation_ids },
        receipt: { ...rejectionFixture.receipt, project_scope: scope,
          command_digest: await digestRejectProposalOperations(request, crypto),
          idempotency_key: new Headers(init?.headers).get("idempotency-key") ?? "", proposal_id: PROPOSAL.proposal_id,
          proposal_revision_id: input.proposal_revision_id,
          selected_pending_operation_ids: input.selected_pending_operation_ids,
          expected_target_revisions: input.expected_target_revisions } });
    }
    log.unexpected.push(name);
    throw new Error(`unexpected request: ${name}`);
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    log.started.push(name);
    log.active.set(name, (log.active.get(name) ?? 0) + 1);
    try {
      if (holding.delete(name)) await new Promise<void>((resolve) => { held.push(resolve); });
      return await answer(name, init);
    } finally {
      log.active.set(name, log.active.get(name)! - 1);
      if (armed?.trigger === name) {
        holding = new Set(armed.names);
        armed = undefined;
      }
    }
  };
  return {
    scenario, state, log, fetchImpl,
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
  if (loaded.state.kind === "project-ready" && loaded.state.editor.kind === "editor-ready") {
    trackDatabase(loaded.state.editor.database, openDatabases);
  }
  const view = mountStage1View(loaded.root, loaded);
  unmounts.push(() => view.unmount());
  const title = () => loaded.root.querySelector(".editor-panel h2")?.textContent;
  await expect.poll(title).toBe("Chapter A");
  return { root: loaded.root, title };
}

/** The reads of the open that a reopen after a command does not send again. */
const openReads = (requests: string[]) => requests.filter((name) => [PROTOCOL, LIST, PROJECT_READ].includes(name));

it.each([
  { tab: "with its stored Editor Session", stored: true, session: SESSION_READ },
  { tab: "in a new tab", stored: false, session: CHALLENGE },
])("opens a Project $tab with the independent reads in flight at the same time", async ({ stored, session }) => {
  const server = fakeServer();
  await deleteJournal(server.scenario.journalName);
  if (stored) sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  server.hold([PROTOCOL, PROJECT_READ, LIST]);
  const opening = openControlledProject({ baseUrl: location.origin, projectId: PROJECT,
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  await expect.poll(server.inFlight).toEqual([LIST, PROJECT_READ, PROTOCOL].sort());
  server.hold([chapterRead(CHAPTER), session]);
  server.release();
  await expect.poll(server.inFlight).toEqual([chapterRead(CHAPTER), session].sort());
  server.release();
  const opened = await opening;
  if (opened.kind === "project-ready" && opened.editor.kind === "editor-ready") {
    trackDatabase(opened.editor.database, openDatabases);
  }
  expect({ kind: opened.kind, editor: opened.kind === "project-ready" ? opened.editor.kind : undefined,
    unexpected: server.log.unexpected }).toEqual({ kind: "project-ready", editor: "editor-ready", unexpected: [] });
});

it("reopens after a Chapter switch with only the Chapter read and the Editor Session read, at the same time", async () => {
  const server = fakeServer();
  const shown = await openWorkspace(server);
  await expect.poll(() => shown.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
  const before = server.log.started.length;
  server.holdAfter(SWITCH, [chapterRead(CHAPTER_B), SESSION_READ]);

  shown.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();
  await expect.poll(server.inFlight).toEqual([chapterRead(CHAPTER_B), SESSION_READ].sort());
  server.release();

  await expect.poll(shown.title).toBe("Chapter B");
  const later = server.log.started.slice(server.log.started.indexOf(SWITCH, before));
  expect({ open: openReads(later), unexpected: server.log.unexpected }).toEqual({ open: [], unexpected: [] });
});

it.each([
  { decision: "an acceptance", button: "button[data-proposal-accept]", command: ACCEPTANCE },
  { decision: "a rejection", button: "button[data-proposal-reject]", command: REJECTION },
])("reopens after $decision with only the Chapter read and the Editor Session read, at the same time",
  async ({ button, command }) => {
    const server = fakeServer();
    server.state.proposals = [PROPOSAL];
    const shown = await openWorkspace(server);
    const control = () => shown.root.querySelector<HTMLButtonElement>(
      `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] ${button}`);
    await expect.poll(() => shown.root.querySelector(`.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"]`)
      ?.getAttribute("data-proposal-eligibility")).toBe("eligible");
    await expect.poll(() => control()?.disabled).toBe(false);
    const before = server.log.started.length;
    server.holdAfter(command, [chapterRead(CHAPTER), SESSION_READ]);

    control()!.click();
    await expect.poll(server.inFlight).toEqual([chapterRead(CHAPTER), SESSION_READ].sort());
    server.release();

    await expect.poll(() => server.count(command)).toBe(1);
    await expect.poll(() => server.inFlight()).toEqual([]);
    const later = server.log.started.slice(server.log.started.indexOf(command, before));
    expect({ open: openReads(later), unexpected: server.log.unexpected }).toEqual({ open: [], unexpected: [] });
  });

it("reopens after a takeover with the Project read, then the Chapter read and the Editor Session read at the same time",
  async () => {
    const server = fakeServer({ kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" });
    const shown = await openWorkspace(server);
    const takeOver = () => shown.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
    await expect.poll(takeOver).not.toBeNull();
    const before = server.log.started.length;
    server.holdAfter(TAKEOVER, [PROJECT_READ]);

    takeOver()!.click();
    await expect.poll(server.inFlight).toEqual([PROJECT_READ]);
    server.holdAfter(PROJECT_READ, [chapterRead(CHAPTER), SESSION_READ]);
    server.release();
    await expect.poll(server.inFlight).toEqual([chapterRead(CHAPTER), SESSION_READ].sort());
    server.release();

    await expect.poll(takeOver).toBeNull();
    const later = server.log.started.slice(server.log.started.indexOf(TAKEOVER, before));
    // The takeover check of the Journal keeps its three reads in their order, and the view uses its tree.
    const reads = later.filter((name) => ![CHALLENGE, STATISTICS, `GET ${P}/activity`, `GET ${P}/assistance`,
      `GET ${P}/skills`, `GET ${P}/chapters/${CHAPTER}/open-proposals`].includes(name));
    expect({ reads: [...reads.slice(0, 2), reads.slice(2, 4).sort(), ...reads.slice(4)],
      unexpected: server.log.unexpected }).toEqual({
      reads: [TAKEOVER, PROJECT_READ, [chapterRead(CHAPTER), SESSION_READ].sort(), TREE,
        `GET ${P}/snapshots/${CANONICAL_SNAPSHOT}`, SESSION_READ],
      unexpected: [],
    });
  });

it("reads the list of Projects, the tree, and the statistics one time for one open", async () => {
  const server = fakeServer();
  server.hold([TREE]);
  const shown = await openWorkspace(server);
  await expect.poll(server.inFlight).toContain(TREE);
  // The statistics wait for the tree revision, and the view uses the list of the open.
  expect({ list: server.count(LIST), statistics: server.count(STATISTICS) }).toEqual({ list: 1, statistics: 0 });
  server.release();

  await expect.poll(() => shown.root.querySelector("[data-manuscript-statistics]")).not.toBeNull();
  expect({ list: server.count(LIST), tree: server.count(TREE), statistics: server.count(STATISTICS),
    unexpected: server.log.unexpected }).toEqual({ list: 1, tree: 1, statistics: 1, unexpected: [] });
});

it("reads the tree one time for one open with a writer generation above 1", async () => {
  const server = fakeServer({ kind: "current_writer", writer_generation: "2" });
  server.state.position = "1";
  sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  const shown = await openWorkspace(server);

  await expect.poll(() => server.count(STATISTICS)).toBe(1);
  await expect.poll(() => shown.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
  expect({ list: server.count(LIST), tree: server.count(TREE), unexpected: server.log.unexpected })
    .toEqual({ list: 1, tree: 1, unexpected: [] });
});
