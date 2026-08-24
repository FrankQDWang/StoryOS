import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createProjectChallenge,
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

function challengeRequest(idempotencyKey: string, title = "Prospective Novel"): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000310",
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

async function projectEffectSnapshot() {
  return JSON.parse(await queryPostgres(`
    SELECT json_build_object(
      'projects', (SELECT count(*) FROM storyos.projects),
      'admissions', (SELECT count(*) FROM storyos.author_command_admissions),
      'receipts', (SELECT count(*) FROM storyos.domain_receipts),
      'activities', (SELECT count(*) FROM storyos.project_activity_events),
      'chapters', (SELECT count(*) FROM storyos.manuscript_objects)
    )::text`));
}

test("createProjectChallenge replays, conflicts, isolates Users, and creates no Project", async () => {
  const { baseUrl, server } = await startRealServer();
  const before = await projectEffectSnapshot();
  const request = challengeRequest("018f0000-0000-7001-8000-000000000301");
  try {
    const options = { baseUrl, request, fetchImpl: browserFetch(baseUrl, "session-a") };
    const first = await createProjectChallenge(options);
    const retry = await createProjectChallenge(options);
    assert.deepEqual(retry, first);
    assert.match(first.prospective_project_id, UUID_V7);
    assert.equal(first.canonical_command_digest.algorithm, "sha256");
    assert.equal(first.canonical_command_digest.profile, "storyos.command.createProject.jcs.v1");
    assert.match(first.canonical_command_digest.value_hex_lowercase, /^[0-9a-f]{64}$/);
    assert.match(first.nonce, /^[0-9a-f]{64}$/);
    assert.equal(first.limit_profile_revision, "storyos.foundation.absolute.v1");

    await assert.rejects(
      createProjectChallenge({
        ...options,
        request: challengeRequest("018f0000-0000-7001-8000-000000000301", "Changed Title"),
      }),
      (error) => protocolFailure(error, 409),
    );

    const foreign = await createProjectChallenge({
      baseUrl,
      request: challengeRequest("018f0000-0000-7001-8000-000000000301"),
      fetchImpl: browserFetch(baseUrl, "session-b"),
    });
    assert.notEqual(foreign.prospective_project_id, first.prospective_project_id);
    assert.notEqual(foreign.nonce, first.nonce);
    assert.doesNotMatch(JSON.stringify(foreign), new RegExp(first.prospective_project_id));
    assert.doesNotMatch(JSON.stringify(foreign), new RegExp(first.nonce));

    assert.deepEqual(await projectEffectSnapshot(), before);
  } finally {
    await stopRealServer(server);
  }
});

test("createProjectChallenge refuses a missing Origin without disclosing User identity", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const response = await fetch(new URL("/api/v1/anti-forgery-challenges", baseUrl), {
      method: "POST",
      headers: {
        "content-type": "application/json",
        cookie: "storyos_session=session-a",
        host: new URL(baseUrl).host,
      },
      body: JSON.stringify(challengeRequest("018f0000-0000-7001-8000-000000000302")),
    });
    const body = await response.text();
    assert.equal(response.status, 403);
    assert.doesNotMatch(body, new RegExp(USER_A));
    assert.doesNotMatch(body, /nonce/);
  } finally {
    await stopRealServer(server);
  }
});
