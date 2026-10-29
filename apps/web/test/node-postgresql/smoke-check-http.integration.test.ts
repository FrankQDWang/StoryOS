// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { afterAll, test } from "vitest";

import * as client from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateEditorSessionResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { bootProtectedWebClient } from "../../src/boot.ts";
import { BINDING, USER_A, addVolume, createProject, createStory, drainLeftoverWork, freshNamespace, id, openProposal,
  problemOf, serverForFile, settleOnce,
} from "../support/http-harness.ts";
import type { CommandInput, ProjectCommandId, TestProject, TestStory } from "../support/http-harness.ts";
import { queryStoryOSPostgres } from "../support/node-integration.ts";

// The Smoke Check table (ADR 0075). Each row proves only that its route answers: the route exists, a request with no
// session is refused, a project command with no Command Challenge proof is refused, and the response has the declared
// schema. A row expects a declared refusal only when a Profile Probe or a kept file proves the success path. A read that
// is bound to the latest Project Snapshot also proves its expired-Snapshot answer.
const server = serverForFile();
// The Worker claims export work before AgentRun work, so the table settles its exports before a later file runs.
afterAll(async () => {
  for (let attempt = 0; attempt < 16 && await unsettledExports() !== "0"; attempt += 1) await settleOnce();
  assert.equal(await unsettledExports(), "0");
  await drainLeftoverWork();
});

const CATALOG = (JSON.parse(readFileSync(new URL(
  "../../../../generated/route-catalog/versioned-protocol-release-1-route-catalog.json", import.meta.url), "utf8")) as {
  operations: { operation_id: string; schemas: { response: string } }[];
}).operations;
const RESPONSE_SCHEMA = new Map(CATALOG.map((operation) => [operation.operation_id, operation.schemas.response]));
const NONCE = /^[0-9a-f]{64}$/;

// Fixtures

function unsettledExports(): Promise<string> {
  return queryStoryOSPostgres(`
    SELECT (SELECT count(*) FROM storyos.project_export_operations WHERE settled_result IS NULL)
         + (SELECT count(*) FROM storyos.human_readable_manuscript_export_operations WHERE settled_result IS NULL)`);
}

function digest(text: string): string {
  return createHash("sha256").update(text).digest("hex");
}

async function chapter(story: TestStory, index = 0) {
  return (await client.getChapter({ baseUrl: story.baseUrl, projectId: story.projectId,
    chapterId: story.chapterIds[index] ?? "", fetchImpl: story.fetchImpl })).chapter;
}

/** A Project with an open Proposal on its first Chapter and a writer Editor Session. A streamed Proposal has a
 * Proposal Generation. */
async function proposed(title: string, options: { streamed?: true } = {}) {
  await drainLeftoverWork();
  const story = await createStory(server, { title, assistance: true });
  const target = (await chapter(story)).current_revision.revision_id;
  const proposal = options.streamed === true ? await streamedProposal(story) : (await openProposal(story)).proposal;
  const session = await story.openEditorSession();
  return { story, target, proposal, editorSessionId: session.editor_session.editor_session_id };
}

type Proposed = Awaited<ReturnType<typeof proposed>>;

async function streamedProposal(story: TestStory) {
  const input = agentRun(story);
  input.request.create_agent_run_input.author_message = { text: "Stream this passage: keep the voice." };
  const run = await story.send("createAgentRun", input);
  if (run.response.effect.kind !== "admitted") throw new Error("expected an admitted AgentRun");
  await drainLeftoverWork();
  const settled = await client.getAgentRun({ ...scope(story), fetchImpl: story.fetchImpl,
    runId: run.response.effect.run_id });
  if (settled.decision.kind !== "prose_change" || settled.decision.opened_proposal.kind !== "present") {
    throw new Error("expected an opened Proposal");
  }
  return (await client.getProposal({ ...scope(story), fetchImpl: story.fetchImpl,
    proposalId: settled.decision.opened_proposal.proposal_id })).proposal;
}

