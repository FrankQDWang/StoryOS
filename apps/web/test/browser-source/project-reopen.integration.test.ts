import { afterEach, beforeEach, expect, it } from "vitest";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import conversationFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-conversation.json";
import proposalFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-proposal.json";
import rejectionFixture from "../../../../generated/golden-wire/storyos-public-release-1/reject-proposal-operations.json";
import setCurrentChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-current-chapter.json";
import sourcesFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-research-sources.json";
import skillsFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-skills.json";
import takeoverFixture from "../../../../generated/golden-wire/storyos-public-release-1/take-over-project-writer.json";
import updateChapterFixture from "../../../../generated/golden-wire/storyos-public-release-1/update-chapter.json";
import updateProjectFixture from "../../../../generated/golden-wire/storyos-public-release-1/update-project.json";
import { digestAcceptProposal, digestRejectProposalOperations }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, GetEditorSessionResponse, RejectProposalOperationsRequest, SnapshotDescriptor }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { loadStoryOSWebState } from "../../src/app.ts";
import { openControlledProject } from "../../src/boot.ts";
import { storedActivityStreamCursor } from "../../src/project-activity-sync.ts";
import { POINTER_STAY_MS } from "../../src/chapter-copies.ts";
import type { ChallengeAdmissionTimers } from "../../src/challenge-admission-wait.ts";
import { mountStage1View } from "../../src/stage1-view.tsx";
import { beginTreeAction } from "../support/inline-chapter-creation.ts";
import {
  BLOCK, CHAPTER, EDITOR_SNAPSHOT, OWNER, PROJECT, REVISION, SESSION, chapterRevision, closeTrackedDatabases,
  createBrowserScenario, deleteJournal, emptyActivityStream, jsonResponse, requestResult, trackDatabase,
} from "./scenario.ts";

// A Project opens, and opens again after a command, with fewer requests in series (#1875).
const P = `/api/v1/projects/${PROJECT}`;
const CHAPTER_B = setCurrentChapterFixture.project.open.current_chapter_id;
const REVISION_B = "018f0000-0000-7001-8000-000000000805";
const SWITCH_SNAPSHOT = "018f0000-0000-7001-8000-000000000831";
const TAKEOVER_SNAPSHOT = "018f0000-0000-7001-8000-000000000832";
const PRIOR_SESSION = "018f0000-0000-7001-8000-000000000833";
const SECOND_SESSION = "018f0000-0000-7001-8000-000000000837";
// The Snapshot of a takeover by another Editor Session.
const ELSEWHERE_SNAPSHOT = "018f0000-0000-7001-8000-000000000839";
const ELSEWHERE_SWITCH_SNAPSHOT = "018f0000-0000-7001-8000-000000000840";
const ACCEPTANCE_SNAPSHOT = "018f0000-0000-7001-8000-000000000834";
const ACCEPTED_REVISION = "018f0000-0000-7001-8000-000000000806";
const ACCEPTANCE_COMMIT = "018f0000-0000-7001-8000-000000000835";
// The Chapter A revision and the Snapshot of an edit in another tab, which writes the candidate text by hand.
const EDITED_REVISION = "018f0000-0000-7001-8000-000000000807";
const EDIT_SNAPSHOT = "018f0000-0000-7001-8000-000000000836";
const CHAPTER_B_BODY = "Text of Chapter B";
const CHAPTER_C = "018f0000-0000-7001-8000-000000000816";
const EARLIER_REVISION_B = "018f0000-0000-7001-8000-000000000817";
// The SHA-256 digest of CHAPTER_B_BODY.
const CHAPTER_B_BODY_DIGEST = "d845fc2318f60d01dc1be9c5076aa60b964a0a43ade6d5c017a6134f1813c564";
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
const ACTIVITY_READ = `GET ${P}/activity`;
const SWITCH = `PUT ${P}/current-chapter`;
const TAKEOVER = `POST ${P}/editor-sessions/${SESSION}/takeovers`;
const PROPOSAL = { ...proposalFixture.proposal, chapter_id: CHAPTER, manuscript_block_id: BLOCK,
  base_authoritative_revision_id: REVISION, operations: proposalFixture.proposal.operations
    .map((operation) => ({ ...operation, manuscript_block_id: BLOCK })) };
const PROPOSAL_READ = `GET ${P}/proposals/${PROPOSAL.proposal_id}`;
// A second open Proposal of Chapter A.
const OTHER = { ...PROPOSAL, proposal_id: "018f0000-0000-7001-8000-000000000894",
  revision_id: "018f0000-0000-7001-8000-000000000895", operations: PROPOSAL.operations
    .map((operation) => ({ ...operation, operation_id: "018f0000-0000-7001-8000-000000000896" })) };
const OTHER_READ = `GET ${P}/proposals/${OTHER.proposal_id}`;
// The Proposal after another tab edited its candidate text: a new Proposal revision.
const REVISED_TEXT = "Keep the narrator voice close in this passage.";
const REVISED_PROPOSAL = { ...PROPOSAL, revision_id: "018f0000-0000-7001-8000-000000000898", candidate_text: REVISED_TEXT,
  operations: PROPOSAL.operations.map((operation) => ({ ...operation, candidate_text: REVISED_TEXT })) };
const RENAME = `PATCH ${P}/chapters/${CHAPTER}`;
const PROJECT_RENAME = `PATCH ${P}`;
const snapshotRead = (snapshotId: string) => `GET ${P}/snapshots/${snapshotId}`;
const openProposalsRead = (chapterId: string) => `GET ${P}/chapters/${chapterId}/open-proposals`;
const ASSISTANCE = `GET ${P}/assistance`;
const SKILLS = `GET ${P}/skills`;
const PANEL_READS = [TREE, `GET ${P}/activity`, STATISTICS, ASSISTANCE, SKILLS];
const UNAVAILABLE_SENTENCE = "写作助手当前不可用。你仍可以直接写作。";
const CONVERSATION = "018f0000-0000-7001-8000-000000000891";
const RUN = "018f0000-0000-7001-8000-000000000892";
const CONVERSATION_READ = `GET ${P}/conversations/${CONVERSATION}`;
const RUN_READ = `GET ${P}/agent-runs/${RUN}`;
const SOURCES_READ = `GET ${P}/agent-runs/${RUN}/research-sources`;
/** The reads of the open of Chapter A and of its panels. */
const OPEN_READS = [PROTOCOL, LIST, PROJECT_READ, chapterRead(CHAPTER), openProposalsRead(CHAPTER), ...PANEL_READS];
const ACCEPTANCE = `POST ${P}/proposals/${PROPOSAL.proposal_id}/acceptances`;
const REJECTION = `POST ${P}/proposals/${PROPOSAL.proposal_id}/rejections`;

type Writer = GetEditorSessionResponse["writer"];
/**
 * The writer of a stored Editor Session of this tab that another tab took. The open of such a tab sends no takeover,
 * so the tab stays read-only until a click of the button (ADR 0106 section 1).
 */
const LOST_WRITER = { kind: "read_only", observed_writer_generation: "1", reason: "superseded_by_takeover" } as const;

/** The command of each route template that a Challenge names. */
const CHALLENGE_ROUTES: Record<string, string> = {
  "POST /api/v1/projects/{project_id}/editor-sessions": SESSION_CREATE,
  "PUT /api/v1/projects/{project_id}/current-chapter": SWITCH,
  "POST /api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers": TAKEOVER,
  "POST /api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances": ACCEPTANCE,
  "POST /api/v1/projects/{project_id}/proposals/{proposal_id}/rejections": REJECTION,
  "PATCH /api/v1/projects/{project_id}/chapters/{chapter_id}": RENAME,
  "PATCH /api/v1/projects/{project_id}": PROJECT_RENAME,
};

/**
 * A fake Server of one Project with Chapters A (current) and B. It serves only the `reads` and the `commands` of the
 * test (the creation of an Editor Session too), and a Challenge for one of these commands. It records each other
 * request and throws. It records the start and the end of each request, and each response body that the client read. A switch, a takeover, and an applied Acceptance
 * settle a new Snapshot, which the Editor Session base, the Chapter read, and the tree name. `acceptance` gives the
 * effect of an Acceptance. `takeoverAfterAcceptance` makes another tab take over the writer and switch to Chapter B
 * after the Acceptance settles: the Editor Session of this page keeps its base. A Rejection removes the suggestion from
 * the open suggestions. `assistant` gives the availability of the assistance read and serves the Skills; with no
 * `assistant`, both reads answer a 503 problem. `holdAfter` holds the next request of each given name after the
 * trigger request ends, until `release`. After `answerBeforeHold(names)`, a held request of these names has the answer
 * of the Server at its start. `holdEach` holds each request until `release`. `unavailable` answers the given
 * requests with a 503 problem. `lost` applies the given requests and then loses their response. A `late` Acceptance
 * applies at the first Proposal read after the next Chapter A read and Editor Session read. `takeover` and
 * `rejection` give the effect of these commands. Before a `conflicted` Rejection, another tab changes the text of
 * Chapter A, and the Operation stays pending. Before a `refused` Rejection, another tab applies the Operation.
 * `decideElsewhere` is a change of the Proposal in another tab: a new Proposal revision, or a rejection of its
 * Operation. As on the Server (`reject_proposal_operations.rs`), a Rejection of an earlier Proposal revision gets
 * `refused stale_proposal_revision`, and a Rejection of an Operation that is not pending gets `refused
 * operation_not_pending`; these refusals change nothing. `other`
 * adds a second open Proposal. As on the Server, only the first created Editor Session holds the writer; a later
 * creation gives a read-only Session. A Session with writer generation above 1 has the Snapshot of its takeover at
 * position 1. A rename of Chapter A changes its title in the tree and advances the tree revision. A
 * rename of the Project changes its title, advances its revision, and records the expected revision.
 * `renameElsewhere` is a rename of the Project in another tab. `takeOverElsewhere(generation)` is a takeover by the
 * Editor Session of another tab: the Session of this tab is read-only with `superseded_by_takeover`, and the
 * takeover settles a new Snapshot. `switchElsewhere` is a switch to Chapter A in a duplicated tab with the same Editor
 * Session, so the Session base follows the switch. `failNext(names)` answers the next request of each given name with a
 * 503 problem. `releaseOnly(names)` releases only the held requests of the given names. With
 * `proposalResult`, the Run of the Conversation has a Proposal result with a change in Chapter A. With `third`, the
 * Project also has Chapter C. After `earlierChapterB(true)`, a Chapter B read gives the text of an earlier Revision.
 */
