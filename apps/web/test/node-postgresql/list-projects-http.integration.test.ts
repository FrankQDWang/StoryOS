import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createProject,
  createProjectChallenge,
  getProject,
  listProjects,
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
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const PROJECT_B = "018f0000-0000-7001-8000-000000000102";
const CHAPTER_A = "018f0000-0000-7001-8000-000000000003";

function challengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000610",
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

test("listProjects returns only the current User library and getProject opens in Project Scope", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const listedA = await listProjects({
      baseUrl,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(listedA.schema_id, "storyos.query.project-list.response.v1");
    assert.equal(listedA.owner_user_id, USER_A);
    assert.deepEqual(
      listedA.projects.find((item) => item.project_scope.project_id === PROJECT_A),
      {
        project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
        title: "Project A",
        lifecycle: { kind: "active" },
        revision: "1",
        open: { kind: "current_chapter", current_chapter_id: CHAPTER_A },
      },
    );
    assert.equal(
      listedA.projects.some((item) => item.project_scope.project_id === PROJECT_B),
      false,
    );

    const listedB = await listProjects({
      baseUrl,
      fetchImpl: browserFetch(baseUrl, "session-b"),
    });
    assert.equal(listedB.owner_user_id, USER_B);
    assert.ok(listedB.projects.some((item) => item.title === "Project B secret"));
    assert.equal(
      listedB.projects.some((item) => item.project_scope.project_id === PROJECT_A),
      false,
    );

    await assert.rejects(
      getProject({
        baseUrl,
        projectId: PROJECT_B,
        fetchImpl: browserFetch(baseUrl, "session-a"),
      }),
      (error) => sanitizedProtocolFailure(error, 404, /Project B|secret/),
    );

    const empty = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000611",
      "Library Empty",
    );
    const listedAfterCreate = await listProjects({
      baseUrl,
      fetchImpl: empty.fetchImpl,
    });
    const emptyItem = listedAfterCreate.projects.find((item) => item.title === "Library Empty");
    if (emptyItem === undefined) {
      throw new Error("listed empty Project is missing");
    }
    assert.equal(emptyItem.project_scope.project_id, empty.created.project_scope.project_id);
    assert.equal(emptyItem.open.kind, "empty");
    assert.equal(emptyItem.lifecycle.kind, "active");

    const opened = await getProject({
      baseUrl,
      projectId: empty.created.project_scope.project_id,
      fetchImpl: empty.fetchImpl,
    });
    assert.equal(opened.project_scope.owner_user_id, USER_A);
    assert.equal(opened.project_scope.project_id, empty.created.project_scope.project_id);
    assert.equal(opened.project.title, "Library Empty");
    assert.equal(opened.project.open.kind, "empty");

    const listedBAfter = await listProjects({
      baseUrl,
      fetchImpl: browserFetch(baseUrl, "session-b"),
    });
    assert.equal(
      listedBAfter.projects.some((item) => item.title === "Library Empty"),
      false,
    );
  } finally {
    await stopRealServer(server);
  }
});