function rejection({ story, target, proposal, editorSessionId }: Proposed): CommandInput<"rejectProposalOperations"> {
  return { request: {
    command_schema: "storyos.command.reject-proposal-operations.request.v1",
    reject_proposal_operations_input: { proposal_revision_id: proposal.revision_id,
      selected_pending_operation_ids: [proposal.operation_id], expected_target_revisions: [target],
      rejection_reason: { kind: "author_declined", note: { kind: "omitted" } }, editor_session_id: editorSessionId,
      ...story.binding() },
  }, path: { proposalId: proposal.proposal_id } };
}

function withdrawal({ story, target, proposal, editorSessionId }: Proposed): CommandInput<"withdrawProposal"> {
  return { request: {
    command_schema: "storyos.command.withdraw-proposal.request.v1",
    withdraw_proposal_input: { cause: "author", proposal_revision_id: proposal.revision_id, expected_closure: "open",
      expected_target_revisions: [target], withdrawal_reason: { kind: "author_withdrew", note: { kind: "omitted" } },
      editor_session_id: editorSessionId, ...story.binding() },
  }, path: { proposalId: proposal.proposal_id } };
}

/** A Project with an assistance-enabled Chapter and one admitted AgentRun that no Worker settled. */
async function admittedRun(title: string) {
  const story = await createStory(server, { title, assistance: true });
  const run = await story.send("createAgentRun", agentRun(story));
  if (run.response.effect.kind !== "admitted") throw new Error("expected an admitted AgentRun");
  return { story, runId: run.response.effect.run_id, conversationId: run.response.conversation_id };
}

function agentRun(story: TestStory): CommandInput<"createAgentRun"> {
  return { request: {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: { conversation: { kind: "new" }, author_message: { text: "Revise this passage." },
      working_target: { kind: "current_chapter", chapter_id: story.chapterIds[0] ?? "" },
      instruction: { kind: "absent" }, cause: { kind: "author_request" }, ...story.binding() },
  } };
}

async function authorEdit(story: TestStory,
  session?: CreateEditorSessionResponse): Promise<CommandInput<"applyAuthorEdit">> {
  session ??= await story.openEditorSession();
  if (session.writer.kind !== "current_writer") throw new Error("expected the current writer");
  return { request: {
    command_schema: "storyos.command.apply-author-edit.request.v1", ...story.binding(),
    editor_session_id: session.editor_session.editor_session_id, writer_generation: session.writer.writer_generation,
    chapter_id: session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: [], target_refs: session.base_snapshot.target_refs,
    observed_ownership_partition: "authoritative", editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: story.key(), completed_intent_record_id: story.key(), local_intent_sequence: "1",
    author_edit_units: [{ normalized_primitives: [{ kind: "replace_selection", from: 0, to: 0, text: "Smoke" }],
      selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 0 } }],
  } };
}

/** One Project with a record for each query: a Proposal, an AgentRun, an Author Edit, and two ready exports. */
async function buildReadFixture() {
  await drainLeftoverWork();
  const story = await createStory(server, { title: "Smoke Read Novel", assistance: true });
  const session = await story.openEditorSession();
  const edit = await story.challenge("applyAuthorEdit", await authorEdit(story, session));
  assert.equal((await edit.send()).response.effect.kind, "authoritative_applied");
  const proposal = (await openProposal(story)).proposal;
  const archive = await story.send("exportProjectArchive", archiveExport(story));
  const readable = await story.send("exportHumanReadableManuscript", readableExport(story));
  if (archive.response.effect.kind !== "admitted" || readable.response.effect.kind !== "admitted") {
    throw new Error("expected admitted exports");
  }
  await settleOnce();
  const tree = await client.getManuscriptTree({ baseUrl: story.baseUrl, projectId: story.projectId,
    fetchImpl: story.fetchImpl });
  return { story, proposal, edit, session, snapshotId: tree.snapshot.snapshot_id,
    archiveId: archive.response.effect.export_id, readableId: readable.response.effect.export_id };
}

let readFixture: ReturnType<typeof buildReadFixture> | undefined;
const reads = () => (readFixture ??= buildReadFixture());
type ReadFixture = Awaited<ReturnType<typeof buildReadFixture>>;

