import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { acceptProposal, applyAuthorEdit, createAgentRun, createChapter, createEditorSession, createProject,
  createProjectChallenge, createProjectCommandChallenge, createVolume, digestAcceptProposal, digestApplyAuthorEdit,
  digestCreateAgentRun, digestCreateChapter, digestCreateEditorSession, digestCreateVolume,
  digestRejectProposalOperations, digestUpdateProjectAssistance, getAgentRun, getChapter, getProposal,
  rejectProposalOperations, updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, ApplyAuthorEditRequest, CreateAgentRunRequest, CreateEditorSessionRequest,
  RejectProposalOperationsRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { queryStoryOSPostgres as queryPostgres, runStoryOSWorker,
  sessionFetch as browserFetch, startStoryOSServer, withChallengeBudget } from "./node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const bin = (name: string) => join(repositoryRoot, "target", "release-package", process.platform === "win32" ? `${name}.exe` : name);
export const USER_A = "018f0000-0000-7001-8000-000000000001";
export const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
export const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};
export const PROSE = "Guard the narrator voice in this passage.";
export const REVISED = "Keep the narrator voice in this passage.";

export function id(suffix: string): string {
  return `018f0000-0000-7001-8000-${suffix.padStart(12, "0")}`;
}

function runRequest(chapterId: string, correlationId: string): CreateAgentRunRequest {
  return {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation: { kind: "new" },
      author_message: { text: "Revise this passage: keep the voice." },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

export async function startRealServer(bind = "127.0.0.1:0") {
  return startStoryOSServer({
    bind,
    repositoryRoot,
    serverBinary: bin("storyos-server"),
    sessions: { "session-a": USER_A, "session-c": USER_A, "session-b": "018f0000-0000-7001-8000-00000000000b" },
  });
}

export async function settleOnce(extraEnv?: Readonly<Record<string, string>>) {
  await runStoryOSWorker({
    repositoryRoot,
    workerBinary: bin("storyos-worker"),
    args: ["--once"],
    ...(extraEnv === undefined ? {} : { extraEnv }),
  });
}

export async function drainLeftoverWork() {
  for (let attempt = 0; attempt < 16; attempt += 1) {
    const leftover = await queryPostgres(`
      SELECT count(*)::text
        FROM storyos.agent_runs
       WHERE status IN ('queued', 'claimed') OR (status = 'cancelled' AND wakeup_pending);
    `);
    if (leftover === "0") return;
    await settleOnce();
  }
  throw new Error("leftover AgentRun work did not drain");
}

export async function challenged<T>(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  method: string,
  route: string,
  schema: string,
  digest: Awaited<ReturnType<typeof digestCreateAgentRun>>,
  key: string,
  send: (antiForgery: string) => Promise<T>,
): Promise<T> {
  const challenge = await withChallengeBudget(projectId, () => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: { method, route_template: route, command_schema: schema, canonical_command_digest: digest, idempotency_key: key },
  }));
  return send(challenge.nonce);
}

