// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/continue-conversation-input-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createAgentRun,
  createChapter,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  digestCreateAgentRun,
  digestCreateChapter,
  digestCreateVolume,
  digestUpdateProjectAssistance,
  getAgentRun,
  getChapter,
  getProposal,
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateAgentRunRequest,
  GetProposalResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  runStoryOSWorker,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const bin = (name: string) => join(repositoryRoot, "target", "release-package", process.platform === "win32" ? `${name}.exe` : name);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};
const PROSE = "Guard the narrator voice in this passage.";

function id(suffix: string): string {
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

async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary: bin("storyos-server"),
    sessions: { "session-a": USER_A, "session-b": "018f0000-0000-7001-8000-000000000101" },
  });
}

async function settleOnce() {
  await runStoryOSWorker({
    repositoryRoot,
    workerBinary: bin("storyos-worker"),
    args: ["--once"],
  });
}

async function drainLeftoverWork() {
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

async function challenged<T>(
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

async function prepare(baseUrl: string, createKey: string, title: string, ns: string) {
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

async function admitProse(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  chapterId: string,
  key: string,
) {
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

function assertOpenedProposal(queried: Awaited<ReturnType<typeof getAgentRun>>): string {
  assert.equal(queried.status, "completed");
  assert.equal(queried.decision.kind, "prose_change");
  if (queried.decision.kind !== "prose_change") throw new Error("expected prose");
  assert.equal(queried.decision.opened_proposal.kind, "present");
  if (queried.decision.opened_proposal.kind !== "present") throw new Error("expected opened");
  assert.match(queried.decision.opened_proposal.proposal_id, UUID_V7);
  return queried.decision.opened_proposal.proposal_id;
}

function assertSameProposal(first: GetProposalResponse, second: GetProposalResponse) {
  assert.deepEqual(
    { ...first.proposal, source: first.proposal.source, validation_receipt: first.proposal.validation_receipt },
    { ...second.proposal, source: second.proposal.source, validation_receipt: second.proposal.validation_receipt },
  );
}

test("Worker opens one Block Proposal in place without changing Authoritative prose", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f111"), "Open Proposal Novel", "f2");
    const before = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    const queried = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f131"));
    const proposalId = assertOpenedProposal(queried);
    if (queried.decision.kind !== "prose_change") throw new Error("expected prose");
    const inspected = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(inspected.proposal.kind, "block_edit");
    assert.equal(inspected.proposal.generation, "ready");
    assert.equal(inspected.proposal.validation, "valid");
    assert.equal(inspected.proposal.closure, "open");
    assert.equal(inspected.proposal.operation_resolution, "pending");
    assert.equal(inspected.proposal.reservation_state, "unresolved");
    assert.equal(inspected.proposal.candidate_text, PROSE);
    assert.equal(inspected.proposal.chapter_id, prepared.chapterId);
    assert.equal(inspected.proposal.base_authoritative_revision_id, before.chapter.current_revision.revision_id);
    assert.match(inspected.proposal.proposal_id, UUID_V7);
    assert.match(inspected.proposal.revision_id, UUID_V7);
    assert.match(inspected.proposal.operation_id, UUID_V7);
    assert.match(inspected.proposal.manuscript_block_id, UUID_V7);
    assert.deepEqual(inspected.proposal.source, {
      kind: "agent_run_decision",
      run_id: queried.run_id,
      decision_id: queried.decision.decision_id,
    });
    assert.equal(inspected.proposal.validation_receipt.kind, "present");
    if (inspected.proposal.validation_receipt.kind !== "present") throw new Error("expected receipt");
    assert.match(inspected.proposal.validation_receipt.validation_receipt_id, UUID_V7);
    assert.equal(inspected.proposal.validation_receipt.result, "valid");
    const reloaded = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId,
      fetchImpl: prepared.fetchImpl,
    });
    assertSameProposal(inspected, reloaded);
    const after = await getChapter({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      chapterId: prepared.chapterId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.deepEqual(after.chapter, before.chapter);
    await assert.rejects(
      () => getProposal({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        proposalId,
        fetchImpl: browserFetch(started.baseUrl, "session-b"),
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    await assert.rejects(
      () => getProposal({
        baseUrl: started.baseUrl,
        projectId: prepared.projectId,
        proposalId: id("dead"),
        fetchImpl: prepared.fetchImpl,
      }),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test("Worker keeps two non-overlapping Block Proposals and refuses a reserved current Block", async () => {
  const started = await startRealServer();
  try {
    await drainLeftoverWork();
    const prepared = await prepare(started.baseUrl, id("f141"), "Reservation Novel", "f5");
    const first = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f151"));
    const firstId = assertOpenedProposal(first);
    const overlap = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f153"));
    assert.equal(overlap.decision.kind, "prose_change");
    if (overlap.decision.kind !== "prose_change") throw new Error("expected prose");
    assert.deepEqual(overlap.decision.opened_proposal, { kind: "absent" });
    const secondBlockId = id("f155");
    await queryPostgres(`
      WITH first AS (
        SELECT owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id
          FROM storyos.manuscript_revision_members
         WHERE project_id = '${prepared.projectId}'::uuid
           AND manuscript_object_id = '${prepared.chapterId}'::uuid
         ORDER BY block_order
         LIMIT 1
      ),
      ins_block AS (
        INSERT INTO storyos.manuscript_blocks
          (owner_user_id, project_id, manuscript_block_id, manuscript_object_id, block_kind)
        SELECT owner_user_id, project_id, '${secondBlockId}'::uuid, manuscript_object_id, 'paragraph'
          FROM first
      ),
      ins_member AS (
        INSERT INTO storyos.manuscript_revision_members
          (owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id, block_order)
        SELECT owner_user_id, project_id, manuscript_object_id, revision_id, '${secondBlockId}'::uuid, 2
          FROM first
      )
      UPDATE storyos.authoritative_payloads AS payload
         SET canonical_bytes = convert_to(
           json_build_object(
             'format', 'storyos.manuscript-payload.v1',
             'schema_version', 1,
             'coordinate_version', 1,
             'blocks', json_build_array(
               json_build_object(
                 'manuscript_block_id', first.manuscript_block_id,
                 'block_kind', 'paragraph',
                 'text', ''
               ),
               json_build_object(
                 'manuscript_block_id', '${secondBlockId}',
                 'block_kind', 'paragraph',
                 'text', ''
               )
             )
           )::text,
           'UTF8'
         )
        FROM first
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
              revision.revision_id) =
             (first.owner_user_id, first.project_id, first.manuscript_object_id, first.revision_id)
       WHERE (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id);
    `);
    const second = await admitProse(started.baseUrl, prepared.fetchImpl, prepared.projectId, prepared.chapterId, id("f157"));
    const secondId = assertOpenedProposal(second);
    assert.notEqual(secondId, firstId);
    const firstProposal = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: firstId,
      fetchImpl: prepared.fetchImpl,
    });
    const secondProposal = await getProposal({
      baseUrl: started.baseUrl,
      projectId: prepared.projectId,
      proposalId: secondId,
      fetchImpl: prepared.fetchImpl,
    });
    assert.equal(firstProposal.proposal.reservation_state, "unresolved");
    assert.equal(secondProposal.proposal.reservation_state, "unresolved");
    assert.notEqual(firstProposal.proposal.manuscript_block_id, secondProposal.proposal.manuscript_block_id);
    assert.equal(firstProposal.proposal.chapter_id, secondProposal.proposal.chapter_id);
  } finally {
    await stopRealServer(started.server);
  }
});