function archiveExport(story: TestProject): CommandInput<"exportProjectArchive"> {
  return { request: {
    command_schema: "storyos.command.export-project-archive.request.v1",
    export_project_archive_input: { ...story.binding(), archive_profile: "storyos.project-export.v1",
      archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" },
  } };
}

function readableExport(story: TestProject): CommandInput<"exportHumanReadableManuscript"> {
  return { request: {
    command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
    export_human_readable_manuscript_input: story.binding(),
  } };
}

/** The current Proposal Generation. No public response gives its ID, so the row reads it. */
async function generationId(proposalId: string): Promise<string> {
  return queryStoryOSPostgres(`SELECT generation_id::text FROM storyos.proposal_generation_heads
    WHERE proposal_id = '${proposalId}'::uuid;`);
}

// Rows

/** A project command row: the arranged Project, the input, and the expected part of the challenged response. */
interface CommandRow<Op extends ProjectCommandId> {
  arrange(): Promise<{ project: TestProject; input: CommandInput<Op> }>;
  expected: object;
}

type CommandRows = { [Op in ProjectCommandId]: CommandRow<Op> };

const COMMANDS: CommandRows = {
  updateProject: {
    arrange: async () => {
      const project = await createProject(server, { title: "Smoke Rename Novel" });
      return { project, input: { request: {
        command_schema: "storyos.command.update-project.request.v1",
        update_project_input: { title: "Smoke Renamed Novel", expected_project_revision: "1", ...project.binding() },
      } } };
    },
    expected: { effect: { kind: "authoritative_applied", title: "Smoke Renamed Novel", revision: "2" } },  },
  updateProjectAssistance: {
    arrange: async () => {
      const project = await createProject(server, { title: "Smoke Assistance Novel" });
      return { project, input: { request: {
        command_schema: "storyos.command.update-project-assistance.request.v1",
        update_project_assistance_input: { availability: "available", expected_assistance_revision: "0",
          ...project.binding() },
      } } };
    },
    expected: { effect: { kind: "initialized", availability: "available", revision: "1" } },  },
  archiveProject: {
    arrange: async () => {
      const project = await createProject(server, { title: "Smoke Archive Novel" });
      return { project, input: { request: {
        command_schema: "storyos.command.archive-project.request.v1",
        archive_project_input: { expected_project_revision: "1", ...project.binding() },
      } } };
    },
    expected: { effect: { kind: "authoritative_applied", revision: "2" } },  },
  createEditorSession: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Session Novel" });
      return { project, input: { request: {
        command_schema: "storyos.command.create-editor-session.request.v1", ...project.binding(),
      } } };
    },
    expected: { writer: { kind: "current_writer" } },
  },
  takeOverProjectWriter: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Takeover Novel" });
      const writer = await project.openEditorSession();
      const observer = await project.openEditorSession();
      if (writer.writer.kind !== "current_writer") throw new Error("expected the current writer");
      const editorSessionId = observer.editor_session.editor_session_id;
      return { project, input: { request: {
        command_schema: "storyos.command.take-over-project-writer.request.v1", ...project.binding(),
        editor_session_id: editorSessionId, observed_writer_generation: writer.writer.writer_generation,
        editor_contract_revision: "storyos.editor-contract.release-1.v3",
      }, path: { editorSessionId } } };
    },
    expected: { result: { kind: "takeover_applied", prior_writer_generation: "1", resulting_writer_generation: "2" } },  },
  createVolume: {
    arrange: async () => {
      const project = await createProject(server, { title: "Smoke Volume Novel" });
      return { project, input: { request: {
        command_schema: "storyos.command.create-volume.request.v1",
        create_volume_input: { title: "Volume A", expected_tree_revision: "1", ...project.binding() },
      } } };
    },
    expected: { effect: { kind: "authoritative_applied" } },
  },
  updateVolume: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Volume Rename Novel" });
      return { project, input: { request: {
        command_schema: "storyos.command.update-volume.request.v1",
        update_volume_input: { title: "Volume B", order: "1", expected_tree_revision: await project.treeRevision(),
          ...project.binding() },
      }, path: { volumeId: project.volumeId } } };
    },
    expected: { effect: { kind: "authoritative_applied", title: "Volume B", order: "1" } },  },
  deleteVolume: {
    arrange: async () => {
      const project = await addVolume(await createProject(server, { title: "Smoke Volume Delete Novel" }), []);
      return { project, input: { request: {
        command_schema: "storyos.command.delete-volume.request.v1",
        delete_volume_input: { expected_tree_revision: await project.treeRevision(), ...project.binding() },
      }, path: { volumeId: project.volumeId } } };
    },
    expected: { effect: { kind: "authoritative_applied" } },
  },
  createChapter: {
    arrange: async () => {
      const project = await addVolume(await createProject(server, { title: "Smoke Chapter Novel" }), []);
      return { project, input: { request: {
        command_schema: "storyos.command.create-chapter.request.v1",
        create_chapter_input: { title: "Chapter A", expected_tree_revision: await project.treeRevision(),
          ...project.binding() },
      }, path: { volumeId: project.volumeId } } };
    },
    expected: { effect: { kind: "authoritative_applied", title: "Chapter A", order: "1" } },  },
  updateChapter: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Chapter Rename Novel" });
      return { project, input: { request: {
        command_schema: "storyos.command.update-chapter.request.v1",
        update_chapter_input: { title: "Chapter B", order: "1", expected_tree_revision: await project.treeRevision(),
          ...project.binding() },
      }, path: { chapterId: project.chapterIds[0] ?? "" } } };
    },
    expected: { effect: { kind: "authoritative_applied", title: "Chapter B", order: "1" } },  },
  deleteChapter: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Chapter Delete Novel" });
      return { project, input: { request: {
        command_schema: "storyos.command.delete-chapter.request.v1",
        delete_chapter_input: { expected_tree_revision: await project.treeRevision(), ...project.binding() },
      }, path: { chapterId: project.chapterIds[0] ?? "" } } };
    },
    expected: { effect: { kind: "authoritative_applied" } },
  },
  setCurrentChapter: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Current Novel", chapters: ["Chapter A", "Chapter B"] });
      const session = await project.openEditorSession();
      return { project, input: { request: {
        command_schema: "storyos.command.set-current-chapter.request.v1",
        set_current_chapter_input: { chapter_id: project.chapterIds[1] ?? "",
          expected_current_chapter_id: project.chapterIds[0] ?? "",
          expected_target_revision_id: (await chapter(project, 1)).current_revision.revision_id,
          editor_session_id: session.editor_session.editor_session_id, ...project.binding() },
      } } };
    },
    expected: { effect: { kind: "authoritative_applied" } },
  },
  applyAuthorEdit: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Edit Novel" });
      return { project, input: await authorEdit(project) };
    },
    expected: { effect: { kind: "authoritative_applied" } },
  },
  acceptProposal: {
    arrange: async () => {
      const { story, target, proposal, editorSessionId } = await proposed("Smoke Accept Novel");
      if (proposal.validation_receipt.kind !== "present") throw new Error("expected a Validation Receipt");
      return { project: story, input: { request: {
        command_schema: "storyos.command.accept-proposal.request.v1",
        accept_proposal_input: { proposal_revision_id: proposal.revision_id,
          validation_receipt_id: proposal.validation_receipt.validation_receipt_id,
          selected_operation_ids: [proposal.operation_id], expected_authoritative_revision_id: target,
          editor_session_id: editorSessionId, ...story.binding() },
      }, path: { proposalId: proposal.proposal_id } } };
    },
    expected: { effect: { kind: "applied", target_resolution: "exact_base" } },  },
  rejectProposalOperations: {
    arrange: async () => {
      const opened = await proposed("Smoke Reject Novel");
      return { project: opened.story, input: rejection(opened) };
    },
    expected: { effect: { kind: "resolved", undo_disposition: "forward", prior_resolution: "pending",
      resulting_resolution: "rejected", preserved_generation: "ready", preserved_validation: "valid",
      preserved_closure: "open" } },  },
  withdrawProposal: {
    arrange: async () => {
      const opened = await proposed("Smoke Withdraw Novel");
      return { project: opened.story, input: withdrawal(opened) };
    },
    expected: { effect: { kind: "resolved", undo_disposition: "forward", prior_closure: "open",
      resulting_closure: "withdrawn", preserved_generation: "ready", preserved_validation: "valid" } },  },
  replanProposal: {
    arrange: async () => {
      const { story, target, proposal, editorSessionId } = await proposed("Smoke Replan Novel");
      return { project: story, input: { request: {
        command_schema: "storyos.command.replan-proposal.request.v1",
        replan_proposal_input: { conflicted_proposal_revision_id: proposal.revision_id,
          expected_current_proposal_head: proposal.revision_id, expected_current_target_revisions: [target],
          replacement_operations: [proposal.operation_id],
          source_condition: { kind: "proposal_conflict", proposal_conflict_ref: story.key() },
          editor_session_id: editorSessionId, ...story.binding() },
      }, path: { proposalId: proposal.proposal_id } } };
    },
    expected: { effect: { kind: "refused", reason: "unavailable_proof" } },  },
  reopenWithdrawnProposal: {
    arrange: async () => {
      const opened = await proposed("Smoke Reopen Novel");
      const { story, target, proposal, editorSessionId } = opened;
      const withdrawn = await story.send("withdrawProposal", withdrawal(opened));
      if (withdrawn.response.effect.kind !== "resolved") throw new Error("expected a withdrawal");
      return { project: story, input: { request: {
        command_schema: "storyos.command.reopen-withdrawn-proposal.request.v1",
        reopen_withdrawn_proposal_input: { proposal_revision_id: proposal.revision_id,
          withdrawal_event_ref: withdrawn.response.effect.closure_event_refs[0] ?? "", expected_closure: "withdrawn",
          expected_target_revisions: [target], editor_session_id: editorSessionId, ...story.binding() },
      }, path: { proposalId: proposal.proposal_id } } };
    },
    expected: { effect: { kind: "resolved", undo_disposition: "forward", prior_closure: "withdrawn",
      resulting_closure: "open", resulting_validation: "pending", preserved_generation: "ready",
      preserved_operation_resolution: "pending" } },  },
  reopenRejectedOperations: {
    arrange: async () => {
      const opened = await proposed("Smoke Reopen Rejected Novel");
      const { story, target, proposal, editorSessionId } = opened;
      const rejected = await story.send("rejectProposalOperations", rejection(opened));
      if (rejected.response.effect.kind !== "resolved") throw new Error("expected a rejection");
      return { project: story, input: { request: {
        command_schema: "storyos.command.reopen-rejected-operations.request.v1",
        reopen_rejected_operations_input: { proposal_revision_id: proposal.revision_id,
          selected_rejected_operation_ids: [proposal.operation_id],
          rejection_event_refs: rejected.response.effect.resolution_event_refs, expected_target_revisions: [target],
          editor_session_id: editorSessionId, ...story.binding() },
      }, path: { proposalId: proposal.proposal_id } } };
    },
    expected: { effect: { kind: "resolved", undo_disposition: "forward", prior_resolution: "rejected",
      resulting_resolution: "pending", resulting_validation: "pending", preserved_generation: "ready",
      preserved_closure: "open" } },  },
  completeReadyPartialProposal: {
    arrange: async () => {
      const { story, target, proposal, editorSessionId } = await proposed("Smoke Complete Novel", { streamed: true });
      return { project: story, input: { request: {
        command_schema: "storyos.command.complete-ready-partial-proposal.request.v1",
        complete_ready_partial_proposal_input: { proposal_revision_id: proposal.revision_id,
          generation_id: await generationId(proposal.proposal_id),
          expected_candidate_digest: digest(proposal.candidate_text), last_applied_stream_seq: "1",
          expected_target_revisions: [target], editor_session_id: editorSessionId, ...story.binding() },
      }, path: { proposalId: proposal.proposal_id } } };
    },
    expected: { effect: { kind: "refused", reason: "not_ready_partial" } },
  },
  continueProposalGeneration: {
    arrange: async () => {
      const { story, target, proposal, editorSessionId } = await proposed("Smoke Continue Novel", { streamed: true });
      return { project: story, input: { request: {
        command_schema: "storyos.command.continue-proposal-generation.request.v1",
        continue_proposal_generation_input: { proposal_revision_id: proposal.revision_id,
          prior_generation_id: await generationId(proposal.proposal_id), expected_generation_state: "ready",
          expected_candidate_digest: digest(proposal.candidate_text),
          selected_pending_operation_ids: [proposal.operation_id], expected_target_revisions: [target],
          editor_session_id: editorSessionId, ...story.binding() },
      }, path: { proposalId: proposal.proposal_id } } };
    },
    expected: { effect: { kind: "started", undo_disposition: "forward", prior_generation_state: "ready",
      resulting_generation_state: "generating", preserved_validation: "valid", preserved_closure: "open",
      preserved_operation_resolution: "pending" } },  },
  undoLatestAuthorAction: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Undo Novel" });
      const session = await project.openEditorSession();
      const editorSessionId = session.editor_session.editor_session_id;
      const frontier = (await client.getEditorSession({ baseUrl: project.baseUrl, projectId: project.projectId,
        editorSessionId, fetchImpl: project.fetchImpl })).author_undo_frontier_sequence ?? "";
      return { project, input: { request: {
        command_schema: "storyos.command.undo-latest-author-action.request.v1",
        undo_latest_author_action_input: { expected_author_undo_frontier_sequence: frontier,
          expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
          editor_session_id: editorSessionId, ...project.binding() },
      } } };
    },
    expected: { effect: { kind: "compensated" } },
  },
  createAgentRun: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Run Novel", assistance: true });
      return { project, input: agentRun(project) };
    },
    expected: { effect: { kind: "admitted" } },
  },
  steerAgentRun: {
    arrange: async () => {
      const { story, runId, conversationId } = await admittedRun("Smoke Steer Novel");
      return { project: story, input: { request: {
        command_schema: "storyos.command.steer-agent-run.request.v1",
        steer_agent_run_input: { conversation_id: conversationId, author_message: { text: "Keep it short." },
          ...story.binding() },
      }, path: { runId } } };
    },
    expected: { effect: { kind: "retained" } },
  },
  pauseAgentRun: {
    arrange: async () => {
      const { story, runId } = await admittedRun("Smoke Pause Novel");
      return { project: story, input: { request: {
        command_schema: "storyos.command.pause-agent-run.request.v1", pause_agent_run_input: story.binding(),
      }, path: { runId } } };
    },
    expected: { effect: { kind: "applied", status: "paused" } },  },
  cancelAgentRun: {
    arrange: async () => {
      const { story, runId } = await admittedRun("Smoke Cancel Novel");
      return { project: story, input: { request: {
        command_schema: "storyos.command.cancel-agent-run.request.v1", cancel_agent_run_input: story.binding(),
      }, path: { runId } } };
    },
    expected: { effect: { kind: "applied", status: "cancelled" } },  },
  exportProjectArchive: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Archive Export Novel" });
      return { project, input: archiveExport(project) };
    },
    expected: { effect: { kind: "admitted", archive_profile: "storyos.project-export.v1" } },  },
  exportHumanReadableManuscript: {
    arrange: async () => {
      const project = await createStory(server, { title: "Smoke Readable Export Novel" });
      return { project, input: readableExport(project) };
    },
    expected: { effect: { kind: "admitted", export_profile: "storyos.readable-export.utf8-lf.v1" } },  },
};