export async function prepare(baseUrl: string, createKey: string, title: string, ns: string) {
  const fetchImpl = browserFetch(baseUrl, "session-a");
  const createRequest = {
    command_schema: "storyos.command.create-project.request.v1" as const,
    create_project_input: { title, ...BINDING, correlation_id: id(`${ns}0`) },
    idempotency_key: createKey,
  };
  const created = await createProjectChallenge({ baseUrl, request: createRequest, fetchImpl });
  await createProject({
    baseUrl,
    fetchImpl,
    idempotencyKey: createKey,
    antiForgery: created.nonce,
    request: {
      command_schema: createRequest.command_schema,
      prospective_project_id: created.prospective_project_id,
      create_project_input: createRequest.create_project_input,
    },
  });
  const projectId = created.prospective_project_id;
  const assistance = {
    command_schema: "storyos.command.update-project-assistance.request.v1" as const,
    update_project_assistance_input: {
      availability: "available" as const,
      expected_assistance_revision: "0",
      ...BINDING,
      correlation_id: id(`${ns}1`),
    },
  };
  await challenged(baseUrl, fetchImpl, projectId, "PUT", "/api/v1/projects/{project_id}/assistance", assistance.command_schema, await digestUpdateProjectAssistance(assistance), id(`${ns}2`), (antiForgery) => updateProjectAssistance({ baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}2`), antiForgery, request: assistance }));
  const volume = {
    command_schema: "storyos.command.create-volume.request.v1" as const,
    create_volume_input: { title: "Volume A", expected_tree_revision: "1", ...BINDING, correlation_id: id(`${ns}3`) },
  };
  const createdVolume = await challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/volumes", volume.command_schema, await digestCreateVolume(volume), id(`${ns}4`), (antiForgery) => createVolume({ baseUrl, projectId, fetchImpl, idempotencyKey: id(`${ns}4`), antiForgery, request: volume }));
  if (createdVolume.effect.kind !== "authoritative_applied") throw new Error("Create Volume must apply");
  const volumeId = createdVolume.effect.volume_id;
  const chapter = {
    command_schema: "storyos.command.create-chapter.request.v1" as const,
    create_chapter_input: { title: "Chapter A", expected_tree_revision: "2", ...BINDING, correlation_id: id(`${ns}5`) },
  };
  const createdChapter = await challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters", chapter.command_schema, await digestCreateChapter(chapter), id(`${ns}6`), (antiForgery) => createChapter({ baseUrl, projectId, volumeId, fetchImpl, idempotencyKey: id(`${ns}6`), antiForgery, request: chapter }));
  if (createdChapter.effect.kind !== "authoritative_applied") throw new Error("Create Chapter must apply");
  return { fetchImpl, projectId, chapterId: createdChapter.effect.chapter_id };
}

export async function admitProse(baseUrl: string, fetchImpl: typeof fetch, projectId: string, chapterId: string, key: string) {
  const request = runRequest(chapterId, id(key.slice(-4)));
  const created = await challenged(
    baseUrl,
    fetchImpl,
    projectId,
    "POST",
    "/api/v1/projects/{project_id}/agent-runs",
    request.command_schema,
    await digestCreateAgentRun(request),
    key,
    (antiForgery) => createAgentRun({ baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request }),
  );
  if (created.effect.kind !== "admitted") throw new Error("expected admitted");
  await settleOnce();
  return getAgentRun({ baseUrl, projectId, runId: created.effect.run_id, fetchImpl });
}

export async function reviseCandidate(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  opened: Awaited<ReturnType<typeof getProposal>>,
  ns: string,
  currentSession?: Awaited<ReturnType<typeof createEditorSession>>,
) {
  const sessionRequest: CreateEditorSessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}1`),
  };
  const session = currentSession ?? await challenged(
    baseUrl,
    fetchImpl,
    projectId,
    "POST",
    "/api/v1/projects/{project_id}/editor-sessions",
    sessionRequest.command_schema,
    await digestCreateEditorSession(sessionRequest),
    id(`${ns}2`),
    (antiForgery) => createEditorSession({
      baseUrl,
      projectId,
      fetchImpl,
      idempotencyKey: id(`${ns}2`),
      antiForgery,
      request: sessionRequest,
    }),
  );
  if (session.writer.kind !== "current_writer") throw new Error("expected current writer");
  const editRequest: ApplyAuthorEditRequest = {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    ...BINDING,
    correlation_id: id(`${ns}3`),
    editor_session_id: session.editor_session.editor_session_id,
    writer_generation: session.writer.writer_generation,
    chapter_id: session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: [opened.proposal.revision_id],
    proposal_target: {
      proposal_id: opened.proposal.proposal_id,
      operation_id: opened.proposal.operation_id,
      revision_id: opened.proposal.revision_id,
      manuscript_block_id: opened.proposal.manuscript_block_id,
    },
    target_refs: session.base_snapshot.target_refs,
    observed_ownership_partition: "mixed",
    editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: id(`${ns}4`),
    completed_intent_record_id: id(`${ns}5`),
    local_intent_sequence: currentSession ? "2" : "1",
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from: 0, to: 5, text: "Keep" }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 5,
      },
    }],
  };
  await challenged(
    baseUrl,
    fetchImpl,
    projectId,
    "POST",
    "/api/v1/projects/{project_id}/manuscript/author-edits",
    editRequest.command_schema,
    await digestApplyAuthorEdit(editRequest),
    id(`${ns}6`),
    (antiForgery) => applyAuthorEdit({
      baseUrl,
      projectId,
      fetchImpl,
      idempotencyKey: id(`${ns}6`),
      antiForgery,
      request: editRequest,
    }),
  );
  const revised = await getProposal({ baseUrl, projectId, proposalId: opened.proposal.proposal_id, fetchImpl });
  return { session, revised };
}

