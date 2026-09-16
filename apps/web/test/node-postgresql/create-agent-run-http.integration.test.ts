import assert from "node:assert/strict";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
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
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateAgentRunRequest,
  CreateChapterRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  UpdateProjectAssistanceRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};

function createChallengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      ...BINDING,
      correlation_id: "018f0000-0000-7001-8000-000000000b10",
    },
    idempotency_key: idempotencyKey,
  };
}

function assistanceRequest(correlationId: string): UpdateProjectAssistanceRequest {
  return {
    command_schema: "storyos.command.update-project-assistance.request.v1",
    update_project_assistance_input: {
      availability: "available",
      expected_assistance_revision: "0",
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

function volumeRequest(correlationId: string): CreateVolumeRequest {
  return {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: {
      title: "Volume A",
      expected_tree_revision: "1",
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

function chapterRequest(correlationId: string): CreateChapterRequest {
  return {
    command_schema: "storyos.command.create-chapter.request.v1",
    create_chapter_input: {
      title: "Chapter A",
      expected_tree_revision: "2",
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

function runRequest(
  conversation: CreateAgentRunRequest["create_agent_run_input"]["conversation"],
  chapterId: string,
  correlationId: string,
): CreateAgentRunRequest {
  return {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: {
      conversation,
      author_message: { text: "Help with this passage." },
      working_target: { kind: "current_chapter", chapter_id: chapterId },
      instruction: { kind: "absent" },
      cause: { kind: "author_request" },
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

async function startRealServer(extraEnv?: Readonly<Record<string, string>>) {
  return extraEnv === undefined
    ? startStoryOSServer({
        repositoryRoot,
        serverBinary,
        sessions: { "session-a": USER_A, "session-b": USER_B },
      })
    : startStoryOSServer({
        repositoryRoot,
        serverBinary,
        sessions: { "session-a": USER_A, "session-b": USER_B },
        extraEnv,
      });
}

function problemCode(error: unknown): string | undefined {
  const protocol = requireStoryOSProtocolError(error);
  if (protocol.responseBody === undefined) return undefined;
  try {
    return (JSON.parse(protocol.responseBody) as { code?: string }).code;
  } catch {
    return undefined;
  }
}

async function challenged<T>(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  projectId: string;
  method: string;
  route: string;
  schema: string;
  digest: { algorithm: string; profile: string; value_hex_lowercase: string };
  key: string;
  send: (antiForgery: string) => Promise<T>;
}): Promise<T> {
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    fetchImpl: options.fetchImpl,
    request: {
      method: options.method,
      route_template: options.route,
      command_schema: options.schema,
      canonical_command_digest: options.digest,
      idempotency_key: options.key,
    },
  }));
  return options.send(challenge.nonce);
}

async function createEmpty(baseUrl: string, session: string, idempotencyKey: string, title: string) {
  const fetchImpl = browserFetch(baseUrl, session);
  const request = createChallengeRequest(idempotencyKey, title);
  const challenge = await createProjectChallenge({ baseUrl, request, fetchImpl });
  await createProject({
    baseUrl,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request: {
      command_schema: request.command_schema,
      prospective_project_id: challenge.prospective_project_id,
      create_project_input: request.create_project_input,
    },
  });
  return { fetchImpl, projectId: challenge.prospective_project_id };
}

async function prepareProject(baseUrl: string, fetchImpl: typeof fetch, projectId: string) {
  const assistance = assistanceRequest("018f0000-0000-7001-8000-000000000b20");
  await challenged({
    baseUrl,
    fetchImpl,
    projectId,
    method: "PUT",
    route: "/api/v1/projects/{project_id}/assistance",
    schema: assistance.command_schema,
    digest: await digestUpdateProjectAssistance(assistance),
    key: "018f0000-0000-7001-8000-000000000b21",
    send: (antiForgery) => updateProjectAssistance({
      baseUrl,
      projectId,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b21",
      antiForgery,
      request: assistance,
    }),
  });
  const volume = volumeRequest("018f0000-0000-7001-8000-000000000b22");
  const createdVolume = await challenged({
    baseUrl,
    fetchImpl,
    projectId,
    method: "POST",
    route: "/api/v1/projects/{project_id}/volumes",
    schema: volume.command_schema,
    digest: await digestCreateVolume(volume),
    key: "018f0000-0000-7001-8000-000000000b23",
    send: (antiForgery) => createVolume({
      baseUrl,
      projectId,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b23",
      antiForgery,
      request: volume,
    }),
  });
  if (createdVolume.effect.kind !== "authoritative_applied") {
    throw new Error("Create Volume must apply");
  }
  const chapter = chapterRequest("018f0000-0000-7001-8000-000000000b24");
  const createdChapter = await challenged({
    baseUrl,
    fetchImpl,
    projectId,
    method: "POST",
    route: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
    schema: chapter.command_schema,
    digest: await digestCreateChapter(chapter),
    key: "018f0000-0000-7001-8000-000000000b25",
    send: (antiForgery) => createChapter({
      baseUrl,
      projectId,
      volumeId: createdVolume.effect.volume_id,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b25",
      antiForgery,
      request: chapter,
    }),
  });
  if (createdChapter.effect.kind !== "authoritative_applied") {
    throw new Error("Create Chapter must apply");
  }
  return createdChapter.effect.chapter_id;
}

async function postRun(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  key: string,
  request: CreateAgentRunRequest,
) {
  const digest = await digestCreateAgentRun(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/agent-runs",
      command_schema: "storyos.command.create-agent-run.request.v2",
      canonical_command_digest: digest,
      idempotency_key: key,
    },
  }));
  const admitted = await createAgentRun({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey: key,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, admitted };
}

test("createAgentRun admits one conversation and keeps query scope closed", async () => {
  const started = await startRealServer();
  try {
    const first = await createEmpty(started.baseUrl, "session-a", "018f0000-0000-7001-8000-000000000b30", "Run Novel");
    const chapterId = await prepareProject(started.baseUrl, first.fetchImpl, first.projectId);
    const request = runRequest({ kind: "new" }, chapterId, "018f0000-0000-7001-8000-000000000b31");
    const created = await postRun(
      started.baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000b32",
      request,
    );
    assert.equal(created.admitted.acknowledgement, "accepted");
    assert.equal(created.admitted.effect.kind, "admitted");
    if (created.admitted.effect.kind !== "admitted") throw new Error("expected admitted");
    assert.match(created.admitted.conversation_id, UUID_V7);
    assert.match(created.admitted.memory_settings_revision, UUID_V7);
    assert.equal(created.admitted.effect.conversation_id, created.admitted.conversation_id);
    assert.equal(
      created.admitted.effect.memory_settings_revision,
      created.admitted.memory_settings_revision,
    );
    assert.match(created.admitted.effect.run_id, UUID_V7);
    assert.equal(created.admitted.operation_ref?.kind, "agent_run");

    const replay = await createAgentRun({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b32",
      antiForgery: created.challenge.nonce,
      request,
    });
    assert.deepEqual(replay, created.admitted);

    const queried = await getAgentRun({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      runId: created.admitted.effect.run_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(queried.conversation_id, created.admitted.conversation_id);
    assert.equal(queried.memory_settings_revision, created.admitted.memory_settings_revision);
    assert.equal(queried.run_id, created.admitted.effect.run_id);
    assert.equal(queried.status, "queued");

    await assert.rejects(
      () => postRun(
        started.baseUrl,
        first.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000b33",
        runRequest(
          { kind: "existing", conversation_id: created.admitted.conversation_id },
          chapterId,
          "018f0000-0000-7001-8000-000000000b34",
        ),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 422 && problemCode(error) === "conversation_busy";
      },
    );

    await assert.rejects(
      () => postRun(
        started.baseUrl,
        first.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000b35",
        runRequest({ kind: "new" }, "018f0000-0000-7001-8000-00000000bad1", "018f0000-0000-7001-8000-000000000b36"),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 422 && problemCode(error) === "invalid_chapter_join";
      },
    );

    const secondNew = await postRun(
      started.baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000b37",
      runRequest({ kind: "new" }, chapterId, "018f0000-0000-7001-8000-000000000b38"),
    );
    assert.equal(secondNew.admitted.acknowledgement, "accepted");
    assert.notEqual(secondNew.admitted.conversation_id, created.admitted.conversation_id);

    const settings = await queryPostgres(`
      SELECT bool_and(use_enabled)::text || ' ' || bool_and(contribution_enabled)::text
        FROM storyos.conversation_memory_settings
       WHERE project_id = '${first.projectId}'::uuid;
    `);
    assert.equal(settings, "true true");

    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    await assert.rejects(
      () => getAgentRun({
        baseUrl: started.baseUrl,
        projectId: first.projectId,
        runId: created.admitted.effect.run_id,
        fetchImpl: foreignFetch,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );

    const counts = await queryPostgres(`
      SELECT
        (SELECT count(*) FROM storyos.project_conversations
          WHERE project_id = '${first.projectId}'::uuid)::text
        || ' ' ||
        (SELECT count(*) FROM storyos.agent_runs
          WHERE project_id = '${first.projectId}'::uuid)::text
        || ' ' ||
        (SELECT count(*) FROM storyos.conversation_memory_settings
          WHERE project_id = '${first.projectId}'::uuid)::text;
    `);
    assert.equal(counts, "2 2 2");
  } finally {
    await stopRealServer(started.server);
  }
});

async function waitForSettledKey(projectId: string, idempotencyKey: string) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    const outcome = await queryPostgres(`
      SELECT outcome_kind FROM storyos.command_idempotency
       WHERE project_id = '${projectId}'::uuid
         AND command_kind = 'createAgentRun'
         AND idempotency_key = '${idempotencyKey}'::uuid;
    `);
    if (outcome === "settled") return;
    await new Promise((resolve) => {
      setTimeout(resolve, 10);
    });
  }
  throw new Error(`createAgentRun ${idempotencyKey} did not settle`);
}

test("createAgentRun reopens an idle conversation and refuses digest or scope substitution", async () => {
  const started = await startRealServer();
  try {
    const first = await createEmpty(started.baseUrl, "session-a", "018f0000-0000-7001-8000-000000000b40", "Reopen Novel");
    const chapterId = await prepareProject(started.baseUrl, first.fetchImpl, first.projectId);
    const created = await postRun(
      started.baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000b41",
      runRequest({ kind: "new" }, chapterId, "018f0000-0000-7001-8000-000000000b42"),
    );
    if (created.admitted.effect.kind !== "admitted") throw new Error("expected admitted");
    await queryPostgres(`
      DELETE FROM storyos.agent_runs
       WHERE project_id = '${first.projectId}'::uuid
         AND run_id = '${created.admitted.effect.run_id}'::uuid;
    `);
    const reopenRequest = runRequest(
      { kind: "existing", conversation_id: created.admitted.conversation_id },
      chapterId,
      "018f0000-0000-7001-8000-000000000b44",
    );
    const reopened = await postRun(
      started.baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000b43",
      reopenRequest,
    );
    assert.equal(reopened.admitted.acknowledgement, "accepted");
    assert.equal(reopened.admitted.conversation_id, created.admitted.conversation_id);
    assert.equal(
      reopened.admitted.memory_settings_revision,
      created.admitted.memory_settings_revision,
    );
    assert.notEqual(reopened.admitted.effect.run_id, created.admitted.effect.run_id);

    const mutated = structuredClone(reopenRequest);
    mutated.create_agent_run_input.author_message = { text: "A different assistance request." };
    await assert.rejects(
      () => createAgentRun({
        baseUrl: started.baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000b43",
        antiForgery: reopened.challenge.nonce,
        request: mutated,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 409 && problemCode(error) === "idempotency_binding_conflict";
      },
    );

    await assert.rejects(
      () => postRun(
        started.baseUrl,
        first.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000b45",
        runRequest(
          { kind: "existing", conversation_id: "018f0000-0000-7001-8000-00000000dead" },
          chapterId,
          "018f0000-0000-7001-8000-000000000b46",
        ),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );

    const empty = await createEmpty(started.baseUrl, "session-a", "018f0000-0000-7001-8000-000000000b49", "No Binding Novel");
    await assert.rejects(
      () => postRun(
        started.baseUrl,
        empty.fetchImpl,
        empty.projectId,
        "018f0000-0000-7001-8000-000000000b4a",
        runRequest({ kind: "new" }, chapterId, "018f0000-0000-7001-8000-000000000b4b"),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 422 && problemCode(error) === "assistance_unavailable";
      },
    );
    assert.equal(
      await queryPostgres(`
        SELECT
          (SELECT count(*) FROM storyos.project_conversations
            WHERE project_id = '${empty.projectId}'::uuid)::text
          || ' ' ||
          (SELECT count(*) FROM storyos.conversation_memory_settings
            WHERE project_id = '${empty.projectId}'::uuid)::text;
      `),
      "0 0",
    );

    const foreignFetch = browserFetch(started.baseUrl, "session-b");
    await assert.rejects(
      () => postRun(
        started.baseUrl,
        foreignFetch,
        first.projectId,
        "018f0000-0000-7001-8000-000000000b47",
        runRequest({ kind: "new" }, chapterId, "018f0000-0000-7001-8000-000000000b48"),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
  } finally {
    await stopRealServer(started.server);
  }
});

test("createAgentRun exact retry after commit keeps the first acknowledgement", async () => {
  const heldKey = "018f0000-0000-7001-8000-000000000b50";
  const holdPath = join(tmpdir(), `storyos-ack-hold-${heldKey}.flag`);
  writeFileSync(holdPath, "hold");
  const started = await startRealServer({
    STORYOS_TEST_ACK_HOLD_PATH: holdPath,
    STORYOS_TEST_ACK_HOLD_IDEMPOTENCY_KEY: heldKey,
  });
  try {
    const first = await createEmpty(started.baseUrl, "session-a", "018f0000-0000-7001-8000-000000000b51", "Held Run Novel");
    const chapterId = await prepareProject(started.baseUrl, first.fetchImpl, first.projectId);
    const request = runRequest({ kind: "new" }, chapterId, "018f0000-0000-7001-8000-000000000b52");
    const digest = await digestCreateAgentRun(request);
    const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/agent-runs",
        command_schema: "storyos.command.create-agent-run.request.v2",
        canonical_command_digest: digest,
        idempotency_key: heldKey,
      },
    }));
    const firstAck = createAgentRun({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: heldKey,
      antiForgery: challenge.nonce,
      request,
    });
    await waitForSettledKey(first.projectId, heldKey);
    const heldConversation = await queryPostgres(`
      SELECT conversation_id::text FROM storyos.agent_runs
       WHERE project_id = '${first.projectId}'::uuid;
    `);
    await assert.rejects(
      () => postRun(
        started.baseUrl,
        first.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000b53",
        runRequest(
          { kind: "existing", conversation_id: heldConversation },
          chapterId,
          "018f0000-0000-7001-8000-000000000b54",
        ),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 422 && problemCode(error) === "conversation_busy";
      },
    );
    unlinkSync(holdPath);
    const held = await firstAck;
    assert.equal(held.acknowledgement, "accepted");
    assert.match(held.memory_settings_revision, UUID_V7);
    const replay = await createAgentRun({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: heldKey,
      antiForgery: challenge.nonce,
      request,
    });
    assert.deepEqual(replay, held);
  } finally {
    try {
      unlinkSync(holdPath);
    } catch {
      // The hold file is already removed after the first acknowledgement.
    }
    await stopRealServer(started.server);
  }
});

async function waitForReached(path: string) {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (existsSync(path)) return;
    await new Promise((resolve) => {
      setTimeout(resolve, 10);
    });
  }
  throw new Error(`createAgentRun conversation hold did not reach ${path}`);
}

test("createAgentRun competing existing admission keeps one queued run", async () => {
  const heldKey = "018f0000-0000-7001-8000-000000000b60";
  const holdPath = join(tmpdir(), `storyos-conversation-hold-${heldKey}.flag`);
  const reachedPath = join(tmpdir(), `storyos-conversation-hold-${heldKey}.reached`);
  writeFileSync(holdPath, "hold");
  const started = await startRealServer({
    STORYOS_TEST_CONVERSATION_HOLD_PATH: holdPath,
    STORYOS_TEST_CONVERSATION_HOLD_REACHED_PATH: reachedPath,
    STORYOS_TEST_CONVERSATION_HOLD_IDEMPOTENCY_KEY: heldKey,
  });
  try {
    const first = await createEmpty(started.baseUrl, "session-a", "018f0000-0000-7001-8000-000000000b61", "Compete Novel");
    const chapterId = await prepareProject(started.baseUrl, first.fetchImpl, first.projectId);
    const created = await postRun(
      started.baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000b62",
      runRequest({ kind: "new" }, chapterId, "018f0000-0000-7001-8000-000000000b63"),
    );
    if (created.admitted.effect.kind !== "admitted") throw new Error("expected admitted");
    await queryPostgres(`
      DELETE FROM storyos.agent_runs
       WHERE project_id = '${first.projectId}'::uuid
         AND run_id = '${created.admitted.effect.run_id}'::uuid;
    `);
    const heldRequest = runRequest(
      { kind: "existing", conversation_id: created.admitted.conversation_id },
      chapterId,
      "018f0000-0000-7001-8000-000000000b64",
    );
    const competingRequest = runRequest(
      { kind: "existing", conversation_id: created.admitted.conversation_id },
      chapterId,
      "018f0000-0000-7001-8000-000000000b65",
    );
    const heldChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/agent-runs",
        command_schema: "storyos.command.create-agent-run.request.v2",
        canonical_command_digest: await digestCreateAgentRun(heldRequest),
        idempotency_key: heldKey,
      },
    }));
    const competingChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/agent-runs",
        command_schema: "storyos.command.create-agent-run.request.v2",
        canonical_command_digest: await digestCreateAgentRun(competingRequest),
        idempotency_key: "018f0000-0000-7001-8000-000000000b66",
      },
    }));
    const held = createAgentRun({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: heldKey,
      antiForgery: heldChallenge.nonce,
      request: heldRequest,
    });
    await waitForReached(reachedPath);
    const competing = createAgentRun({
      baseUrl: started.baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000b66",
      antiForgery: competingChallenge.nonce,
      request: competingRequest,
    });
    unlinkSync(holdPath);
    const admitted = await held;
    assert.equal(admitted.acknowledgement, "accepted");
    await assert.rejects(
      () => competing,
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 422 && problemCode(error) === "conversation_busy";
      },
    );
    assert.equal(
      await queryPostgres(`
        SELECT
          (SELECT count(*) FROM storyos.project_conversations
            WHERE project_id = '${first.projectId}'::uuid)::text
          || ' ' ||
          (SELECT count(*) FROM storyos.agent_runs
            WHERE project_id = '${first.projectId}'::uuid)::text;
      `),
      "1 1",
    );
  } finally {
    try {
      unlinkSync(holdPath);
    } catch {
      // The hold file is already removed after the competing admission.
    }
    try {
      unlinkSync(reachedPath);
    } catch {
      // The reached file is created only after the held request starts.
    }
    await stopRealServer(started.server);
  }
});