function fakeServer({ reads, writer = { kind: "current_writer", writer_generation: "1" }, commands = [],
  proposals = false, acceptance = "refused", takeoverAfterAcceptance = false, assistant, unavailable = [], lost = [],
  takeover = "applied", rejection = "resolved", other = false, proposalResult = false, third = false }: {
  reads: string[]; writer?: Writer; commands?: string[]; proposals?: boolean;
  acceptance?: "refused" | "applied" | "late"; takeoverAfterAcceptance?: boolean;
  assistant?: "available" | "unavailable"; unavailable?: string[]; lost?: string[];
  takeover?: "applied" | "compare_failed"; rejection?: "resolved" | "conflicted" | "refused"; other?: boolean;
  proposalResult?: boolean; third?: boolean;
}) {
  const scenario = createBrowserScenario();
  const scope = scenario.project.project_scope;
  const takenOver = writer.kind === "current_writer" && writer.writer_generation !== "1";
  const state = { current: CHAPTER, writer, snapshot: { id: EDITOR_SNAPSHOT, position: takenOver ? "1" : "0" },
    accepted: false,
    rejected: false, revised: false, edited: false, title: "Chapter A", treeRevision: 3, created: 0,
    projectTitle: "Project A", projectRevision: 1, projectRenames: [] as string[],
    superseded: undefined as GetEditorSessionResponse | undefined,
    late: undefined as { chapter: boolean; session: boolean } | undefined, earlierB: false };
  const project = () => ({ ...scenario.project.project, title: state.projectTitle, open: open() });
  const revisionA = () => state.accepted ? chapterRevision(ACCEPTED_REVISION, PROPOSAL.candidate_text)
    : state.edited ? chapterRevision(EDITED_REVISION, PROPOSAL.candidate_text) : chapterRevision(REVISION, "Base");
  const log = { started: [] as string[], unexpected: [] as string[], active: new Map<string, number>(),
    events: [] as string[], consumed: [] as string[], rejectedRevisions: [] as string[] };
  let armed: { trigger: string; names: string[] } | undefined;
  let holding = new Set<string>();
  let holdEach = false;
  let answerFirst = new Set<string>();
  const failing = new Set<string>();
  const held: { name: string; resolve: () => void }[] = [];
  const open = () => ({ kind: "current_chapter" as const, current_chapter_id: state.current });
  const current = () => state.revised ? REVISED_PROPOSAL : PROPOSAL;
  const session = (): GetEditorSessionResponse => {
    if (state.superseded !== undefined) return state.superseded;
    const base = state.current === CHAPTER ? { ...scenario.session.base_snapshot,
      authoritative_head_revision_id: revisionA().revision_id, materialized_revision: revisionA(),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: state.accepted || state.edited ? ACCEPTED_BODY_DIGEST
          : scenario.session.base_snapshot.materialized_payload_digest.value_hex_lowercase } }
      : { ...scenario.session.base_snapshot,
      chapter_id: CHAPTER_B, authoritative_head_revision_id: REVISION_B, target_refs: [`manuscript:${CHAPTER_B}`],
      materialized_revision: chapterRevision(REVISION_B, CHAPTER_B_BODY),
      materialized_payload_digest: { ...scenario.session.base_snapshot.materialized_payload_digest,
        value_hex_lowercase: CHAPTER_B_BODY_DIGEST } };
    return { ...scenario.session, schema_id: "storyos.query.editor-session.response.v1", writer: state.writer,
      base_snapshot: { ...base, snapshot_id: state.snapshot.id, project_activity_position: state.snapshot.position } };
  };
  const chapter = (chapterId: string) => chapterId === CHAPTER
    ? { ...scenario.chapter, project_activity_position: state.snapshot.position,
      chapter: { ...scenario.chapter.chapter, current_revision: revisionA() } }
    : { ...scenario.chapter, project_activity_position: state.snapshot.position,
      chapter: { chapter_id: CHAPTER_B, title: "Chapter B", current_revision: chapterRevision(REVISION_B, CHAPTER_B_BODY) } };
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
    if (unavailable.includes(name) || failing.delete(name)) {
      // The problem of a Server whose Project store is unavailable (`project_reader` of `storyos-server`).
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "project_store_unavailable",
        message: "The Project store is unavailable." }, 503);
    }
    if (name === PROTOCOL) return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (name === LIST) {
      return jsonResponse({ schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013", owner_user_id: OWNER,
        projects: [{ project_scope: scope, title: state.projectTitle, revision: String(state.projectRevision),
          open: open() }] });
    }
    if (name === PROJECT_READ) return jsonResponse({ ...scenario.project, project: project() });
    if (name === chapterRead(CHAPTER)) {
      if (state.late !== undefined) state.late.chapter = true;
      return jsonResponse(chapter(CHAPTER));
    }
    if (name === chapterRead(CHAPTER_B) && state.earlierB) {
      return jsonResponse({ ...chapter(CHAPTER_B), chapter: { chapter_id: CHAPTER_B, title: "Chapter B",
        current_revision: chapterRevision(EARLIER_REVISION_B, "Earlier text of Chapter B") } });
    }
    if (name === chapterRead(CHAPTER_B)) return jsonResponse(chapter(CHAPTER_B));
    if (name === chapterRead(CHAPTER_C)) {
      return jsonResponse({ ...scenario.chapter, project_activity_position: state.snapshot.position, chapter: {
        chapter_id: CHAPTER_C, title: "Chapter C", current_revision: chapterRevision(ACCEPTED_REVISION, "Gamma") } });
    }
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
    if (name === SESSION_READ) {
      if (state.late !== undefined) state.late.session = true;
      return jsonResponse(session());
    }
    if (name === TREE) {
      return jsonResponse({ schema_id: "storyos.query.manuscript-tree.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000014", project_scope: scope,
        tree_revision: String(state.treeRevision),
        snapshot: canonical(), volumes: [{ volume_id: "018f0000-0000-7001-8000-000000000815", title: "Volume A",
          order: "1", chapters: [{ chapter_id: CHAPTER, title: state.title, order: "1" },
            { chapter_id: CHAPTER_B, title: "Chapter B", order: "2" },
            ...(third ? [{ chapter_id: CHAPTER_C, title: "Chapter C", order: "3" }] : [])] }] });
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
    if (name === CONVERSATION_READ) {
      return jsonResponse({ ...conversationFixture, project_scope: scope, conversation_id: CONVERSATION,
        runs: [{ ...conversationFixture.runs[0], run_id: RUN, chapter_id: CHAPTER }] });
    }
    if (name === SOURCES_READ) {
      return jsonResponse({ ...sourcesFixture, project_scope: scope, run_id: RUN, snapshots: [], gaps: [],
        unknown_calls: [] });
    }
    if (name === RUN_READ) {
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN, conversation_id: CONVERSATION,
        status: "completed", results: proposalResult ? [{ kind: "proposal", decision_id: PROPOSAL.source.decision_id,
          proposal_id: PROPOSAL.proposal_id, locations: [{ chapter_id: CHAPTER, manuscript_block_id: BLOCK,
            base_authoritative_revision_id: REVISION, candidate_text: PROPOSAL.candidate_text,
            explanation: "Keep the narrator voice.", current: null, outcome: { kind: "opened",
              proposal_id: PROPOSAL.proposal_id, operation_id: PROPOSAL.operations[0]!.operation_id,
              revision_id: PROPOSAL.revision_id, validation_receipt_id: "018f0000-0000-7001-8000-000000000897" } }] }]
          : [], decision: { kind: "advisory", decision_id: "018f0000-0000-7001-8000-000000000893",
          selected: true, text: "Keep the voice of Chapter A.", continuation: { kind: "absent" } } });
    }
    // A test that is not about these panels shows that their read is unavailable.
    if ([STATISTICS, ASSISTANCE, SKILLS].includes(name)) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "service_unavailable", message: "Unavailable." }, 503);
    }
    if (name === `GET ${P}/chapters/${state.current}/open-proposals`) {
      return jsonResponse({ schema_id: "storyos.query.chapter-open-proposals.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000c71", project_scope: scope, chapter_id: state.current,
        completeness: "complete", proposals: [...(proposals && !state.accepted && !state.rejected ? [current()] : []),
          ...(other ? [OTHER] : [])].map(({ proposal_id, revision_id, kind,
          source }) => ({ proposal_id, revision_id, kind, source })) });
    }
    // Chapters B and C have no open suggestion.
    const listed = [CHAPTER_B, CHAPTER_C].find((id) => name === openProposalsRead(id));
    if (listed !== undefined) {
      return jsonResponse({ schema_id: "storyos.query.chapter-open-proposals.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000c72", project_scope: scope, chapter_id: listed,
        completeness: "complete", proposals: [] });
    }
    if (name === PROPOSAL_READ && state.late?.chapter === true && state.late.session) {
      state.late = undefined;
      state.accepted = true;
      settle(ACCEPTANCE_SNAPSHOT);
    }
    if (name === OTHER_READ) return jsonResponse({ ...proposalFixture, project_scope: scope, proposal: OTHER });
    if (name === RENAME) {
      state.title = (JSON.parse(String(init?.body)) as { update_chapter_input: { title: string } })
        .update_chapter_input.title;
      state.treeRevision += 1;
      return jsonResponse({ ...updateChapterFixture, project_scope: scope, project: project(),
        effect: { ...updateChapterFixture.effect, chapter_id: CHAPTER, title: state.title, order: "1",
          tree_revision: String(state.treeRevision) },
        receipt: { ...updateChapterFixture.receipt, project_scope: scope } });
    }
    if (name === PROPOSAL_READ) {
      // As on the Server, a decided Operation has a resolved reservation.
      const resolution = state.accepted ? "applied" : state.rejected ? "rejected" : undefined;
      return jsonResponse({ ...proposalFixture, project_scope: scope, proposal: resolution === undefined ? current()
        : { ...current(), operation_resolution: resolution, reservation_state: "resolved", operations: current()
          .operations.map((operation) => ({ ...operation, resolution, reservation_state: "resolved" })) } });
    }
    if (name === PROJECT_RENAME) {
      const input = (JSON.parse(String(init?.body)) as {
        update_project_input: { title: string; expected_project_revision: string } }).update_project_input;
      state.projectRenames.push(input.expected_project_revision);
      state.projectTitle = input.title;
      state.projectRevision += 1;
      return jsonResponse({ ...updateProjectFixture, project_scope: scope, project: project(),
        effect: { ...updateProjectFixture.effect, title: state.projectTitle, revision: String(state.projectRevision) },
        receipt: { ...updateProjectFixture.receipt, project_scope: scope } });
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
    if (name === TAKEOVER && takeover === "compare_failed") {
      // Another tab took the writer after the admission of this takeover, so the Server changes nothing.
      return jsonResponse({ ...takeoverFixture,
        receipt: { ...takeoverFixture.receipt, expected_heads: [REVISION], prior_heads: [REVISION],
          resulting_heads: [REVISION] },
        result: { kind: "takeover_compare_failed", observed_writer_generation: "1", current_writer_generation: "2",
          current_writer_projection: { kind: "read_only", observed_writer_generation: "2", reason: "secondary_session" },
          current_snapshot_id: state.snapshot.id, current_snapshot_activity_position: state.snapshot.position,
          current_heads: [REVISION], reason: "writer_generation_advanced_after_admission" } });
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
      if (acceptance === "late" && state.late === undefined && !state.accepted) {
        state.late = { chapter: false, session: false };
      }
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
    log.rejectedRevisions.push(input.proposal_revision_id);
    const refusal = input.proposal_revision_id !== current().revision_id ? "stale_proposal_revision"
      : state.accepted || state.rejected ? "operation_not_pending" : undefined;
    if (refusal !== undefined) {
      // A refused Rejection changes nothing.
    } else if (rejection === "conflicted") {
      // Another tab changed the Chapter text. The Operation stays pending, so the Server finds a changed Chapter Head.
      state.edited = true;
      settle(EDIT_SNAPSHOT);
    } else if (rejection === "refused") {
      // Another tab applied the Operation, so the Server refuses: the Operation is not pending.
      state.accepted = true;
      settle(ACCEPTANCE_SNAPSHOT);
    } else {
      state.rejected = true;
    }
    return jsonResponse({ ...rejectionFixture, correlation_id: input.correlation_id, project_scope: scope,
      project: project(),
      effect: refusal !== undefined ? { kind: "refused", reason: refusal }
        : rejection === "conflicted" ? { kind: "conflicted", reason: "changed_head" }
        : rejection === "refused" ? { kind: "refused", reason: "operation_not_pending" }
        : { ...rejectionFixture.effect, operation_ids: input.selected_pending_operation_ids },
      receipt: { ...rejectionFixture.receipt, project_scope: scope,
        command_digest: await digestRejectProposalOperations(request, crypto),
        idempotency_key: new Headers(init?.headers).get("idempotency-key") ?? "", proposal_id: PROPOSAL.proposal_id,
        proposal_revision_id: input.proposal_revision_id,
        selected_pending_operation_ids: input.selected_pending_operation_ids,
        // As on the Server (`reject_proposal_operations.rs`), the three lists have the expected Chapter revision.
        expected_target_revisions: input.expected_target_revisions,
        prior_authoritative_revision_ids: input.expected_target_revisions,
        resulting_authoritative_revision_ids: input.expected_target_revisions,
        ...(refusal !== undefined ? { result: "refused" } : rejection === "resolved" ? {} : { result: rejection }) } });
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    log.started.push(name);
    log.events.push(`start ${name}`);
    log.active.set(name, (log.active.get(name) ?? 0) + 1);
    try {
      const hold = holding.delete(name) || holdEach;
      const early = hold && answerFirst.has(name) ? answer(name, init) : undefined;
      if (hold) await new Promise<void>((resolve) => { held.push({ name, resolve }); });
      const response = await (early ?? answer(name, init));
      if (lost.includes(name)) throw new TypeError("Failed to fetch");
      const text = response.text.bind(response);
      response.text = async () => {
        const body = await text();
        log.consumed.push(name);
        return body;
      };
      return response;
    } finally {
      log.active.set(name, log.active.get(name)! - 1);
      log.events.push(`end ${name}`);
      if (armed?.trigger === name) {
        holding = new Set(armed.names);
        armed = undefined;
      }
    }
  };
  return {
    scenario, log, fetchImpl, projectRenames: state.projectRenames,
    // The open reads the stored Session of a lost writer.
    stored: writer.kind === "read_only" && writer.reason === "superseded_by_takeover",
    answerBeforeHold(names: string[]) { answerFirst = new Set(names); },
    decideElsewhere(change: "revised" | "rejected") {
      if (change === "revised") state.revised = true;
      else state.rejected = true;
    },
    renameElsewhere(title: string) { state.projectTitle = title; state.projectRevision += 1; },
    takeOverElsewhere(generation: string) {
      // The Server keeps the base of this Session, and the takeover settles a new Snapshot.
      state.superseded = { ...session(), writer: { kind: "read_only", observed_writer_generation: generation,
        reason: "superseded_by_takeover" } };
      settle(ELSEWHERE_SNAPSHOT);
    },
    switchElsewhere() {
      state.current = CHAPTER;
      settle(ELSEWHERE_SWITCH_SNAPSHOT);
    },
    failNext(names: string[]) { for (const name of names) failing.add(name); },
    earlierChapterB(on: boolean) { state.earlierB = on; },
    holdAfter(trigger: string, names: string[]) { armed = { trigger, names }; },
    hold(names: string[]) { holding = new Set(names); },
    holdEach(on: boolean) { holdEach = on; },
    held: () => held.length,
    release() { for (const { resolve } of held.splice(0)) resolve(); },
    releaseOnly(names: string[]) {
      for (const entry of held.filter(({ name }) => names.includes(name))) {
        held.splice(held.indexOf(entry), 1);
        entry.resolve();
      }
    },
    inFlight: () => [...log.active].filter(([, count]) => count > 0).map(([name]) => name).sort(),
    count: (name: string) => log.started.filter((entry) => entry === name).length,
  };
}

