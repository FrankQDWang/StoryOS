import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createProject,
  createProjectChallenge,
  getManuscriptTree,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateProjectChallengeRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B = "018f0000-0000-7001-8000-000000000102";

function challengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000710",
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

function sanitizedProtocolFailure(error: unknown, status: number, sensitive: RegExp): boolean {
  const protocolError = requireStoryOSProtocolError(error);
  return protocolError.status === status && !sensitive.test(protocolError.responseBody ?? "");
}

async function createEmpty(baseUrl: string, session: string, idempotencyKey: string, title: string) {
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
  return { created, fetchImpl };
}

test("getManuscriptTree returns the empty canonical tree and fail-closes across Project Scope", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const empty = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000711",
      "Tree Empty",
    );
    const tree = await getManuscriptTree({
      baseUrl,
      projectId: empty.created.project_scope.project_id,
      fetchImpl: empty.fetchImpl,
    });
    assert.equal(tree.schema_id, "storyos.query.manuscript-tree.response.v1");
    assert.equal(tree.project_scope.owner_user_id, USER_A);
    assert.equal(tree.project_scope.project_id, empty.created.project_scope.project_id);
    assert.deepEqual(tree.volumes, []);
    assert.equal(tree.snapshot.snapshot_kind, "canonical");
    assert.equal(tree.snapshot.project_scope.project_id, empty.created.project_scope.project_id);

    const second = await getManuscriptTree({
      baseUrl,
      projectId: empty.created.project_scope.project_id,
      fetchImpl: empty.fetchImpl,
    });
    assert.equal(second.snapshot.snapshot_id, tree.snapshot.snapshot_id);
    assert.equal(second.tree_revision, tree.tree_revision);

    await assert.rejects(
      getManuscriptTree({
        baseUrl,
        projectId: PROJECT_B,
        fetchImpl: browserFetch(baseUrl, "session-a"),
      }),
      (error) => sanitizedProtocolFailure(error, 404, /Project B|secret/),
    );

    const unauthenticated = await fetch(
      new URL(`/api/v1/projects/${empty.created.project_scope.project_id}/manuscript/tree`, baseUrl),
      { headers: { origin: baseUrl } },
    );
    assert.equal(unauthenticated.status, 401);

    const foreignOrigin = await fetch(
      new URL(`/api/v1/projects/${empty.created.project_scope.project_id}/manuscript/tree`, baseUrl),
      { headers: { origin: "https://foreign.example", cookie: "storyos_session=session-a" } },
    );
    assert.equal(foreignOrigin.status, 403);
    assert.doesNotMatch(await foreignOrigin.text(), /Tree Empty/);
  } finally {
    await stopRealServer(server);
  }
});