/** Moves the Chapter Head to a new Revision with these Blocks, as an author change outside the editor. */
export async function advanceChapterHead(projectId: string, chapterId: string, fromRevisionId: string,
  revisionId: string, blocks: { manuscript_block_id: string; text: string }[]) {
  const payload = blocks.length === 1 ? blocks[0]!.text : JSON.stringify({
    format: "storyos.manuscript-payload.v1", schema_version: 1, coordinate_version: 1,
    blocks: blocks.map((block) => ({ ...block, block_kind: "paragraph" })) });
  const from = `FROM storyos.authoritative_revisions WHERE project_id = '${projectId}'::uuid
    AND revision_id = '${fromRevisionId}'::uuid`;
  await queryPostgres([
    ...blocks.map((block) => `INSERT INTO storyos.manuscript_blocks SELECT owner_user_id, project_id,
      '${block.manuscript_block_id}'::uuid, manuscript_object_id, 'paragraph' ${from} ON CONFLICT DO NOTHING;`),
    `INSERT INTO storyos.authoritative_payloads SELECT owner_user_id, project_id, '${revisionId}'::uuid,
      convert_to('${payload.replaceAll("'", "''")}', 'UTF8') ${from};`,
    `INSERT INTO storyos.authoritative_revisions SELECT owner_user_id, project_id, manuscript_object_id,
      '${revisionId}'::uuid, '${revisionId}'::uuid ${from};`,
    ...blocks.map((block, index) => `INSERT INTO storyos.manuscript_revision_members SELECT owner_user_id,
      project_id, manuscript_object_id, '${revisionId}'::uuid, '${block.manuscript_block_id}'::uuid, ${index + 1} ${from};`),
    `UPDATE storyos.authoritative_heads SET current_revision_id = '${revisionId}'::uuid
      WHERE project_id = '${projectId}'::uuid AND manuscript_object_id = '${chapterId}'::uuid;`,
  ].join("\n"));
  await alignWriterBase(projectId, chapterId, revisionId, revisionId);
}

/** Moves the editor session base of the current writer to the Chapter Head, as an Author Edit does. */
export async function alignWriterBase(projectId: string, chapterId: string,
  nextRevision: string, snapshotId: string): Promise<void> {
  const updated = await queryPostgres(`WITH advanced AS (
      UPDATE storyos.scope_counters
      SET project_activity_position = project_activity_position + 1
      WHERE project_id = '${projectId}'::uuid
      RETURNING project_activity_position
    ), moved AS (
      UPDATE storyos.editor_session_base_snapshots AS snapshot
      SET snapshot_id = '${snapshotId}'::uuid,
          authoritative_revision_id = '${nextRevision}'::uuid,
          project_activity_position = (SELECT project_activity_position FROM advanced),
          created_at = clock_timestamp()
      FROM storyos.project_writer_generations AS writer
      WHERE snapshot.owner_user_id = writer.owner_user_id
        AND snapshot.project_id = writer.project_id
        AND snapshot.editor_session_id = writer.current_editor_session_id
        AND snapshot.project_id = '${projectId}'::uuid
        AND snapshot.chapter_object_id = '${chapterId}'::uuid
        AND writer.writer_generation = (
          SELECT max(latest.writer_generation) FROM storyos.project_writer_generations AS latest
          WHERE latest.project_id = '${projectId}'::uuid
        )
      RETURNING snapshot.snapshot_id
    )
    UPDATE storyos.project_snapshots AS canonical
    SET project_activity_position = (SELECT project_activity_position FROM advanced)
    WHERE canonical.project_id = '${projectId}'::uuid
      AND canonical.snapshot_kind = 'canonical'
      AND canonical.project_activity_position = (
        SELECT max(current.project_activity_position) FROM storyos.project_snapshots AS current
        WHERE current.project_id = '${projectId}'::uuid AND current.snapshot_kind = 'canonical'
      )
      AND EXISTS (SELECT 1 FROM moved)
    RETURNING canonical.project_activity_position::text`);
  assert.match(updated.split("\n")[0] ?? "", /^[0-9]+$/);
}