const openDatabases = new Set<IDBDatabase>();
// Each Journal connection that the page closed.
const closedDatabases = new Set<IDBDatabase>();
const unmounts: (() => void)[] = [];
// The view opens the Journal again at each reopen, so the test closes each connection before it deletes the Journal.
const openJournal = indexedDB.open.bind(indexedDB);

beforeEach(() => {
  Object.defineProperty(indexedDB, "open", { configurable: true, value: (name: string, version?: number) => {
    const request = openJournal(name, version);
    request.addEventListener("success", () => {
      const database = request.result;
      trackDatabase(database, openDatabases);
      const close = database.close.bind(database);
      database.close = () => { closedDatabases.add(database); close(); };
    });
    return request;
  } });
});

afterEach(async () => {
  for (const unmount of unmounts.splice(0)) unmount();
  Reflect.deleteProperty(indexedDB, "open");
  history.replaceState(null, "", location.pathname);
  closeTrackedDatabases(openDatabases);
  closedDatabases.clear();
  document.body.replaceChildren();
  sessionStorage.clear();
  await deleteJournal(createBrowserScenario().journalName);
});

/** Opens the workspace at the address with the query `search`, for example the Conversation of the address. */
async function openWorkspace(server: ReturnType<typeof fakeServer>, search = "",
  writingTimers?: ChallengeAdmissionTimers) {
  await deleteJournal(server.scenario.journalName);
  if (server.stored) sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  history.replaceState(null, "", `${location.pathname}${search}`);
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const view = mountStage1View(loaded.root, { ...loaded, writingTimers });
  unmounts.push(() => view.unmount());
  // A writable workspace shows the saved state. A read-only recovery shows "needs_attention".
  const shown = () => ({ title: loaded.root.querySelector(".editor-panel h2")?.textContent,
    save: loaded.root.querySelector("[data-save-state]")?.getAttribute("data-save-state"),
    failure: loaded.root.querySelector("[data-editor-failure]")?.getAttribute("data-editor-failure") });
  await expect.poll(() => shown().title).toBe("Chapter A");
  return { root: loaded.root, shown };
}