async function checkCommand<Op extends ProjectCommandId>(operation: Op, row: CommandRow<Op>): Promise<void> {
  const { project, input } = await row.arrange();
  const anonymous = await problemOf(project.sendUnchallenged(operation, input, ""));
  const unproved = await problemOf(project.sendUnchallenged(operation, input));
  const foreign = await problemOf(project.as("session-b").send(operation, input));
  assert.deepEqual([anonymous.status, anonymous.code, unproved.status, unproved.code, foreign.status],
    [401, "authentication_required", 422, "challenge_invalid", 404]);
  assert.ok(!foreign.body.includes(USER_A));
  const { response } = await project.send(operation, input);
  assert.partialDeepStrictEqual(response, { schema_id: RESPONSE_SCHEMA.get(operation), ...row.expected });
}

/** A row of a query, a stream, a Challenge, or Create Project. `call` sends the request with this fetch. */
interface ReadRow {
  call(fixture: ReadFixture, fetchImpl: typeof fetch): Promise<unknown>;
  /** The shape check of a response that has no schema ID. */
  shape?(response: unknown): void;
  /** The route needs no session (`public`) or only a session (`user`). Otherwise it is in Project Scope. */
  access?: "public" | "user";
  /** The route reads the latest Project Snapshot and answers 409 `snapshot_expired` when that Snapshot expires. */
  snapshotBound?: true;
}