/**
 * Gives the current Proposal Revision an Acceptance condition of this kind. A real Acceptance
 * with an unknown Validation Receipt writes the condition row and its Receipts. Then the row and
 * its Receipts become the records that a conflicted Acceptance on a changed Chapter Head wrote
 * before ADR 0064.
 */
export async function seedAcceptanceCondition(baseUrl: string, fetchImpl: typeof fetch, projectId: string,
  proposal: Awaited<ReturnType<typeof getProposal>>["proposal"], editorSessionId: string,
  expectedRevisionId: string, kind: "proposal_conflict" | "proposal_recovery_conflict", ns: string) {
  const request: AcceptProposalRequest = {
    command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: {
      proposal_revision_id: proposal.revision_id, validation_receipt_id: id(`${ns}01`),
      selected_operation_ids: [proposal.operation_id], expected_authoritative_revision_id: expectedRevisionId,
      editor_session_id: editorSessionId, ...BINDING, correlation_id: id(`${ns}02`),
    },
  };
  const invalid = await challenged(baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", request.command_schema,
    await digestAcceptProposal(request), id(`${ns}03`), (antiForgery) => acceptProposal({
      baseUrl, projectId, proposalId: proposal.proposal_id, fetchImpl, idempotencyKey: id(`${ns}03`),
      antiForgery, request }));
  if (invalid.effect.kind !== "invalid") throw new Error("expected the condition Acceptance to be invalid");
  const conflictRef = id(`${ns}04`);
  const receipt = `project_id = '${projectId}'::uuid AND acceptance_receipt_id = '${invalid.receipt.receipt_id}'::uuid`;
  // The condition row and the Acceptance Receipt change together, so the foreign key check waits.
  await queryPostgres(`SET session_replication_role = replica;
    UPDATE storyos.proposal_validation_conditions
    SET validation = 'conflicted', conflict_id = '${conflictRef}'::uuid, condition_kind = '${kind}' WHERE ${receipt};
    UPDATE storyos.acceptance_receipts SET result = 'conflicted' WHERE ${receipt};
    UPDATE storyos.domain_receipts SET result_kind = 'conflicted', condition_refs = ARRAY['${conflictRef}'],
      result_payload = '{"reason":"changed_head"}'::jsonb
    WHERE project_id = '${projectId}'::uuid AND receipt_id = '${invalid.receipt.receipt_id}'::uuid;`);
  return { conflictRef, receiptId: invalid.receipt.receipt_id };
}

export type Started = Awaited<ReturnType<typeof startRealServer>>;