/** Loads the page again with the Local Edit Journal and the session storage of the tab. */
async function reloadWorkspace(server: ReturnType<typeof fakeServer>) {
  for (const unmount of unmounts.splice(0)) unmount();
  document.body.innerHTML = '<main id="app"></main>';
  const loaded = await loadStoryOSWebState({ documentImpl: document,
    locationImpl: { origin: location.origin, pathname: `/projects/${PROJECT}` },
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const view = mountStage1View(loaded.root, loaded);
  unmounts.push(() => view.unmount());
  return loaded.root;
}

/** The reads of the open that a reopen after a command sends again (the Project and the list), and the protocol read. */
const openReads = (requests: string[]) => requests.filter((name) => [PROTOCOL, LIST, PROJECT_READ].includes(name));

/** The four reads of a reopen at the Chapter `chapterId`. */
const reopenReads = (chapterId: string) => [PROJECT_READ, LIST, chapterRead(chapterId), SESSION_READ];

/**
 * Proves the one step of a reopen after `command` (#1898): the fake holds the Project read, the list read, the Chapter
 * read, and the Editor Session read, and sees the four in flight before it answers one of them. Then it releases them.
 */
async function expectReopenSteps(server: ReturnType<typeof fakeServer>, command: string, chapterId: string) {
  server.holdAfter(command, reopenReads(chapterId));
  return async () => {
    await expect.poll(server.inFlight).toEqual(reopenReads(chapterId).sort());
    server.release();
  };
}

it("opens a Project with its stored Editor Session with the independent reads in flight at the same time", async () => {
  const server = fakeServer({ reads: [PROTOCOL, LIST, PROJECT_READ, chapterRead(CHAPTER), SESSION_READ] });
  await deleteJournal(server.scenario.journalName);
  sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  server.hold([PROTOCOL, PROJECT_READ, LIST]);
  const opening = openControlledProject({ baseUrl: location.origin, projectId: PROJECT,
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  await expect.poll(server.inFlight).toEqual([LIST, PROJECT_READ, PROTOCOL].sort());
  server.hold([chapterRead(CHAPTER), SESSION_READ]);
  server.release();
  await expect.poll(server.inFlight).toEqual([chapterRead(CHAPTER), SESSION_READ].sort());
  server.release();
  const opened = await opening;
  expect({ kind: opened.kind, editor: opened.kind === "project-ready" ? opened.editor.kind : undefined,
    unexpected: server.log.unexpected }).toEqual({ kind: "project-ready", editor: "editor-ready", unexpected: [] });
});

it("creates no Editor Session when the Chapter read of a new tab fails, and the next open is the writer", async () => {
  const unavailable = [chapterRead(CHAPTER)];
  const server = fakeServer({ reads: [PROTOCOL, LIST, PROJECT_READ, chapterRead(CHAPTER)], commands: [SESSION_CREATE],
    unavailable });
  await deleteJournal(server.scenario.journalName);
  const open = () => openControlledProject({ baseUrl: location.origin, projectId: PROJECT,
    fetchImpl: server.fetchImpl, indexedDBImpl: indexedDB, cryptoImpl: crypto });
  const failed = await open();
  // The failed open closes its Journal as its last step, so no work of it is left after that.
  await expect.poll(() => openDatabases.size === 1 && [...openDatabases].every((database) =>
    closedDatabases.has(database))).toBe(true);
  const created = server.count(SESSION_CREATE);
  const retry = server.log.started.length;
  unavailable.length = 0;

  const opened = await open();

  // Only the first created Session holds the writer, so an earlier creation would make this open read-only.
  expect({ failed: failed.kind, created, retry: server.log.started.slice(retry)
    .filter((name) => name === SESSION_CREATE).length, kind: opened.kind,
    writer: opened.kind === "project-ready" && opened.editor.kind === "editor-ready"
      ? opened.editor.session.writer.kind : "none", unexpected: server.log.unexpected }).toEqual({
    failed: "project-blocked", created: 0, retry: 1, kind: "project-ready", writer: "current_writer", unexpected: [] });
});

it("reopens after a Chapter switch with the Project, the list, the Chapter, and the Editor Session reads together",
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
])("reopens after $decision with the Project, the list, the Chapter, and the Editor Session reads together",
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

it("reopens after an applied acceptance with the Project, the list, the Chapter, and the Editor Session reads together",
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

it("reopens after a takeover with the Project, the list, the Chapter, and the Editor Session reads together",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT)],
      writer: LOST_WRITER,
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
    expect({ reads: [reads[0], reads.slice(1, 5).sort(), ...reads.slice(5)],
      unexpected: server.log.unexpected }).toEqual({
      reads: [TAKEOVER, reopenReads(CHAPTER).sort(), TREE, snapshotRead(TAKEOVER_SNAPSHOT), SESSION_READ],
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

    // The response of the refused Acceptance names Chapter A, but the Project read names Chapter B. The reopen read
    // Chapter A with the Project read, discards it, and reads Chapter B (#1898).
    await expect.poll(() => workspace.shown().title).toBe("Chapter B");
    const later = server.log.started.slice(server.log.started.indexOf(ACCEPTANCE, before));
    expect({ open: openReads(later).sort(), chapters: later.filter((name) => name.startsWith(`GET ${P}/chapters/`)
      && !name.endsWith("/open-proposals")), unexpected: server.log.unexpected }).toEqual({
      open: [LIST, PROJECT_READ].sort(), chapters: [chapterRead(CHAPTER), chapterRead(CHAPTER_B)], unexpected: [] });
  });

/** Each row of each store of the Local Edit Journal of the Project, with its key. */
async function journalRows(server: ReturnType<typeof fakeServer>) {
  const journal = await requestResult(indexedDB.open(server.scenario.journalName)) as IDBDatabase;
  const names = [...journal.objectStoreNames];
  const transaction = journal.transaction(names, "readonly");
  const rows = await Promise.all(names.map(async (name) => {
    const store = transaction.objectStore(name);
    const [keys, values] = await Promise.all([requestResult(store.getAllKeys()), requestResult(store.getAll())]);
    return [name, keys.map((key, index) => [key, values[index]])];
  }));
  journal.close();
  return Object.fromEntries(rows);
}

it.each([
  { order: "the early Chapter read and then the early Session read", first: chapterRead(CHAPTER_B), second: SESSION_READ },
  { order: "the early Session read and then the early Chapter read", first: SESSION_READ, second: chapterRead(CHAPTER_B) },
])("discards the Chapter read of a switch when the Project read names another Chapter, and shows only that Chapter,"
  + " after $order", async ({ first, second }) => {
    const server = fakeServer({ reads: [...SWITCH_READS, chapterRead(CHAPTER), SESSION_READ],
      commands: [SESSION_CREATE, SWITCH] });
    const workspace = await openWorkspace(server);
    await expect.poll(() => chapterB(workspace.root)?.disabled).toBe(false);
    await expect.poll(server.inFlight).toEqual([]);
    // The open stores the cursor of its Project Activity Stream after the client read the stream.
    await expect.poll(async () => JSON.stringify(await journalRows(server))).toContain(
      `"project_activity_stream:${OWNER}:${PROJECT}"`);
    const journal = await journalRows(server);
    const before = server.log.started.length;
    // The observer records the title, the text, and the Revision of the editor at each change of the page. The
    // selection shows the Arriving Chapter B read-only from the Chapter read of the switch (ADR 0105). An editor of the
    // discarded read has the Revision of Chapter B. A render that a later render in the same task replaces is gone
    // before the observer reads the page. Thus for each mutation record, the observer also examines the target, the
    // old value, and each added and each removed node: the Revision of Chapter B anywhere.
    const shownChapters = new Set<string>();
    const discardedShown = new Set<string>();
    const revision = "data-authoritative-revision-id";
    const examine = (node: Node, value: string | null) => {
      if (value === REVISION_B) discardedShown.add(REVISION_B);
      if (node instanceof Element && [node, ...node.querySelectorAll(`[${revision}]`)]
        .some((element) => element.getAttribute(revision) === REVISION_B)) discardedShown.add(REVISION_B);
    };
    const observer = new MutationObserver((records) => {
      for (const record of records) {
        examine(record.target, record.oldValue);
        for (const node of [...record.addedNodes, ...record.removedNodes]) examine(node, null);
      }
      shownChapters.add(JSON.stringify([workspace.root.querySelector(".editor-panel h2")?.textContent,
        workspace.root.querySelector("[data-manuscript-editor]")?.textContent,
        workspace.root.querySelector(`[${revision}]`)?.getAttribute(revision)]));
    });
    observer.observe(workspace.root, { subtree: true, childList: true, characterData: true, attributes: true,
      attributeFilter: [revision], attributeOldValue: true });
    server.holdAfter(SWITCH, reopenReads(CHAPTER_B));

    chapterB(workspace.root)!.click();
    await expect.poll(server.inFlight).toEqual(reopenReads(CHAPTER_B).sort());
    // A duplicated tab with the same Editor Session switches back to Chapter A before the four reads end.
    server.switchElsewhere();
    server.hold([chapterRead(CHAPTER)]);
    const consumedBefore = server.log.consumed.length;
    const consumed = (names: string[]) => names.every((name) => server.log.consumed.slice(consumedBefore).includes(name));
    // The discarded open ends before its replacement starts: the fake answers the two early reads one by one, each
    // after the Project read. After the client read a response body, the steps that follow it in the client end before
    // the next poll, so a request that they start is in flight at the check.
    server.releaseOnly([PROJECT_READ, LIST]);
    await expect.poll(() => consumed([PROJECT_READ, LIST])).toBe(true);
    expect(server.inFlight()).toEqual([chapterRead(CHAPTER_B), SESSION_READ].sort());
    server.releaseOnly([first]);
    await expect.poll(() => consumed([first])).toBe(true);
    expect(server.inFlight()).toEqual([second]);
    server.releaseOnly([second]);
    await expect.poll(server.inFlight).toEqual([chapterRead(CHAPTER)]);
    // The discarded reads ended and wrote nothing to the Journal.
    const discarded = await journalRows(server);
    server.release();

    await expect.poll(() => ({ ...workspace.shown(), inFlight: server.inFlight() }))
      .toEqual({ title: "Chapter A", save: "saved", failure: "", inFlight: [] });
    observer.disconnect();
    const later = server.log.started.slice(before);
    expect({ shown: [...shownChapters], discardedShown: [...discardedShown], journal: discarded,
      // The four reads of each open start at the same time. The panels of the editor part read with no order.
      later: [...later.slice(0, 3), later.slice(3, 7).sort(), ...later.slice(7, 9), later.slice(9).sort()],
      unexpected: server.log.unexpected }).toEqual({
      // With no copy, the editor part shows the earlier text read-only until the Chapter read of the switch answers.
      shown: [JSON.stringify(["Chapter A", "Base", REVISION]), JSON.stringify(["Chapter B", CHAPTER_B_BODY, REVISION])],
      discardedShown: [], journal,
      later: [chapterRead(CHAPTER_B), CHALLENGE, SWITCH, reopenReads(CHAPTER_B).sort(), chapterRead(CHAPTER), SESSION_READ,
        [openProposalsRead(CHAPTER), TREE, `GET ${P}/activity`, STATISTICS].sort()],
      unexpected: [] });
  });

it.each([
  { read: "the Chapter read", failed: chapterRead(CHAPTER_B) },
  { read: "the Project read", failed: PROJECT_READ },
])("shows the Project as unavailable when $read of the reopen after a switch fails", async ({ failed }) => {
  const server = fakeServer({ reads: SWITCH_READS, commands: [SESSION_CREATE, SWITCH] });
  const workspace = await openWorkspace(server);
  await expect.poll(() => chapterB(workspace.root)?.disabled).toBe(false);
  await expect.poll(server.inFlight).toEqual([]);
  server.holdAfter(SWITCH, [failed]);
  const connections = new Set(openDatabases);

  chapterB(workspace.root)!.click();
  await expect.poll(server.inFlight).toEqual([failed]);
  server.failNext([failed]);
  server.release();

  await expect.poll(() => ({ boot: workspace.root.dataset.bootState, inFlight: server.inFlight(),
    heading: workspace.root.querySelector("[role=alert] h1")?.textContent }))
    .toEqual({ boot: "project-blocked", inFlight: [], heading: "StoryOS 无法打开项目" });
  // The reopen closes each Journal connection that it opened.
  await expect.poll(() => [...openDatabases].filter((database) => !connections.has(database)
    && !closedDatabases.has(database)).length).toBe(0);
  expect(server.log.unexpected).toEqual([]);
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

// The takeover check runs only when this page did not do its work for the Editor Session and the writer generation
// (#1897). Its reads are the tree, the Snapshot of the tree, and a second Editor Session read.
/** The Editor Session, tree, and Snapshot reads in `requests` up to the last Editor Session read. */
const checkReads = (requests: string[]) => requests.slice(0, requests.lastIndexOf(SESSION_READ) + 1)
  .filter((name) => name === TREE || name === SESSION_READ || name.startsWith(`GET ${P}/snapshots/`));
const chapterB = (root: HTMLElement) => root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`);

/** Opens the workspace with the Editor Session of this tab, which holds writer generation 2. */
async function openSecondGeneration(server: ReturnType<typeof fakeServer>) {
  sessionStorage.setItem(`active_session:${OWNER}:${PROJECT}`, SESSION);
  const workspace = await openWorkspace(server);
  await expect.poll(() => chapterB(workspace.root)?.disabled).toBe(false);
  await expect.poll(server.inFlight).toEqual([]);
  return workspace;
}

/**
 * Holds the second Editor Session read of the check after the read of the Snapshot `snapshotId`. Then another Editor
 * Session takes over the writer with `generation`, so this Session is read-only and the check refuses the open.
 */
async function takeOverDuringCheck(server: ReturnType<typeof fakeServer>, snapshotId: string, generation: string) {
  server.holdAfter(snapshotRead(snapshotId), [SESSION_READ]);
  return async () => {
    await expect.poll(server.inFlight).toEqual([SESSION_READ]);
    server.takeOverElsewhere(generation);
    server.release();
  };
}

const SWITCH_READS = [...OPEN_READS, SESSION_READ, chapterRead(CHAPTER_B), openProposalsRead(CHAPTER_B)];

it("opens the Chapter of a switch with writer generation 2 and sends no read of the takeover check", async () => {
  const server = fakeServer({ reads: [...SWITCH_READS, snapshotRead(EDITOR_SNAPSHOT)],
    writer: { kind: "current_writer", writer_generation: "2" }, commands: [SWITCH] });
  const workspace = await openSecondGeneration(server);
  // The view took the Activity of the Project, so the Journal holds a stream cursor.
  const journal = await requestResult(indexedDB.open(server.scenario.journalName)) as IDBDatabase;
  const cursor = await storedActivityStreamCursor(journal.transaction("metadata").objectStore("metadata"),
    server.scenario.project.project_scope);
  journal.close();
  expect(cursor).toEqual({ snapshot_id: EDITOR_SNAPSHOT, last_event_id: null });
  const before = server.log.started.length;
  // The view reads the tree after the new view. A check of the open waits for this held read.
  server.holdAfter(SWITCH, [TREE]);

  chapterB(workspace.root)!.click();

  await expect.poll(() => ({ ...workspace.shown(), held: server.inFlight().includes(TREE) }))
    .toEqual({ title: "Chapter B", save: "saved", failure: "", held: true });
  server.release();
  await expect.poll(server.inFlight).toEqual([]);
  const later = server.log.started.slice(server.log.started.indexOf(SWITCH, before));
  expect({ first: checkReads(server.log.started.slice(0, before)), later: checkReads(later),
    unexpected: server.log.unexpected }).toEqual({ first: [SESSION_READ, TREE, snapshotRead(EDITOR_SNAPSHOT),
    SESSION_READ], later: [SESSION_READ], unexpected: [] });
});

it("runs the takeover check at the first open after a takeover in this tab, and refuses a changed Session",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT)],
      writer: LOST_WRITER,
      commands: [SESSION_CREATE, TAKEOVER] });
    const workspace = await openWorkspace(server);
    const takeOver = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
    await expect.poll(() => takeOver()?.disabled).toBe(false);
    const before = server.log.started.length;
    const takenOver = await takeOverDuringCheck(server, TAKEOVER_SNAPSHOT, "3");

    takeOver()!.click();
    await takenOver();

    // The refused open has no writer, so the page shows no takeover button.
    await expect.poll(() => ({ save: workspace.shown().save, takeOver: takeOver(), inFlight: server.inFlight() }))
      .toEqual({ save: "needs_attention", takeOver: null, inFlight: [] });
    expect({ check: checkReads(server.log.started.slice(before)), unexpected: server.log.unexpected })
      .toEqual({ check: [SESSION_READ, TREE, snapshotRead(TAKEOVER_SNAPSHOT), SESSION_READ], unexpected: [] });
  });

it("runs the takeover check at a reload with an empty local database, and refuses a changed Session", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(EDITOR_SNAPSHOT)],
    writer: { kind: "current_writer", writer_generation: "2" } });
  await openSecondGeneration(server);
  for (const unmount of unmounts.splice(0)) unmount();
  closeTrackedDatabases(openDatabases);
  await deleteJournal(server.scenario.journalName);
  const before = server.log.started.length;
  const takenOver = await takeOverDuringCheck(server, EDITOR_SNAPSHOT, "3");

  const reloaded = reloadWorkspace(server);
  await takenOver();
  const root = await reloaded;

  await expect.poll(() => root.querySelector("[data-save-state]")?.getAttribute("data-save-state"))
    .toBe("needs_attention");
  expect({ check: checkReads(server.log.started.slice(before)), unexpected: server.log.unexpected })
    .toEqual({ check: [SESSION_READ, TREE, snapshotRead(EDITOR_SNAPSHOT), SESSION_READ], unexpected: [] });
});

it("refuses the open after a Chapter switch when another Editor Session took over with writer generation 3",
  async () => {
    const server = fakeServer({ reads: [...SWITCH_READS, snapshotRead(EDITOR_SNAPSHOT)],
      writer: { kind: "current_writer", writer_generation: "2" }, commands: [SWITCH] });
    const workspace = await openSecondGeneration(server);
    const before = server.log.started.length;
    server.holdAfter(SWITCH, reopenReads(CHAPTER_B));

    chapterB(workspace.root)!.click();
    await expect.poll(server.inFlight).toEqual(reopenReads(CHAPTER_B).sort());
    // Another tab takes over the writer after the switch settled, so this Session is read-only.
    server.takeOverElsewhere("3");
    server.release();

    // The open shows the Session read-only, with the sentence of a lost writer (ADR 0106 section 1).
    await expect.poll(() => ({ ...workspace.shown(), inFlight: server.inFlight(),
      writer: workspace.root.querySelector("[data-writer-kind]")?.getAttribute("data-writer-kind"),
      reason: workspace.root.querySelector("[data-writer-read-only]")?.textContent }))
      .toEqual({ title: "Chapter B", save: "needs_attention", failure: "", inFlight: [], writer: "read_only",
        reason: "已在另一个标签页继续写作。" });
    expect({ check: checkReads(server.log.started.slice(server.log.started.indexOf(SWITCH, before))),
      unexpected: server.log.unexpected }).toEqual({ check: [SESSION_READ], unexpected: [] });
  });

/**
 * Puts a copy of the stored partition and of its active base into the Journal of the Project, with another owner User.
 * The base is older than the base of the next open, so only the comparison of the User can refuse the copy.
 */
async function storePartitionOfAnotherUser(server: ReturnType<typeof fakeServer>) {
  const otherUser = "018f0000-0000-7001-8000-000000000838";
  const journal = await requestResult(indexedDB.open(server.scenario.journalName)) as IDBDatabase;
  const transaction = journal.transaction(["partitions", "metadata"], "readwrite");
  const partitions = transaction.objectStore("partitions");
  const metadata = transaction.objectStore("metadata");
  const [stored] = await requestResult(partitions.getAll()) as { journal_partition_id: string;
    project_scope: { owner_user_id: string; project_id: string } }[];
  const copyId = stored!.journal_partition_id.replace(OWNER, otherUser);
  const base = await requestResult(metadata.get(`active_base:${stored!.journal_partition_id}`)) as { value: unknown };
  await requestResult(partitions.put({ ...stored!, project_scope: { ...stored!.project_scope, owner_user_id: otherUser },
    journal_partition_id: copyId }));
  await requestResult(metadata.put({ key: `active_base:${copyId}`, value: base.value }));
  journal.close();
}

it("runs the takeover check after a takeover when the Journal holds a partition of another User, and refuses",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT)],
      writer: { kind: "read_only", observed_writer_generation: "1", reason: "secondary_session" },
      commands: [SESSION_CREATE, TAKEOVER] });
    // The open takes over the writer with no click (ADR 0106 section 1). The fake holds that takeover until the
    // Journal holds the partition of another User.
    server.hold([TAKEOVER]);
    const workspace = await openWorkspace(server);
    const takeOver = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
    await expect.poll(server.inFlight).toEqual([TAKEOVER]);
    await storePartitionOfAnotherUser(server);
    const before = server.log.started.length;

    server.release();

    // The refused open has no writer, so the page shows no takeover button.
    await expect.poll(() => ({ save: workspace.shown().save, takeOver: takeOver(), inFlight: server.inFlight() }))
      .toEqual({ save: "needs_attention", takeOver: null, inFlight: [] });
    expect({ check: checkReads(server.log.started.slice(before)), unexpected: server.log.unexpected })
      .toEqual({ check: [SESSION_READ, TREE, snapshotRead(TAKEOVER_SNAPSHOT), SESSION_READ], unexpected: [] });
  });

it("refuses the open of a switch with writer generation 2 when the Journal holds a partition of another User",
  async () => {
    const server = fakeServer({ reads: [...SWITCH_READS, snapshotRead(EDITOR_SNAPSHOT)],
      writer: { kind: "current_writer", writer_generation: "2" }, commands: [SWITCH] });
    const workspace = await openSecondGeneration(server);
    await storePartitionOfAnotherUser(server);
    const before = server.log.started.length;

    chapterB(workspace.root)!.click();

    await expect.poll(() => workspace.shown()).toEqual({ title: "Chapter B", save: "needs_attention", failure: "" });
    await expect.poll(server.inFlight).toEqual([]);
    expect({ check: checkReads(server.log.started.slice(server.log.started.indexOf(SWITCH, before))),
      unexpected: server.log.unexpected }).toEqual({ check: [SESSION_READ], unexpected: [] });
  });

it("runs the takeover check at the open of a switch with writer generation 2 when the Journal lost the partition",
  async () => {
    const server = fakeServer({ reads: [...SWITCH_READS, snapshotRead(EDITOR_SNAPSHOT), snapshotRead(SWITCH_SNAPSHOT)],
      writer: { kind: "current_writer", writer_generation: "2" }, commands: [SWITCH] });
    const workspace = await openSecondGeneration(server);
    // For example, the browser removed the site data while the page stayed open.
    const journal = await requestResult(indexedDB.open(server.scenario.journalName)) as IDBDatabase;
    const transaction = journal.transaction(["partitions", "metadata"], "readwrite");
    const [stored] = await requestResult(transaction.objectStore("partitions").getAll()) as
      { journal_partition_id: string }[];
    await requestResult(transaction.objectStore("partitions").delete(stored!.journal_partition_id));
    await requestResult(transaction.objectStore("metadata").delete(`active_base:${stored!.journal_partition_id}`));
    journal.close();
    const before = server.log.started.length;

    chapterB(workspace.root)!.click();

    // The open is the open of the base commit: the three reads, then the Snapshot stage and the partition again.
    await expect.poll(() => ({ ...workspace.shown(), inFlight: server.inFlight() }))
      .toEqual({ title: "Chapter B", save: "saved", failure: "", inFlight: [] });
    expect({ check: checkReads(server.log.started.slice(server.log.started.indexOf(SWITCH, before))),
      unexpected: server.log.unexpected }).toEqual({ check: [SESSION_READ, TREE, snapshotRead(SWITCH_SNAPSHOT),
      SESSION_READ], unexpected: [] });
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
  { command: "a Chapter switch", name: SWITCH, title: "Chapter B", revision: REVISION_B },
  { command: "an applied acceptance", name: ACCEPTANCE, title: "Chapter A", revision: ACCEPTED_REVISION },
  { command: "a rejection", name: REJECTION, title: "Chapter A", revision: REVISION },
  { command: "a takeover", name: TAKEOVER, title: "Chapter A", revision: REVISION },
])("keeps the contents list and the assistant panel on the screen through $command", async ({ name, title,
  revision }) => {
  const takeover = name === TAKEOVER;
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, PROPOSAL_READ, chapterRead(CHAPTER_B),
    openProposalsRead(CHAPTER_B), snapshotRead(TAKEOVER_SNAPSHOT), CONVERSATION_READ, RUN_READ, SOURCES_READ],
  commands: [SESSION_CREATE, name], proposals: name === ACCEPTANCE || name === REJECTION, acceptance: "applied",
  assistant: "available",
  ...(takeover ? { writer: LOST_WRITER }
    : {}) });
  // The address opens a Conversation in the assistant panel.
  const workspace = await openWorkspace(server, `?conversation=${CONVERSATION}`);
  const parts = workspaceParts(workspace.root);
  const conversation = () => [...workspace.root.querySelectorAll("[data-assistant-current-run] .assistant-author-message, "
    + "[data-assistant-current-run] .assistant-result")].map((line) => line.textContent);
  const opened = ["Help with this passage.", "Keep the voice of Chapter A."];
  await expect.poll(() => ({ availability: parts.panel()?.getAttribute("data-assistant-availability"),
    chapters: parts.chapters(), conversation: conversation() }))
    .toEqual({ availability: "available", chapters: 2, conversation: opened });
  const control = () => workspace.root.querySelector<HTMLButtonElement>(name === SWITCH
    ? `button[data-chapter-id="${CHAPTER_B}"]` : takeover ? "button[data-take-over-writer]"
    : `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[${name === ACCEPTANCE
      ? "data-proposal-accept" : "data-proposal-reject"}]`);
  await expect.poll(() => control()?.disabled).toBe(false);
  await expect.poll(server.inFlight).toEqual([]);
  const panel = parts.panel();
  typeMessage(parts.message()!, "Keep the narrator voice.");
  const watch = watchWorkspace(workspace.root);
  const before = server.log.started.length;
  const shownRevision = () => workspace.root.querySelector("[data-authoritative-revision-id]")
    ?.getAttribute("data-authoritative-revision-id");
  const done = () => workspace.shown().title === title && shownRevision() === revision
    && workspace.root.querySelector(".block-proposal") === null
    && workspace.root.querySelector("button[data-take-over-writer]") === null;
  // The fake holds each request from the click to the new view, so the observer sees the page at each step.
  server.holdEach(true);

  control()!.click();
  for (;;) {
    await expect.poll(() => server.held() > 0 || (done() && server.inFlight().length === 0)).toBe(true);
    if (server.held() === 0) break;
    server.release();
  }
  server.holdEach(false);
  await expect.poll(server.inFlight).toEqual([]);
  watch.stop();

  expect({ ...workspace.shown(), steps: server.log.started.length - before > 1, gaps: watch.gaps,
    panel: parts.panel() === panel, message: parts.message()?.value, conversation: conversation(),
    assistance: server.count(ASSISTANCE), skills: server.count(SKILLS), conversations: server.count(CONVERSATION_READ),
    unexpected: server.log.unexpected }).toEqual({ title, save: "saved", failure: "", steps: true, gaps: [],
    panel: true, message: "Keep the narrator voice.", conversation: opened, assistance: 1, skills: 1, conversations: 1,
    unexpected: [] });
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

it("keeps the tree of a rename that the author makes while a takeover opens the Project again", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT)],
    writer: LOST_WRITER,
    commands: [SESSION_CREATE, TAKEOVER, RENAME] });
  const workspace = await openWorkspace(server);
  const button = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
  const title = () => workspace.root.querySelector(`.tree-panel button[data-chapter-id="${CHAPTER}"]`)?.textContent;
  await expect.poll(() => button()?.disabled).toBe(false);
  // The reopen read the tree. It waits for its Snapshot read, before its last Editor Session read.
  server.holdAfter(TAKEOVER, [snapshotRead(TAKEOVER_SNAPSHOT)]);
  button()!.click();
  await expect.poll(server.inFlight).toEqual([snapshotRead(TAKEOVER_SNAPSHOT)]);

  await beginTreeAction(workspace.root, `li[data-chapter-id="${CHAPTER}"]`, "[data-begin-rename-chapter]");
  workspace.root.querySelector<HTMLButtonElement>("[data-begin-rename-chapter]")!.click();
  await expect.poll(() => workspace.root.querySelector(`form[data-rename-chapter="${CHAPTER}"] input`)).not.toBeNull();
  const input = workspace.root.querySelector<HTMLInputElement>(`form[data-rename-chapter="${CHAPTER}"] input`)!;
  input.value = "Chapter A, renamed";
  input.form!.requestSubmit();
  await expect.poll(title).toBe("Chapter A, renamed");
  server.release();

  await expect.poll(() => ({ ...workspace.shown(), button: button() }))
    .toEqual({ title: "Chapter A", save: "saved", failure: "", button: null });
  await expect.poll(server.inFlight).toEqual([]);
  expect({ title: title(), unexpected: server.log.unexpected })
    .toEqual({ title: "Chapter A, renamed", unexpected: [] });
});

const projectHeading = (root: HTMLElement) => root.querySelector(".project-heading h1")?.textContent;

/** Sends a rename of the Project from the workspace heading. */
async function submitProjectRename(root: HTMLElement, title: string) {
  await beginTreeAction(root, "div[data-project-id]", "[data-begin-rename-project]");
  root.querySelector<HTMLButtonElement>("[data-begin-rename-project]")!.click();
  await expect.poll(() => root.querySelector("form[data-rename] input")).not.toBeNull();
  const input = root.querySelector<HTMLInputElement>("form[data-rename] input")!;
  input.value = title;
  input.form!.requestSubmit();
}

/** Renames the Project in the workspace heading and waits for the new title. */
async function renameProject(root: HTMLElement, title: string) {
  await submitProjectRename(root, title);
  await expect.poll(() => projectHeading(root)).toBe(title);
}

it("keeps the title and the revision of a Project rename that settles while a takeover opens the Project again",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT)],
      writer: LOST_WRITER,
      commands: [SESSION_CREATE, TAKEOVER, PROJECT_RENAME] });
    const workspace = await openWorkspace(server);
    const button = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
    await expect.poll(() => button()?.disabled).toBe(false);
    // The Project read and the list read of the reopen have the answer from before the rename.
    server.answerBeforeHold([PROJECT_READ, LIST]);
    server.holdAfter(TAKEOVER, [PROJECT_READ, LIST]);
    button()!.click();
    await expect.poll(server.inFlight).toEqual([LIST, PROJECT_READ].sort());

    await renameProject(workspace.root, "Project A, renamed");
    server.release();
    await expect.poll(() => ({ ...workspace.shown(), button: button() }))
      .toEqual({ title: "Chapter A", save: "saved", failure: "", button: null });
    const shown = projectHeading(workspace.root);
    await renameProject(workspace.root, "Project A, renamed again");

    expect({ shown, revisions: server.projectRenames, unexpected: server.log.unexpected })
      .toEqual({ shown: "Project A, renamed", revisions: ["1", "2"], unexpected: [] });
  });

it("keeps the title of a Project rename when a reopen brings the earlier title with the new revision", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT)],
    writer: LOST_WRITER,
    commands: [SESSION_CREATE, TAKEOVER, PROJECT_RENAME] });
  const workspace = await openWorkspace(server);
  const button = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
  await expect.poll(() => button()?.disabled).toBe(false);
  // The Project read of the reopen has the title from before the rename, and its list read has the new revision.
  server.answerBeforeHold([PROJECT_READ]);
  server.holdAfter(TAKEOVER, [PROJECT_READ, LIST]);
  button()!.click();
  await expect.poll(server.inFlight).toEqual([LIST, PROJECT_READ].sort());

  await renameProject(workspace.root, "Project A, renamed");
  server.release();
  await expect.poll(() => ({ ...workspace.shown(), button: button() }))
    .toEqual({ title: "Chapter A", save: "saved", failure: "", button: null });
  const shown = projectHeading(workspace.root);
  await renameProject(workspace.root, "Project A, renamed again");

  expect({ shown, revisions: server.projectRenames, unexpected: server.log.unexpected })
    .toEqual({ shown: "Project A, renamed", revisions: ["1", "2"], unexpected: [] });
});

it("keeps the title and the revision of a Project rename that settles while a Proposal link opens the Chapter",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(SWITCH_SNAPSHOT), PROPOSAL_READ,
      CONVERSATION_READ, RUN_READ, SOURCES_READ], commands: [SESSION_CREATE, SWITCH, PROJECT_RENAME],
    assistant: "available", proposalResult: true });
    const workspace = await openWorkspace(server, `?conversation=${CONVERSATION}`);
    const link = () => workspace.root.querySelector<HTMLButtonElement>("button[data-proposal-location]");
    await expect.poll(() => link()?.disabled).toBe(false);
    // The last open of the Proposal link has the Project read and the list read from before the rename.
    server.answerBeforeHold([PROJECT_READ, LIST]);
    server.holdAfter(SWITCH, [PROJECT_READ, LIST]);
    link()!.click();
    await expect.poll(server.inFlight).toEqual([LIST, PROJECT_READ].sort());

    await renameProject(workspace.root, "Project A, renamed");
    const released = server.log.started.length;
    server.release();
    // The new editor part reads the open suggestions of the Chapter after the new view arrives.
    await expect.poll(() => server.log.started.slice(released).includes(openProposalsRead(CHAPTER))).toBe(true);
    const shown = projectHeading(workspace.root);
    await renameProject(workspace.root, "Project A, renamed again");

    expect({ shown, revisions: server.projectRenames, unexpected: server.log.unexpected })
      .toEqual({ shown: "Project A, renamed", revisions: ["1", "2"], unexpected: [] });
  });

it("takes the title and the revision of a reopen after a Project rename in another tab", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, chapterRead(CHAPTER_B), openProposalsRead(CHAPTER_B),
    SESSION_READ], commands: [SESSION_CREATE, SWITCH, PROJECT_RENAME] });
  const workspace = await openWorkspace(server);
  await expect.poll(() => workspace.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
  server.renameElsewhere("Project A, other tab");

  workspace.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();
  // The Arriving Chapter shows its title before the reopen (ADR 0105). An empty fact of the arrival comes only after the
  // reopen, so the test reads the Project heading after the reopen.
  await expect.poll(() => ({ title: workspace.shown().title, arrival: workspace.root
    .querySelector("[data-arrival-no-open-suggestion]")?.getAttribute("data-arrival-no-open-suggestion") }))
    .toEqual({ title: "Chapter B", arrival: "" });
  const shown = projectHeading(workspace.root);
  await renameProject(workspace.root, "Project A, renamed");

  expect({ shown, revisions: server.projectRenames, unexpected: server.log.unexpected })
    .toEqual({ shown: "Project A, other tab", revisions: ["2"], unexpected: [] });
});

it("keeps the later title and revision of a reopen when a rename response of an earlier revision arrives late",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, chapterRead(CHAPTER_B), openProposalsRead(CHAPTER_B),
      SESSION_READ], commands: [SESSION_CREATE, SWITCH, PROJECT_RENAME] });
    const workspace = await openWorkspace(server);
    await expect.poll(() => workspace.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
    // The rename applies at revision 2, and its response waits. Then another tab renames the Project to revision 3.
    server.answerBeforeHold([PROJECT_RENAME]);
    server.hold([PROJECT_RENAME]);
    await submitProjectRename(workspace.root, "Project A, renamed");
    await expect.poll(server.held).toBe(1);
    server.renameElsewhere("Project A, other tab");
    workspace.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();
    // The reopen of the switch has revision 3. The rename form stays open until the rename response.
    await expect.poll(() => workspace.shown().title).toBe("Chapter B");

    server.release();
    await expect.poll(() => workspace.root.querySelector("form[data-rename]")).toBeNull();
    const shown = projectHeading(workspace.root);
    await renameProject(workspace.root, "Project A, renamed again");

    expect({ shown, revisions: server.projectRenames, unexpected: server.log.unexpected })
      .toEqual({ shown: "Project A, other tab", revisions: ["1", "3"], unexpected: [] });
  });

it("reads the statistics one time for the new view of a Chapter switch with no change of the tree revision",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, chapterRead(CHAPTER_B), openProposalsRead(CHAPTER_B),
      SESSION_READ], commands: [SESSION_CREATE, SWITCH] });
    const workspace = await openWorkspace(server);
    await expect.poll(() => server.count(STATISTICS)).toBe(1);
    await expect.poll(() => workspace.root.querySelector(`button[data-chapter-id="${CHAPTER_B}"]`)).not.toBeNull();
    // The tree read of the new view waits, so that a statistics read before its end shows.
    server.holdAfter(chapterRead(CHAPTER_B), [TREE]);
    workspace.root.querySelector<HTMLButtonElement>(`button[data-chapter-id="${CHAPTER_B}"]`)!.click();
    await expect.poll(() => ({ title: workspace.shown().title, held: server.held() }))
      .toEqual({ title: "Chapter B", held: 1 });
    const from = server.log.events.length;
    server.release();

    // The statistics read of the new view ends after the end of its tree read.
    await expect.poll(() => {
      const events = server.log.events.slice(from);
      const tree = events.indexOf(`end ${TREE}`);
      return tree >= 0 && events.indexOf(`end ${STATISTICS}`, tree) > tree;
    }).toBe(true);
    const events = server.log.events.slice(server.log.events.indexOf(`end ${chapterRead(CHAPTER_B)}`));
    expect({ statistics: events.filter((event) => event === `start ${STATISTICS}`).length,
      tree: server.count(TREE), unexpected: server.log.unexpected })
      .toEqual({ statistics: 1, tree: 2, unexpected: [] });
  });

it("keeps the tree of a rename that the author makes while a Proposal link opens the Chapter", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT),
    snapshotRead(SWITCH_SNAPSHOT), PROPOSAL_READ, CONVERSATION_READ, RUN_READ, SOURCES_READ],
  writer: LOST_WRITER,
  commands: [SESSION_CREATE, TAKEOVER, SWITCH, RENAME], assistant: "available", proposalResult: true });
  const workspace = await openWorkspace(server, `?conversation=${CONVERSATION}`);
  const link = () => workspace.root.querySelector<HTMLButtonElement>("button[data-proposal-location]");
  const title = () => workspace.root.querySelector(`.tree-panel button[data-chapter-id="${CHAPTER}"]`)?.textContent;
  // A takeover gives writer generation 2, so each open reads the tree in its takeover check.
  await expect.poll(() => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]")?.disabled)
    .toBe(false);
  workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]")!.click();
  await expect.poll(() => ({ ...workspace.shown(), link: link()?.disabled }))
    .toEqual({ title: "Chapter A", save: "saved", failure: "", link: false });
  await expect.poll(server.inFlight).toEqual([]);
  // The open after the switch read the tree. It waits for its Snapshot read, before its last Editor Session read.
  server.holdAfter(SWITCH, [snapshotRead(SWITCH_SNAPSHOT)]);
  link()!.click();
  await expect.poll(server.inFlight).toEqual([snapshotRead(SWITCH_SNAPSHOT)]);

  await beginTreeAction(workspace.root, `li[data-chapter-id="${CHAPTER}"]`, "[data-begin-rename-chapter]");
  workspace.root.querySelector<HTMLButtonElement>("[data-begin-rename-chapter]")!.click();
  await expect.poll(() => workspace.root.querySelector(`form[data-rename-chapter="${CHAPTER}"] input`)).not.toBeNull();
  const input = workspace.root.querySelector<HTMLInputElement>(`form[data-rename-chapter="${CHAPTER}"] input`)!;
  input.value = "Chapter A, renamed";
  input.form!.requestSubmit();
  await expect.poll(title).toBe("Chapter A, renamed");
  const released = server.log.started.length;
  server.release();

  // The new editor part reads the open suggestions of the Chapter after the new view arrives.
  await expect.poll(() => server.log.started.slice(released).includes(openProposalsRead(CHAPTER))).toBe(true);
  await expect.poll(server.inFlight).toEqual([]);
  expect({ ...workspace.shown(), tree: title(), unexpected: server.log.unexpected })
    .toEqual({ title: "Chapter A", save: "saved", failure: "", tree: "Chapter A, renamed", unexpected: [] });
});

it("shows that the takeover runs, and sends one takeover for two clicks", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT)],
    writer: LOST_WRITER,
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
    commands: [SESSION_CREATE, TAKEOVER], takeover: "compare_failed" });
  const workspace = await openWorkspace(server);
  const button = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
  await expect.poll(() => button()?.disabled).toBe(false);

  // The Server refuses the takeover: another tab took the writer after the admission.
  button()!.click();

  await expect.poll(() => ({ alert: workspace.root.querySelector('.editor-panel [role="alert"]')?.textContent,
    disabled: button()?.disabled, text: button()?.textContent }))
    .toEqual({ alert: "无法接管写作。", disabled: false, text: "接管写作" });
  expect(server.log.unexpected).toEqual([]);
});

it("reads only the suggestions after a rejection", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, OTHER_READ], commands: [SESSION_CREATE, REJECTION],
    proposals: true, other: true });
  const workspace = await openWorkspace(server);
  const card = () => workspace.root.querySelector(`.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"]`);
  const otherCard = () => workspace.root.querySelector(`.block-proposal[data-proposal-id="${OTHER.proposal_id}"]`);
  const reject = () => card()?.querySelector<HTMLButtonElement>("button[data-proposal-reject]");
  await expect.poll(() => ({ reject: reject()?.disabled, other: otherCard() !== null }))
    .toEqual({ reject: false, other: true });
  const before = server.log.started.length;

  reject()!.click();

  await expect.poll(card).toBeNull();
  await expect.poll(server.inFlight).toEqual([]);
  const later = server.log.started.slice(server.log.started.indexOf(REJECTION, before) + 1);
  // The Chapter text does not change, so the page reads the open suggestions again: the list and each Proposal. It
  // reads no Project, list of Projects, Chapter, Editor Session, or tree. The other suggestion stays.
  expect({ ...workspace.shown(), later: [...new Set(later.filter((entry) => entry !== CHALLENGE
    && entry !== STATISTICS))].sort(), other: otherCard() !== null,
    unexpected: server.log.unexpected }).toEqual({ title: "Chapter A", save: "saved", failure: "",
    later: [openProposalsRead(CHAPTER), PROPOSAL_READ, OTHER_READ].sort(), other: true, unexpected: [] });
});

it("opens the Project again after a rejection that found a changed Chapter Head", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ], commands: [SESSION_CREATE, REJECTION],
    proposals: true, rejection: "conflicted" });
  const workspace = await openWorkspace(server);
  const reject = () => workspace.root.querySelector<HTMLButtonElement>(
    `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-reject]`);
  await expect.poll(() => reject()?.disabled).toBe(false);
  const before = server.log.started.length;
  const steps = await expectReopenSteps(server, REJECTION, CHAPTER);

  // Another tab changed the Chapter text, so the Server finds a changed Chapter Head.
  reject()!.click();
  await steps();

  const revision = () => workspace.root.querySelector("[data-authoritative-revision-id]")
    ?.getAttribute("data-authoritative-revision-id");
  await expect.poll(() => ({ ...workspace.shown(), revision: revision() }))
    .toEqual({ title: "Chapter A", save: "saved", failure: "", revision: EDITED_REVISION });
  expect({ open: openReads(server.log.started.slice(before)).sort(), unexpected: server.log.unexpected })
    .toEqual({ open: [LIST, PROJECT_READ].sort(), unexpected: [] });
});

