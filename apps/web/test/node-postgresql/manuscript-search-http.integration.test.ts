import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createProject,
  createProjectChallenge,
  searchManuscript,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateProjectChallengeRequest, SearchManuscriptRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B = "018f0000-0000-7001-8000-000000000102";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function challengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000760",
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

function problemCode(error: unknown): unknown {
  const responseBody = requireStoryOSProtocolError(error).responseBody;
  assert.ok(responseBody);
  return Reflect.get(JSON.parse(responseBody), "code");
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

test("searchManuscript returns a ready empty page and fail-closes across Project Scope", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const empty = await createEmpty(
      baseUrl,
      "session-a",
      "018f0000-0000-7001-8000-000000000761",
      "Search Empty",
    );
    const projectId = empty.created.project_scope.project_id;
    const request: SearchManuscriptRequest = {
      schema_id: "storyos.query.manuscript-search.request.v1",
      selection: "manuscript",
      query_text: "alpha",
      required_watermark: null,
    };
    const page = await searchManuscript({
      baseUrl,
      projectId,
      fetchImpl: empty.fetchImpl,
      request,
    });
    assert.equal(page.schema_id, "storyos.query.manuscript-search.response.v1");
    assert.equal(page.project_scope.owner_user_id, USER_A);
    assert.equal(page.project_scope.project_id, projectId);
    assert.deepEqual(page.items, []);
    assert.equal(page.completeness, "complete");
    assert.equal(page.lag, "0");
    assert.equal(page.next_cursor, null);
    assert.equal(page.projection_kind, "manuscript_search");
    assert.match(page.source_snapshot.snapshot_id, UUID);
    assert.equal(page.source_snapshot.snapshot_kind, "canonical");
    assert.equal(page.projection_watermark, page.source_snapshot.project_activity_position);

    const rebuilt = await searchManuscript({
      baseUrl,
      projectId,
      fetchImpl: empty.fetchImpl,
      request,
    });
    assert.equal(rebuilt.source_snapshot.snapshot_id, page.source_snapshot.snapshot_id);
    assert.equal(rebuilt.projection_watermark, page.projection_watermark);
    assert.deepEqual(rebuilt.items, page.items);

    await assert.rejects(
      searchManuscript({
        baseUrl,
        projectId,
        fetchImpl: empty.fetchImpl,
        request: {
          ...request,
          required_watermark: String(Number(page.projection_watermark) + 1),
        },
      }),
      (error) => requireStoryOSProtocolError(error).status === 503
        && problemCode(error) === "projection_not_ready"
        && !/Search Empty/.test(requireStoryOSProtocolError(error).responseBody ?? ""),
    );

    await assert.rejects(
      searchManuscript({
        baseUrl,
        projectId,
        fetchImpl: empty.fetchImpl,
        request: { ...request, query_text: "" },
      }),
      (error) => sanitizedProtocolFailure(error, 400, /Search Empty/),
    );

    await assert.rejects(
      searchManuscript({
        baseUrl,
        projectId: PROJECT_B,
        fetchImpl: browserFetch(baseUrl, "session-a"),
        request,
      }),
      (error) => sanitizedProtocolFailure(error, 404, /Project B|secret|Search Empty/),
    );

    const searchPath = `/api/v1/projects/${projectId}/queries/manuscript-search`;
    const unauthenticated = await fetch(new URL(searchPath, baseUrl), {
      method: "POST",
      headers: { origin: baseUrl, "content-type": "application/json" },
      body: JSON.stringify(request),
    });
    assert.equal(unauthenticated.status, 401);

    const foreignOrigin = await fetch(new URL(searchPath, baseUrl), {
      method: "POST",
      headers: {
        origin: "https://foreign.example",
        "content-type": "application/json",
        cookie: "storyos_session=session-a",
      },
      body: JSON.stringify(request),
    });
    assert.equal(foreignOrigin.status, 403);
    assert.doesNotMatch(await foreignOrigin.text(), /Search Empty/);

    const disallowedGet = await fetch(new URL(searchPath, baseUrl), {
      headers: { origin: baseUrl, cookie: "storyos_session=session-a" },
    });
    assert.equal(disallowedGet.status, 405);
  } finally {
    await stopRealServer(server);
  }
});
