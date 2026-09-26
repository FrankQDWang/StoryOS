import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { applyAuthorEdit, createAgentRun, createChapter, createEditorSession, createProject,
  createProjectChallenge, createProjectCommandChallenge, createVolume, digestApplyAuthorEdit,
  digestCreateAgentRun, digestCreateChapter, digestCreateEditorSession, digestCreateVolume,
  digestUpdateProjectAssistance, getAgentRun, getProposal, updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ApplyAuthorEditRequest, CreateAgentRunRequest, CreateEditorSessionRequest
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { queryStoryOSPostgres as queryPostgres, runStoryOSWorker,
  sessionFetch as browserFetch, startStoryOSServer, withChallengeRetry } from "./node-integration.ts";

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
       WHERE status IN ('queued', 'claimed');
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
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
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