it("opens the Project again after a rejection that the Server refused because another tab applied it", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ], commands: [SESSION_CREATE, REJECTION],
    proposals: true, rejection: "refused" });
  const workspace = await openWorkspace(server);
  const reject = () => workspace.root.querySelector<HTMLButtonElement>(
    `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-reject]`);
  await expect.poll(() => reject()?.disabled).toBe(false);
  const before = server.log.started.length;

  reject()!.click();

  // The page shows the current Chapter text, which has the applied Operation.
  const revision = () => workspace.root.querySelector("[data-authoritative-revision-id]")
    ?.getAttribute("data-authoritative-revision-id");
  await expect.poll(() => ({ ...workspace.shown(), revision: revision() }))
    .toEqual({ title: "Chapter A", save: "saved", failure: "", revision: ACCEPTED_REVISION });
  await expect.poll(server.inFlight).toEqual([]);
  expect({ open: openReads(server.log.started.slice(before)).sort(), unexpected: server.log.unexpected })
    .toEqual({ open: [LIST, PROJECT_READ].sort(), unexpected: [] });
});

/** The reads after a settled command start together in an order that changes, so a test compares them sorted. */
const sorted = (entries: string[]) => [...entries].sort();
/** The reads of a reopen after a decision: the Chapter, the Project, the list, and the panels. */
const REOPEN_AFTER_DECISION = [chapterRead(CHAPTER), PROJECT_READ, LIST, TREE, `GET ${P}/activity`, STATISTICS];