const READS: Record<string, ReadRow> = {
  getProtocolProfile: {
    call: (_, fetchImpl) => bootProtectedWebClient({ baseUrl: server.baseUrl, fetchImpl }),
    shape: (boot) => assert.equal((boot as { kind: string }).kind, "protected-ready"),
    access: "public",
  },
  createProjectChallenge: {
    call: (_, fetchImpl) => client.createProjectChallenge({ baseUrl: server.baseUrl, fetchImpl,
      request: createProjectRequest(id(freshNamespace())) }),
    shape: (challenge) => assert.match((challenge as { nonce: string }).nonce, NONCE),
    access: "user",
  },
  createProject: {
    call: async (_, fetchImpl) => {
      const key = id(freshNamespace());
      const request = createProjectRequest(key);
      const issued = await client.createProjectChallenge({ baseUrl: server.baseUrl, fetchImpl: server.fetch(), request });
      const send = (antiForgery: string, sender = fetchImpl) => client.createProject({ baseUrl: server.baseUrl,
        fetchImpl: sender,
        idempotencyKey: key, antiForgery, request: { command_schema: request.command_schema,
          prospective_project_id: issued.prospective_project_id, create_project_input: request.create_project_input } });
      const unproved = await problemOf(send("0".repeat(64), server.fetch()));
      const foreign = await problemOf(send(issued.nonce, server.fetch("session-b")));
      assert.deepEqual([unproved.status, unproved.code, foreign.status], [422, "challenge_invalid", 422]);
      assert.ok(!foreign.body.includes(USER_A));
      return send(issued.nonce);
    },
    access: "user",
  },
  createProjectCommandChallenge: {
    call: async ({ story }, fetchImpl) => client.createProjectCommandChallenge({ baseUrl: server.baseUrl,
      projectId: story.projectId, fetchImpl, request: {
        method: "PATCH", route_template: "/api/v1/projects/{project_id}",
        command_schema: "storyos.command.update-project.request.v1",
        canonical_command_digest: await client.digestUpdateProject({
          command_schema: "storyos.command.update-project.request.v1",
          update_project_input: { title: "Smoke Read Novel", expected_project_revision: "1", ...story.binding() } }),
        idempotency_key: story.key(),
      } }),
    shape: (challenge) => assert.match((challenge as { nonce: string }).nonce, NONCE),
  },
  listProjects: {
    call: async (_, fetchImpl) => {
      const listed = await client.listProjects({ baseUrl: server.baseUrl, fetchImpl });
      assert.equal(listed.owner_user_id, USER_A);
      return listed;
    },
    access: "user",
  },
  getProject: { call: ({ story }, fetchImpl) => client.getProject({ ...scope(story), fetchImpl }) },
  getProjectAssistance: {
    call: ({ story }, fetchImpl) => client.getProjectAssistance({ ...scope(story), fetchImpl }),
  },
  getEditorSession: {
    call: ({ story, session }, fetchImpl) => client.getEditorSession({ ...scope(story), fetchImpl,
      editorSessionId: session.editor_session.editor_session_id }),
  },
  getAgentRun: {
    call: ({ story, proposal }, fetchImpl) => client.getAgentRun({ ...scope(story), fetchImpl,
      runId: proposal.source.run_id }),
  },
  getManuscriptTree: {
    call: ({ story }, fetchImpl) => client.getManuscriptTree({ ...scope(story), fetchImpl }),
    snapshotBound: true,
  },
  getChapter: {
    call: ({ story }, fetchImpl) => client.getChapter({ ...scope(story), fetchImpl,
      chapterId: story.chapterIds[0] ?? "" }),
    snapshotBound: true,
  },
  getStatistics: { call: ({ story }, fetchImpl) => client.getStatistics({ ...scope(story), fetchImpl }) },
  getProposal: {
    call: ({ story, proposal }, fetchImpl) => client.getProposal({ ...scope(story), fetchImpl,
      proposalId: proposal.proposal_id }),
  },
  listChapterOpenProposals: {
    call: ({ story }, fetchImpl) => client.listChapterOpenProposals({ ...scope(story), fetchImpl,
      chapterId: story.chapterIds[0] ?? "" }),
  },
  getApplyAuthorEditOutcome: {
    call: ({ story, edit }, fetchImpl) => client.getApplyAuthorEditOutcome({ ...scope(story), fetchImpl,
      idempotencyKey: edit.key, antiForgery: edit.nonce }),
  },
  getSnapshot: {
    call: ({ story, snapshotId }, fetchImpl) => client.getSnapshot({ ...scope(story), fetchImpl, snapshotId }),
  },
  activityStream: {
    call: ({ story, snapshotId }, fetchImpl) => client.activityStream({ ...scope(story), fetchImpl, snapshotId,
      protocolRelease: "storyos.public.release.1" }),
    shape: (body) => assert.equal(typeof body, "string"),
  },
  searchManuscript: {
    call: ({ story }, fetchImpl) => client.searchManuscript({ ...scope(story), fetchImpl, request: {
      schema_id: "storyos.query.manuscript-search.request.v1", selection: "manuscript", query_text: "Smoke",
      required_watermark: null } }),
  },
  getExportOperation: {
    call: ({ story, archiveId }, fetchImpl) => client.getExportOperation({ ...scope(story), fetchImpl,
      exportId: archiveId }),
  },
  getHumanReadableManuscriptExport: {
    call: ({ story, readableId }, fetchImpl) => client.getHumanReadableManuscriptExport({ ...scope(story), fetchImpl,
      exportId: readableId }),
  },
};

