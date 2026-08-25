import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createProject,
  createProjectChallenge,
  getProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateProjectChallengeRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function challengeRequest(idempotencyKey: string, title = "Empty Novel"): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000510",
    },
    idempotency_key: idempotencyKey,
  };
}

async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A, "session-b": USER_B },
  });
}

function protocolFailure(error: unknown, status: number): boolean {
  return requireStoryOSProtocolError(error).status === status;
}

async function createEmpty(baseUrl: string, session: string, idempotencyKey: string, title = "Empty Novel") {
  const fetchImpl = browserFetch(baseUrl, session);
  const request = challengeRequest(idempotencyKey, title);
  const challenge = await createProjectChallenge({ baseUrl, request, fetchImpl });
  const created = await createProject({
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
  return { challenge, created, fetchImpl, request };
}

test("createProject creates one empty Project, replays, and fails closed", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000501");
    const replay = await createProject({
      baseUrl,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000501",
      antiForgery: first.challenge.nonce,
      request: {
        command_schema: first.request.command_schema,
        prospective_project_id: first.challenge.prospective_project_id,
        create_project_input: first.request.create_project_input,
      },
    });
    assert.equal(replay.command_id, first.created.command_id);
    assert.equal(replay.author_command_admission_id, first.created.author_command_admission_id);
    assert.equal(replay.receipt.receipt_id, first.created.receipt.receipt_id);
    assert.equal(first.created.project.open.kind, "empty");
    assert.equal(first.created.receipt.command_kind, "createProject");
    assert.deepEqual(first.created.receipt.expected_heads, []);
    assert.match(first.created.command_id, UUID_V7);

    const opened = await getProject({
      baseUrl,
      projectId: first.challenge.prospective_project_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.project.open.kind, "empty");
    assert.equal(opened.project.title, "Empty Novel");

    const counts = JSON.parse(await queryPostgres(`
      SELECT json_build_object(
        'chapters', (SELECT count(*) FROM storyos.manuscript_objects
          WHERE project_id = '${first.challenge.prospective_project_id}'::uuid),
        'projects', (SELECT count(*) FROM storyos.projects
          WHERE project_id = '${first.challenge.prospective_project_id}'::uuid),
        'current_chapter', (SELECT current_chapter_id::text FROM storyos.projects
          WHERE project_id = '${first.challenge.prospective_project_id}'::uuid)
      )::text`));
    assert.equal(counts.chapters, 0);
    assert.equal(counts.projects, 1);
    assert.equal(counts.current_chapter, null);

    await assert.rejects(
      createProject({
        baseUrl,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000501",
        antiForgery: first.challenge.nonce,
        request: {
          command_schema: first.request.command_schema,
          prospective_project_id: first.challenge.prospective_project_id,
          create_project_input: { ...first.request.create_project_input, title: "Changed Title" },
        },
      }),
      (error) => protocolFailure(error, 422),
    );
    await assert.rejects(
      createProject({
        baseUrl,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000501",
        antiForgery: "f".repeat(64),
        request: {
          command_schema: first.request.command_schema,
          prospective_project_id: first.challenge.prospective_project_id,
          create_project_input: first.request.create_project_input,
        },
      }),
      (error) => protocolFailure(error, 422),
    );

    await assert.rejects(
      createProject({
        baseUrl,
        fetchImpl: browserFetch(baseUrl, "session-b"),
        idempotencyKey: "018f0000-0000-7001-8000-000000000501",
        antiForgery: first.challenge.nonce,
        request: {
          command_schema: first.request.command_schema,
          prospective_project_id: first.challenge.prospective_project_id,
          create_project_input: first.request.create_project_input,
        },
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 422 && !String(protocol.responseBody).includes(USER_A);
      },
    );

    const existingKey = "018f0000-0000-7001-8000-000000000502";
    const existingChallenge = await createProjectChallenge({
      baseUrl,
      request: challengeRequest(existingKey, "Existing Novel"),
      fetchImpl: first.fetchImpl,
    });
    await queryPostgres(`
      INSERT INTO storyos.projects (owner_user_id, project_id, title, current_chapter_id)
      VALUES ('${USER_A}'::uuid, '${existingChallenge.prospective_project_id}'::uuid, 'Existing', NULL)`);
    await assert.rejects(
      createProject({
        baseUrl,
        fetchImpl: first.fetchImpl,
        idempotencyKey: existingKey,
        antiForgery: existingChallenge.nonce,
        request: {
          command_schema: "storyos.command.create-project.request.v1",
          prospective_project_id: existingChallenge.prospective_project_id,
          create_project_input: challengeRequest(existingKey, "Existing Novel").create_project_input,
        },
      }),
      (error) => protocolFailure(error, 409),
    );
  } finally {
    await stopRealServer(server);
  }
});