it.each([
  // After the command, the requests are those of today (#1900 removes only the read before the command).
  { decision: "acceptance", command: ACCEPTANCE, button: "accept", reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ],
    settled: (root: HTMLElement) => root.querySelector("[data-authoritative-revision-id]")
      ?.getAttribute("data-authoritative-revision-id") === ACCEPTED_REVISION,
    after: [...REOPEN_AFTER_DECISION, openProposalsRead(CHAPTER), PROPOSAL_READ, SESSION_READ, PROPOSAL_READ,
      openProposalsRead(CHAPTER), PROPOSAL_READ] },
  { decision: "rejection", command: REJECTION, button: "reject", reads: [...OPEN_READS, PROPOSAL_READ],
    settled: (root: HTMLElement) => root.querySelector(`.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"]`)
      === null,
    after: [openProposalsRead(CHAPTER), PROPOSAL_READ, PROPOSAL_READ] },
])("sends the $decision of a Proposal Operation with no Proposal read before it", async ({ command, button, reads, settled,
  after }) => {
  const server = fakeServer({ reads, commands: [SESSION_CREATE, command], proposals: true, acceptance: "applied" });
  const workspace = await openWorkspace(server);
  const control = () => workspace.root.querySelector<HTMLButtonElement>(
    `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-${button}]`);
  await expect.poll(() => control()?.disabled).toBe(false);
  await expect.poll(server.inFlight).toEqual([]);
  const from = server.log.started.length;
  server.hold([command]);

  control()!.click();

  // The fake holds the command, so the log has each request from the click to the command.
  await expect.poll(() => server.count(command)).toBe(1);
  expect(server.log.started.slice(from)).toEqual([CHALLENGE, command]);
  server.releaseOnly([command]);
  await expect.poll(() => settled(workspace.root)).toBe(true);
  await expect.poll(server.inFlight).toEqual([]);
  expect({ after: sorted(server.log.started.slice(from + 2)), unexpected: server.log.unexpected })
    .toEqual({ after: sorted(after), unexpected: [] });
});