function scope(story: TestProject) {
  return { baseUrl: story.baseUrl, projectId: story.projectId };
}

function createProjectRequest(key: string) {
  return { command_schema: "storyos.command.create-project.request.v1",
    create_project_input: { title: "Smoke Created Novel", ...BINDING, correlation_id: id(freshNamespace()) },
    idempotency_key: key };
}

async function checkRead(operation: string, row: ReadRow): Promise<void> {
  const fixture = await reads();
  if (row.access !== "public") {
    const anonymous = await problemOf(row.call(fixture, server.fetch("")));
    assert.deepEqual([anonymous.status, anonymous.code], [401, "authentication_required"]);
  }
  if (row.access === undefined) {
    const foreign = await problemOf(row.call(fixture, server.fetch("session-b")));
    assert.equal(foreign.status, 404);
    assert.ok(!foreign.body.includes(USER_A));
  }
  const response = await row.call(fixture, server.fetch());
  if (row.shape === undefined) {
    assert.equal((response as { schema_id?: string }).schema_id, RESPONSE_SCHEMA.get(operation));
  } else {
    row.shape(response);
  }
  if (row.snapshotBound === true) {
    const story = await createStory(server, { title: "Smoke Expired Novel" });
    await row.call({ ...fixture, story }, server.fetch());
    await queryStoryOSPostgres(`
      UPDATE storyos.project_snapshots SET expires_at = clock_timestamp() - interval '1 second'
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${story.projectId}'::uuid`);
    const expired = await problemOf(row.call({ ...fixture, story }, server.fetch()));
    assert.deepEqual([expired.status, expired.code], [409, "snapshot_expired"]);
  }
}

test("the Smoke Check table has one row for each operation of the Route Catalog", () => {
  assert.deepEqual([...Object.keys(COMMANDS), ...Object.keys(READS)].sort(),
    CATALOG.map((operation) => operation.operation_id).sort());
});

for (const [operation, row] of Object.entries(COMMANDS)) {
  test(`${operation} answers`, () => checkCommand(operation as ProjectCommandId, row as CommandRow<ProjectCommandId>));
}
for (const [operation, row] of Object.entries(READS)) {
  test(`${operation} answers`, () => checkRead(operation, row));
}