/** A valid Block Proposal Revision on the Chapter Head H0. */
export async function openedBlockProposal(started: Started, ns: string) {
  const prepared = await prepare(started.baseUrl, id(`${ns}0011`), "Earlier Base Novel", `${ns}02`);
  const queried = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId,
    prepared.chapterId, id(`${ns}0031`));
  if (queried.decision.kind !== "prose_change" || queried.decision.opened_proposal.kind !== "present") {
    throw new Error("expected opened prose");
  }
  const opened = await getProposal({ baseUrl: started.baseUrl, projectId: prepared.projectId,
    proposalId: queried.decision.opened_proposal.proposal_id, fetchImpl: prepared.fetchImpl });
  const { session, revised } = await reviseCandidate(started.baseUrl, prepared.fetchImpl,
    prepared.projectId, opened, `${ns}04`);
  if (revised.proposal.validation_receipt.kind !== "present") throw new Error("expected validation");
  const base = await getChapter({ baseUrl: started.baseUrl, projectId: prepared.projectId,
    chapterId: prepared.chapterId, fetchImpl: prepared.fetchImpl });
  assert.equal(revised.proposal.base_authoritative_revision_id, base.chapter.current_revision.revision_id);
  const history = async () => queryPostgres(`SELECT jsonb_build_object(
    'revisions', (SELECT jsonb_agg(to_jsonb(record) ORDER BY record.revision_id) FROM storyos.proposal_revisions AS record
      WHERE project_id = '${prepared.projectId}'::uuid),
    'validations', (SELECT jsonb_agg(to_jsonb(record) ORDER BY record.validation_receipt_id)
      FROM storyos.validation_receipts AS record WHERE project_id = '${prepared.projectId}'::uuid))::text`);
  return { prepared, session, revised: revised.proposal, base: base.chapter, history,
    validationReceiptId: revised.proposal.validation_receipt.validation_receipt_id };
}

export function acceptRequest(opened: Awaited<ReturnType<typeof openedBlockProposal>>, expectedRevisionId: string,
  correlation: string): AcceptProposalRequest {
  return {
    command_schema: "storyos.command.accept-proposal.request.v1",
    accept_proposal_input: {
      proposal_revision_id: opened.revised.revision_id, validation_receipt_id: opened.validationReceiptId,
      selected_operation_ids: [opened.revised.operation_id], expected_authoritative_revision_id: expectedRevisionId,
      editor_session_id: opened.session.editor_session.editor_session_id, ...BINDING, correlation_id: id(correlation),
    },
  };
}

export async function accept(started: Started, opened: Awaited<ReturnType<typeof openedBlockProposal>>,
  request: AcceptProposalRequest, key: string) {
  const options = { baseUrl: started.baseUrl, projectId: opened.prepared.projectId,
    proposalId: opened.revised.proposal_id, fetchImpl: opened.prepared.fetchImpl, idempotencyKey: id(key), request };
  let nonce = "";
  const accepted = await challenged(started.baseUrl, opened.prepared.fetchImpl, opened.prepared.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances", request.command_schema,
    await digestAcceptProposal(request), id(key), (antiForgery) => acceptProposal({ ...options, antiForgery: (nonce = antiForgery) }));
  return { accepted, replay: () => acceptProposal({ ...options, antiForgery: nonce }) };
}

export async function reject(started: Started, opened: Awaited<ReturnType<typeof openedBlockProposal>>,
  headRevisionId: string, key: string) {
  const request: RejectProposalOperationsRequest = {
    command_schema: "storyos.command.reject-proposal-operations.request.v1",
    reject_proposal_operations_input: { ...BINDING, correlation_id: id(`${key}1`),
      editor_session_id: opened.session.editor_session.editor_session_id,
      proposal_revision_id: opened.revised.revision_id,
      selected_pending_operation_ids: [opened.revised.operation_id],
      expected_target_revisions: [headRevisionId],
      rejection_reason: { kind: "author_declined", note: { kind: "omitted" } } },
  };
  return challenged(started.baseUrl, opened.prepared.fetchImpl, opened.prepared.projectId, "POST",
    "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections", request.command_schema,
    await digestRejectProposalOperations(request), id(`${key}2`), (antiForgery) => rejectProposalOperations({
      baseUrl: started.baseUrl, projectId: opened.prepared.projectId, proposalId: opened.revised.proposal_id,
      request, fetchImpl: opened.prepared.fetchImpl, antiForgery, idempotencyKey: id(`${key}2`) }));
}