const REFUSED_REJECTION = "此次拒绝未生效，请检查当前候选文字。";
/** The requests of a refused rejection: the read of the list and the Proposal before the reopen, the reopen, and the
 * read of the list at the end of the decision (decision 6 of #1900), as after a refused acceptance. */
const REFUSED_LATER = [CHALLENGE, REJECTION, openProposalsRead(CHAPTER), PROPOSAL_READ, ...REOPEN_AFTER_DECISION,
  SESSION_READ, openProposalsRead(CHAPTER)];

/** The suggestion card of the Proposal, its status, and the Chapter revision and text that the page shows. */
function proposalView(root: HTMLElement) {
  const card = () => root.querySelector<HTMLElement>(`.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"]`);
  return { card, reject: () => card()?.querySelector<HTMLButtonElement>("button[data-proposal-reject]"),
    status: () => root.querySelector(`[data-proposal-decision="${PROPOSAL.proposal_id}"]`)?.textContent ?? null,
    revision: () => root.querySelector("[data-authoritative-revision-id]")?.getAttribute("data-authoritative-revision-id"),
    // The blocks of the Chapter text, with no suggestion card.
    text: () => [...root.querySelector("[data-manuscript-editor]")?.children ?? []]
      .filter((block) => !block.classList.contains("block-proposal")).map((block) => block.textContent) };
}

it("shows the sentence of a refused rejection of an earlier Proposal revision under the current revision, and the next"
  + " rejection sends the current revision", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ], commands: [SESSION_CREATE, REJECTION],
    proposals: true });
  const workspace = await openWorkspace(server);
  const view = proposalView(workspace.root);
  await expect.poll(() => view.reject()?.disabled).toBe(false);
  await expect.poll(server.inFlight).toEqual([]);
  const shownText = view.text();
  expect(shownText).toEqual(["Base"]);
  // Another tab edits the candidate text: the Proposal has a new revision.
  server.decideElsewhere("revised");
  const from = server.log.started.length;

  view.reject()!.click();

  await expect.poll(() => ({ status: view.status(), revision: view.card()?.getAttribute("data-proposal-revision-id"),
    text: view.card()?.textContent?.includes(REVISED_TEXT),
    earlierText: view.card()?.textContent?.includes(PROPOSAL.candidate_text) })).toEqual({ status: REFUSED_REJECTION,
    revision: REVISED_PROPOSAL.revision_id, text: true, earlierText: false });
  await expect.poll(server.inFlight).toEqual([]);
  const refused = sorted(server.log.started.slice(from));
  expect({ ...workspace.shown(), revision: view.revision(), text: view.text(), status: view.status() })
    .toEqual({ title: "Chapter A", save: "saved", failure: "", revision: REVISION, text: shownText,
      status: REFUSED_REJECTION });
  await expect.poll(() => view.reject()?.disabled).toBe(false);
  view.reject()!.click();

  await expect.poll(view.card).toBeNull();
  expect({ revisions: server.log.rejectedRevisions, refused, unexpected: server.log.unexpected }).toEqual({
    revisions: [PROPOSAL.revision_id, REVISED_PROPOSAL.revision_id], refused: sorted(REFUSED_LATER),
    unexpected: [] });
});

it("shows the sentence of a refused rejection when another tab rejected the Operation, then no card, and keeps the"
  + " Chapter text", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ], commands: [SESSION_CREATE, REJECTION],
    proposals: true });
  const workspace = await openWorkspace(server);
  const view = proposalView(workspace.root);
  await expect.poll(() => view.reject()?.disabled).toBe(false);
  await expect.poll(server.inFlight).toEqual([]);
  const shownText = view.text();
  expect(shownText).toEqual(["Base"]);
  server.decideElsewhere("rejected");
  const from = server.log.started.length;

  view.reject()!.click();

  await expect.poll(() => ({ status: view.status(), card: view.card() })).toEqual({ status: REFUSED_REJECTION,
    card: null });
  await expect.poll(server.inFlight).toEqual([]);
  // With no open suggestion of this Proposal, the view reads it one more time, as after a resolved rejection
  // (decision 5 of #1900).
  expect({ ...workspace.shown(), revision: view.revision(), text: view.text(), status: view.status(),
    later: sorted(server.log.started.slice(from)), unexpected: server.log.unexpected }).toEqual({ title: "Chapter A",
    save: "saved", failure: "", revision: REVISION, text: shownText, status: REFUSED_REJECTION,
    later: sorted([...REFUSED_LATER, PROPOSAL_READ]), unexpected: [] });
});

/** The status of an accepted Proposal whose Acceptance record the Local Edit Journal keeps. */
const UNCONFIRMED_APPLIED = "正文已变化；此次接受结果尚未确认。请重试同一操作。";

/**
 * Waits until the editor part checked the Acceptance records of the Local Edit Journal and no request is in flight. A
 * second open starts before the end of that check, and the status shows only after it.
 */
async function journalChecked(root: HTMLElement, server: ReturnType<typeof fakeServer>) {
  await expect.poll(() => root.querySelector(`[data-proposal-decision="${PROPOSAL.proposal_id}"]`)?.textContent)
    .toBe(`${UNCONFIRMED_APPLIED}重试接受`);
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
  // The settled Acceptance removed its record, so the editor opens for input after the check of the Journal.
  await expect.poll(() => workspace.root.querySelector("[data-manuscript-editor]")?.getAttribute("contenteditable"))
    .toBe("true");
  await expect.poll(server.inFlight).toEqual([]);
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
  await journalChecked(workspace.root, server);
  expect({ open: openReads(server.log.started.slice(before)).sort(), revision: revision(),
    unexpected: server.log.unexpected }).toEqual({ open: [PROTOCOL, LIST, PROJECT_READ].sort(),
    revision: ACCEPTED_REVISION, unexpected: [] });
});

it("opens the Project again when a reload read the Chapter before the Server applied a lost acceptance", async () => {
  const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ], commands: [SESSION_CREATE, ACCEPTANCE],
    proposals: true, acceptance: "late", lost: [ACCEPTANCE] });
  const workspace = await openWorkspace(server);
  const accept = () => workspace.root.querySelector<HTMLButtonElement>(
    `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-accept]`);
  await expect.poll(() => accept()?.disabled).toBe(false);
  accept()!.click();
  // The page cannot know the result, so it opens the Project again and offers the same Acceptance again.
  await expect.poll(() => workspace.root.querySelector(`[data-proposal-decision="${PROPOSAL.proposal_id}"]`)
    ?.textContent).toBe("接受结果暂不可确认。请刷新检查，或重试同一操作。重试接受");
  await expect.poll(server.inFlight).toEqual([]);
  const before = server.log.started.length;

  // The author reloads the page. The Server applies the Acceptance after the reload read the Chapter.
  const reloaded = await reloadWorkspace(server);

  const revision = () => reloaded.querySelector("[data-authoritative-revision-id]")
    ?.getAttribute("data-authoritative-revision-id");
  await expect.poll(revision).toBe(ACCEPTED_REVISION);
  await journalChecked(reloaded, server);
  expect({ open: openReads(server.log.started.slice(before)).sort(), revision: revision(),
    // The reload opens the Project with each check of the first open. The Journal finds the applied Acceptance, which
    // the editor does not show, so the page reopens the Project after it.
    unexpected: server.log.unexpected }).toEqual({ open: [PROTOCOL, LIST, LIST, PROJECT_READ, PROJECT_READ].sort(),
    revision: ACCEPTED_REVISION, unexpected: [] });
});

it.each([
  { event: "no Proposal change", fact: "true" },
  { event: "a new Proposal of a Run", fact: "false" },
  // A refused decision does not change the list of Chapter A, but another tab can have changed a Proposal.
  { event: "a refused decision", fact: "false" },
])("keeps the fact of no open suggestion in the copy of Chapter B only with $event (ADR 0105)", async ({ event, fact }) => {
  const decision = event === "a refused decision";
  const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, PROPOSAL_READ, CONVERSATION_READ, RUN_READ,
    SOURCES_READ, chapterRead(CHAPTER_B), openProposalsRead(CHAPTER_B)],
  commands: [SESSION_CREATE, SWITCH, ...(decision ? [ACCEPTANCE] : [])], proposals: decision, acceptance: "refused",
  assistant: "available", proposalResult: event === "a new Proposal of a Run" });
  // The stay timer of the pointer waits until the test runs it. Each other timer is a real timer.
  const stays: { callback: () => void; cleared: boolean; fired?: boolean }[] = [];
  const timers: ChallengeAdmissionTimers = {
    setTimeoutImpl: (callback, timeout) => timeout === POINTER_STAY_MS
      ? -stays.push({ callback, cleared: false }) : globalThis.setTimeout(callback, timeout),
    clearTimeoutImpl: (timer) => {
      if (typeof timer === "number" && timer < 0) stays[-timer - 1]!.cleared = true;
      else globalThis.clearTimeout(timer);
    },
  };
  server.hold([RUN_READ]);
  const workspace = await openWorkspace(server, decision ? "" : `?conversation=${CONVERSATION}`, timers);
  const chapterB = () => workspace.root.querySelector<HTMLButtonElement>(`.tree-panel button[data-chapter-id="${CHAPTER_B}"]`)!;
  const pointer = (type: "pointerover" | "pointerout") => chapterB().dispatchEvent(new PointerEvent(type, { bubbles: true }));
  // The contents read Chapter B at the pointer only in a tab with a ready editor that holds the writer. A new stay
  // stops the stay before it.
  const waiting = () => stays.filter((timer) => !timer.cleared && timer.fired !== true);
  await expect.poll(() => {
    if (chapterB() !== null) pointer("pointerover");
    return waiting().length;
  }).toBe(1);
  stays.at(-1)!.fired = true;
  stays.at(-1)!.callback();
  pointer("pointerout");
  // A Chapter with a copy starts no stay timer.
  await expect.poll(() => {
    pointer("pointerover");
    const started = stays.filter((timer) => !timer.cleared && timer.fired !== true).length;
    pointer("pointerout");
    return started;
  }).toBe(0);

  if (decision) {
    const accept = () => workspace.root.querySelector<HTMLButtonElement>(
      `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-accept]`);
    await expect.poll(() => accept()?.disabled).toBe(false);
    accept()!.click();
    await expect.poll(() => server.count(ACCEPTANCE) === 1 && accept()?.disabled === false
      && server.inFlight().length === 0).toBe(true);
  } else {
    server.release();
    // The editor reads the Proposal after the assistant panel gives it.
    await expect.poll(() => server.count(event === "no Proposal change" ? RUN_READ : PROPOSAL_READ) === 1
      && server.inFlight().length === 0).toBe(true);
  }
  server.hold([chapterRead(CHAPTER_B)]);
  const activityReads = server.count(ACTIVITY_READ);
  chapterB().click();

  await expect.poll(() => workspace.root.querySelector("[data-arrival-no-open-suggestion]")
    ?.getAttribute("data-arrival-no-open-suggestion")).toBe(fact);
  await expect.poll(() => server.held()).toBe(1);
  server.release();
  await expect.poll(workspace.shown).toEqual({ title: "Chapter B", save: "saved", failure: "" });
  // The Activity read of the new editor part starts after its Journal read, so the next test has the Journal alone.
  await expect.poll(() => server.count(ACTIVITY_READ) > activityReads && server.inFlight().length === 0).toBe(true);
  expect(server.log.unexpected).toEqual([]);
});

it("writes a copy from each Chapter read of a tab without the writer, also from a read that a later selection discards",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT),
      chapterRead(CHAPTER_B), openProposalsRead(CHAPTER_B), chapterRead(CHAPTER_C), openProposalsRead(CHAPTER_C)],
    writer: LOST_WRITER,
    commands: [SESSION_CREATE, TAKEOVER, SWITCH], third: true });
    const workspace = await openWorkspace(server);
    const takeOver = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
    const chapterC = () => workspace.root.querySelector<HTMLButtonElement>(`.tree-panel button[data-chapter-id="${CHAPTER_C}"]`);
    const chapterA = () => workspace.root.querySelector<HTMLButtonElement>(`.tree-panel button[data-chapter-id="${CHAPTER}"]`);
    await expect.poll(() => [chapterB(workspace.root)?.disabled, chapterC()?.disabled]).toEqual([false, false]);
    // The read of Chapter B answers after the selection of Chapter C, so the view discards it.
    server.hold([chapterRead(CHAPTER_B)]);
    chapterB(workspace.root)!.click();
    await expect.poll(() => server.held()).toBe(1);
    chapterC()!.click();
    await expect.poll(() => workspace.shown().title).toBe("Chapter C");
    server.release();
    await expect.poll(() => server.log.consumed.includes(chapterRead(CHAPTER_B))).toBe(true);
    await expect.poll(() => [workspace.shown().title, server.inFlight()]).toEqual(["Chapter C", []]);
    await expect.poll(takeOver).not.toBeNull();
    chapterA()!.click();
    await expect.poll(() => [workspace.shown().title, server.inFlight()]).toEqual(["Chapter A", []]);
    // The takeover keeps the shown Chapter A (ADR 0106 section 1).
    takeOver()!.click();
    await expect.poll(() => ({ ...workspace.shown(), takeOver: takeOver(), inFlight: server.inFlight() }))
      .toEqual({ title: "Chapter A", save: "saved", failure: "", takeOver: null, inFlight: [] });
    server.hold([chapterRead(CHAPTER_B)]);
    const activityReads = server.count(ACTIVITY_READ);

    chapterB(workspace.root)!.click();

    await expect.poll(() => [workspace.root.querySelector(".editor-panel h2")?.textContent,
      workspace.root.querySelector("[data-manuscript-editor]")?.textContent]).toEqual(["Chapter B", CHAPTER_B_BODY]);
    await expect.poll(() => server.held()).toBe(1);
    server.release();
    await expect.poll(workspace.shown).toEqual({ title: "Chapter B", save: "saved", failure: "" });
    await expect.poll(() => server.count(ACTIVITY_READ) > activityReads && server.inFlight().length === 0).toBe(true);
    expect(server.log.unexpected).toEqual([]);
  });

it("writes no copy from a Chapter read that answers after a later read of the same Chapter started (ADR 0105)",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, SESSION_READ, snapshotRead(TAKEOVER_SNAPSHOT),
      chapterRead(CHAPTER_B), openProposalsRead(CHAPTER_B), chapterRead(CHAPTER_C), openProposalsRead(CHAPTER_C)],
    writer: LOST_WRITER,
    commands: [SESSION_CREATE, TAKEOVER, SWITCH], third: true });
    const workspace = await openWorkspace(server);
    const takeOver = () => workspace.root.querySelector<HTMLButtonElement>("button[data-take-over-writer]");
    const chapterC = () => workspace.root.querySelector<HTMLButtonElement>(`.tree-panel button[data-chapter-id="${CHAPTER_C}"]`);
    const chapterA = () => workspace.root.querySelector<HTMLButtonElement>(`.tree-panel button[data-chapter-id="${CHAPTER}"]`);
    const screen = () => [workspace.root.querySelector(".editor-panel h2")?.textContent,
      workspace.root.querySelector("[data-manuscript-editor]")?.textContent];
    await expect.poll(() => [chapterB(workspace.root)?.disabled, chapterC()?.disabled]).toEqual([false, false]);
    // The first read of Chapter B has the text of an earlier Revision. It answers after a later read of Chapter B.
    server.earlierChapterB(true);
    server.answerBeforeHold([chapterRead(CHAPTER_B)]);
    server.hold([chapterRead(CHAPTER_B)]);
    chapterB(workspace.root)!.click();
    await expect.poll(() => server.held()).toBe(1);
    server.earlierChapterB(false);
    chapterC()!.click();
    await expect.poll(() => workspace.shown().title).toBe("Chapter C");
    chapterB(workspace.root)!.click();
    await expect.poll(screen).toEqual(["Chapter B", CHAPTER_B_BODY]);
    server.release();
    await expect.poll(() => [server.log.consumed.filter((name) => name === chapterRead(CHAPTER_B)).length,
      server.inFlight()]).toEqual([2, []]);
    chapterA()!.click();
    await expect.poll(() => [workspace.shown().title, server.inFlight()]).toEqual(["Chapter A", []]);
    // The takeover keeps the shown Chapter A (ADR 0106 section 1).
    takeOver()!.click();
    await expect.poll(() => ({ ...workspace.shown(), takeOver: takeOver(), inFlight: server.inFlight() }))
      .toEqual({ title: "Chapter A", save: "saved", failure: "", takeOver: null, inFlight: [] });
    server.hold([chapterRead(CHAPTER_B)]);
    const activityReads = server.count(ACTIVITY_READ);

    chapterB(workspace.root)!.click();

    await expect.poll(() => [...screen(), server.held()]).toEqual(["Chapter B", CHAPTER_B_BODY, 1]);
    server.release();
    await expect.poll(workspace.shown).toEqual({ title: "Chapter B", save: "saved", failure: "" });
    await expect.poll(() => server.count(ACTIVITY_READ) > activityReads && server.inFlight().length === 0).toBe(true);
    expect(server.log.unexpected).toEqual([]);
  });



it("removes the fact of no open suggestion when a reload finds an applied Acceptance that the editor does not show",
  async () => {
    const server = fakeServer({ reads: [...OPEN_READS, PROPOSAL_READ, SESSION_READ, chapterRead(CHAPTER_B),
      openProposalsRead(CHAPTER_B)], commands: [SESSION_CREATE, ACCEPTANCE], proposals: true,
    acceptance: "late", lost: [ACCEPTANCE] });
    const workspace = await openWorkspace(server);
    const accept = () => workspace.root.querySelector<HTMLButtonElement>(
      `.block-proposal[data-proposal-id="${PROPOSAL.proposal_id}"] button[data-proposal-accept]`);
    await expect.poll(() => accept()?.disabled).toBe(false);
    accept()!.click();
    await expect.poll(() => workspace.root.querySelector(`[data-proposal-decision="${PROPOSAL.proposal_id}"]`)
      ?.textContent).toBe("接受结果暂不可确认。请刷新检查，或重试同一操作。重试接受");
    await expect.poll(server.inFlight).toEqual([]);
    // The reload waits for its Proposal read until the copy of Chapter B has the fact. The Server applies the
    // Acceptance at that read, and the list of Chapter A answers after it: the list is empty before and after the
    // reopen, so only the recovery of the Acceptance can remove the fact.
    server.hold([PROPOSAL_READ, openProposalsRead(CHAPTER)]);
    const reloaded = await reloadWorkspace(server);
    const button = () => reloaded.querySelector<HTMLButtonElement>(`.tree-panel button[data-chapter-id="${CHAPTER_B}"]`);
    await expect.poll(() => [button()?.disabled, server.inFlight()])
      .toEqual([false, [PROPOSAL_READ, openProposalsRead(CHAPTER)].sort()]);
    // The stay timer of the pointer is a real timer here.
    button()!.dispatchEvent(new PointerEvent("pointerover", { bubbles: true }));
    await expect.poll(() => server.log.consumed.includes(openProposalsRead(CHAPTER_B))
      && server.log.consumed.includes(chapterRead(CHAPTER_B))).toBe(true);
    button()!.dispatchEvent(new PointerEvent("pointerout", { bubbles: true }));
    server.releaseOnly([PROPOSAL_READ]);
    const revision = () => reloaded.querySelector("[data-authoritative-revision-id]")
      ?.getAttribute("data-authoritative-revision-id");
    await expect.poll(revision).toBe(ACCEPTED_REVISION);
    server.release();
    await journalChecked(reloaded, server);
    // The Acceptance waits for a retry, so the quiet step of a selection refuses. Until then the screen shows the
    // Arriving Chapter B and its fact.
    const facts: string[] = [];
    const observer = new MutationObserver(() => {
      const fact = reloaded.querySelector("[data-arrival-no-open-suggestion]")
        ?.getAttribute("data-arrival-no-open-suggestion") ?? "";
      if (fact !== "" && facts.at(-1) !== fact) facts.push(fact);
    });
    observer.observe(reloaded, { subtree: true, childList: true, attributes: true });

    button()!.click();

    await expect.poll(() => reloaded.querySelector('.editor-panel [role="alert"]')?.textContent)
      .toBe("无法切换章节：请先完成当前输入。");
    observer.disconnect();
    expect({ facts, inFlight: server.inFlight() }).toEqual({ facts: ["false"], inFlight: [] });
    expect(server.log.unexpected).toEqual([]);
  });
