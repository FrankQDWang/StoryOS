import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { request as httpRequest } from "node:http";
import type { IncomingHttpHeaders, RequestOptions } from "node:http";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";
import { promisify } from "node:util";

import {
  applyAuthorEdit, createEditorSession, createProjectCommandChallenge, digestApplyAuthorEdit,
  digestCreateEditorSession, getApplyAuthorEditOutcome, getChapter, getEditorSession, getProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  ApplyAuthorEditEffect,
  CreateEditorSessionRequest,
  CreateEditorSessionResponse,
  CreateProjectCommandChallengeRequest,
  DigestValue,
  DomainReceiptResult,
  GetApplyAuthorEditOutcomeResponse,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const CHAPTER_A = "018f0000-0000-7001-8000-000000000003";
const STALE_CHAPTER_A = "018f0000-0000-7001-8000-000000000006";
const UNKNOWN_CHAPTER = "018f0000-0000-7001-8000-00000000ffff";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B = "018f0000-0000-7001-8000-000000000102";
const CHAPTER_B = "018f0000-0000-7001-8000-000000000103";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const execFileAsync = promisify(execFile);

async function startRealServer(
  sessionUsers: Readonly<Record<string, string>> = { "session-a": USER_A, "session-b": USER_B },
) {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: sessionUsers,
  });
}

async function responseSnapshot(response: Response): Promise<{ body: string; status: number }> {
  return { status: response.status, body: await response.text() };
}

async function projectAuthoritySnapshot() {
  const query = `
    SELECT json_build_object(
      'receipts', (SELECT count(*) FROM storyos.domain_receipts
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'admissions', (SELECT count(*) FROM storyos.author_command_admissions
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
          AND command_kind = 'applyAuthorEdit'),
      'consumed_challenges', (SELECT count(*) FROM storyos.project_command_challenges
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
          AND command_kind = 'applyAuthorEdit' AND consumed_at IS NOT NULL),
      'activities', (SELECT count(*) FROM storyos.project_activity_events
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'revision_envelopes', (SELECT count(*) FROM storyos.authoritative_revision_envelopes
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'commits', (SELECT count(*) FROM storyos.authoritative_commits
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'actions', (SELECT count(*) FROM storyos.author_action_entries
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'counter', (SELECT concat(author_action_sequence, '/', authoritative_commit_sequence,
        '/', project_activity_position) FROM storyos.scope_counters
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
      'zero_authority_activities', (SELECT count(*)
        FROM storyos.domain_receipts AS receipt
        JOIN storyos.project_activity_events AS activity
          ON (activity.owner_user_id, activity.project_id, activity.receipt_id) =
             (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
        WHERE receipt.owner_user_id = '${USER_A}'::uuid
          AND receipt.project_id = '${PROJECT_A}'::uuid
          AND receipt.result_kind <> 'authoritative_applied')
    )::text`;
  return JSON.parse(await queryPostgres(query));
}

function httpGet(
  url: string | URL,
  headers: NonNullable<RequestOptions["headers"]>,
): Promise<{ body: string; headers: IncomingHttpHeaders; status: number | undefined }> {
  return new Promise((resolve, reject) => {
    const request = httpRequest(url, { headers }, (response) => {
      response.setEncoding("utf8");
      let body = "";
      response.on("data", (chunk) => { body += chunk; });
      response.on("end", () => resolve({
        status: response.statusCode, body, headers: response.headers,
      }));
    });
    request.on("error", reject);
    request.end();
  });
}

function protocolFailure(error: unknown, status: number): boolean {
  return requireStoryOSProtocolError(error).status === status;
}

function sanitizedProtocolFailure(
  error: unknown,
  status: number,
  sensitive: RegExp,
): boolean {
  const protocolError = requireStoryOSProtocolError(error);
  return protocolError.status === status && !sensitive.test(protocolError.responseBody ?? "");
}

function protocolProblem(error: unknown): unknown {
  const responseBody = requireStoryOSProtocolError(error).responseBody;
  assert.ok(responseBody);
  return Reflect.get(JSON.parse(responseBody), "code");
}

test("two authenticated Users open only their own Project and current Chapter over real HTTP", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const projectA = await getProject({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(projectA.project_scope.owner_user_id, USER_A);
    assert.deepEqual(projectA.project.open, { kind: "current_chapter", current_chapter_id: CHAPTER_A });
    const chapterA = await getChapter({
      baseUrl, projectId: PROJECT_A, chapterId: CHAPTER_A,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(chapterA.chapter.chapter_id, CHAPTER_A);
    assert.equal(chapterA.chapter.current_revision.body, "Authoritative A");

    const projectB = await getProject({
      baseUrl, projectId: PROJECT_B, fetchImpl: browserFetch(baseUrl, "session-b"),
    });
    assert.equal(projectB.project_scope.owner_user_id, USER_B);
    assert.deepEqual(projectB.project.open, { kind: "current_chapter", current_chapter_id: CHAPTER_B });
    const chapterB = await getChapter({
      baseUrl, projectId: PROJECT_B, chapterId: CHAPTER_B,
      fetchImpl: browserFetch(baseUrl, "session-b"),
    });
    assert.equal(chapterB.chapter.chapter_id, CHAPTER_B);
    assert.equal(chapterB.chapter.current_revision.body, "Authoritative B secret");

    await assert.rejects(
      getProject({
        baseUrl, projectId: PROJECT_B, fetchImpl: browserFetch(baseUrl, "session-a"),
      }),
      (error) => sanitizedProtocolFailure(error, 404, /Project B|secret/),
    );
  } finally {
    await stopRealServer(server);
  }
});

test("Project command challenges bind Origin, Scope, nonce, and idempotency on real PostgreSQL", async () => {
  const { baseUrl, server } = await startRealServer();
  const request: CreateProjectCommandChallengeRequest = {
    method: "PATCH",
    route_template: "/api/v1/projects/{project_id}",
    command_schema: "storyos.command.update-project.request.v1",
    canonical_command_digest: {
      algorithm: "sha256",
      profile: "storyos.command.updateProject.jcs.v1",
      value_hex_lowercase: "d".repeat(64),
    },
    idempotency_key: "018f0000-0000-7001-8000-000000000014",
  };
  try {
    const options = {
      baseUrl, projectId: PROJECT_A, request, fetchImpl: browserFetch(baseUrl, "session-a"),
    };
    const first = await createProjectCommandChallenge(options);
    const retry = await createProjectCommandChallenge(options);
    assert.deepEqual(retry, first);
    assert.match(first.nonce, /^[0-9a-f]{64}$/);
    assert.equal(first.limit_profile_revision, "storyos.foundation.absolute.v1");

    await assert.rejects(
      createProjectCommandChallenge({
        ...options,
        request: {
          ...request,
          canonical_command_digest: { ...request.canonical_command_digest, value_hex_lowercase: "e".repeat(64) },
        },
      }),
      (error) => protocolFailure(error, 409),
    );
    await assert.rejects(
      createProjectCommandChallenge({
        ...options,
        request: {
          ...request,
          idempotency_key: "018f0000-0000-7001-8000-000000000017",
          canonical_command_digest: { ...request.canonical_command_digest, profile: "storyos.command.other.jcs.v1" },
        },
      }),
      (error) => protocolFailure(error, 400),
    );
    const editorSession = await createProjectCommandChallenge({
      ...options,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/editor-sessions",
        command_schema: "storyos.command.create-editor-session.request.v1",
        canonical_command_digest: {
          algorithm: "sha256",
          profile: "storyos.command.createEditorSession.jcs.v1",
          value_hex_lowercase: "f".repeat(64),
        },
        idempotency_key: "018f0000-0000-7001-8000-000000000018",
      },
    });
    assert.match(editorSession.nonce, /^[0-9a-f]{64}$/);
    await assert.rejects(
      createProjectCommandChallenge({
        ...options, projectId: PROJECT_B,
        request: { ...request, idempotency_key: "018f0000-0000-7001-8000-000000000015" },
      }),
      (error) => sanitizedProtocolFailure(error, 404, /Project B|secret/),
    );

    const refererOnly = await fetch(new URL(`/api/v1/projects/${PROJECT_A}/anti-forgery-challenges`, baseUrl), {
      method: "POST",
      headers: {
        referer: `${baseUrl}/projects/${PROJECT_A}`,
        cookie: "storyos_session=session-a",
        "content-type": "application/json",
      },
      body: JSON.stringify({ ...request, idempotency_key: "018f0000-0000-7001-8000-000000000016" }),
    });
    assert.equal(refererOnly.status, 403);

  } finally {
    await stopRealServer(server);
  }
});

test("one current writer settles one Author Edit and exact retries return one result", async () => {
  const { baseUrl, server } = await startRealServer({
    "session-a": USER_A, "session-a-alt": USER_A, "session-b": USER_B,
  });
  const requests: readonly [CreateEditorSessionRequest, CreateEditorSessionRequest] = [
    {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000020",
    },
    {
      command_schema: "storyos.command.create-editor-session.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000021",
    },
  ];
  const keys: readonly [string, string] = [
    "018f0000-0000-7001-8000-000000000022",
    "018f0000-0000-7001-8000-000000000023",
  ];
  try {
    assert.deepEqual(await digestCreateEditorSession(requests[0]), await digestCreateEditorSession({
      correlation_id: requests[0].correlation_id,
      security_policy_revision: requests[0].security_policy_revision,
      command_schema: requests[0].command_schema,
      client_contract_revision: requests[0].client_contract_revision,
    }));
    const challenges = await Promise.all(requests.map(async (request, index) => {
      const idempotencyKey = keys[index];
      assert.ok(idempotencyKey);
      return createProjectCommandChallenge({
        baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
        request: {
          method: "POST",
          route_template: "/api/v1/projects/{project_id}/editor-sessions",
          command_schema: request.command_schema,
          canonical_command_digest: await digestCreateEditorSession(request),
          idempotency_key: idempotencyKey,
        },
      });
    }));
    const firstChallenge = challenges.at(0);
    assert.ok(firstChallenge);
    const sessionUrl = new URL(`/api/v1/projects/${PROJECT_A}/editor-sessions`, baseUrl);
    const refererOnly = await fetch(sessionUrl, {
      method: "POST",
      headers: { referer: `${baseUrl}/projects/${PROJECT_A}`, cookie: "storyos_session=session-a",
        "content-type": "application/json", "idempotency-key": keys[0],
        "x-storyos-anti-forgery": firstChallenge.nonce },
      body: JSON.stringify(requests[0]),
    });
    assert.equal(refererOnly.status, 403);
    const missingChallenge = await fetch(sessionUrl, {
      method: "POST",
      headers: { origin: baseUrl, cookie: "storyos_session=session-a",
        "content-type": "application/json", "idempotency-key": keys[0] },
      body: JSON.stringify(requests[0]),
    });
    assert.equal(missingChallenge.status, 400);
    await assert.rejects(createEditorSession({
      baseUrl, projectId: PROJECT_A, request: requests[0], idempotencyKey: keys[0],
      antiForgery: firstChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-b"),
    }), (error) => sanitizedProtocolFailure(error, 422, /Project A|Authoritative A/));
    const sessions = await Promise.all(requests.map((request, index) => {
      const idempotencyKey = keys[index];
      const challenge = challenges[index];
      assert.ok(idempotencyKey);
      assert.ok(challenge);
      return createEditorSession({
        baseUrl, projectId: PROJECT_A, request, idempotencyKey,
        antiForgery: challenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
      });
    }));
    assert.equal(sessions.filter((session) => session.writer.kind === "current_writer").length, 1);
    assert.equal(sessions.filter((session) => session.writer.kind === "read_only").length, 1);
    assert.deepEqual(new Set(sessions.map((session) => session.writer.kind === "current_writer"
      ? session.writer.writer_generation
      : session.writer.observed_writer_generation)), new Set(["1"]));
    const currentIndex = sessions.findIndex((session) => session.writer.kind === "current_writer");
    assert.notEqual(currentIndex, -1);
    const currentSession = sessions[currentIndex];
    const currentRequest = requests[currentIndex];
    const currentKey = keys[currentIndex];
    const currentChallenge = challenges[currentIndex];
    assert.ok(currentSession);
    assert.ok(currentRequest);
    assert.ok(currentKey);
    assert.ok(currentChallenge);
    if (currentSession.writer.kind !== "current_writer") {
      throw new Error("the selected Editor Session was not the current writer");
    }
    assert.deepEqual(await createEditorSession({
      baseUrl, projectId: PROJECT_A, request: currentRequest,
      idempotencyKey: currentKey, antiForgery: currentChallenge.nonce,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), currentSession);
    const read = await getEditorSession({
      baseUrl, projectId: PROJECT_A,
      editorSessionId: currentSession.editor_session.editor_session_id,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    const { schema_id: createSchema, correlation_id: createCorrelation, ...created } = currentSession;
    const { schema_id: readSchema, correlation_id: readCorrelation, ...readPayload } = read;
    assert.deepEqual(readPayload, created);
    assert.equal(createSchema, "storyos.command.create-editor-session.response.v1");
    assert.equal(readSchema, "storyos.query.editor-session.response.v1");
    assert.match(createCorrelation, UUID);
    assert.match(readCorrelation, UUID);
    await assert.rejects(getEditorSession({
      baseUrl, projectId: PROJECT_A,
      editorSessionId: currentSession.editor_session.editor_session_id,
      fetchImpl: browserFetch(baseUrl, "session-b"),
    }), (error) => sanitizedProtocolFailure(error, 404, /Project A|Authoritative A/));
    await assert.rejects(createEditorSession({
      baseUrl, projectId: PROJECT_A,
      request: { ...currentRequest, correlation_id: "018f0000-0000-7001-8000-000000000099" },
      idempotencyKey: currentKey, antiForgery: currentChallenge.nonce,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => protocolFailure(error, 422));

    const authorEditRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000024",
      editor_session_id: currentSession.editor_session.editor_session_id,
      writer_generation: currentSession.writer.writer_generation,
      chapter_id: currentSession.base_snapshot.chapter_id,
      expected_authoritative_revision_id:
        currentSession.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids:
        currentSession.base_snapshot.proposal_head_revision_ids,
      target_refs: currentSession.base_snapshot.target_refs,
      observed_ownership_partition:
        currentSession.base_snapshot.observed_ownership_partition,
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
      undo_group_id: "018f0000-0000-7001-8000-000000000025",
      completed_intent_record_id: "018f0000-0000-7001-8000-000000000026",
      local_intent_sequence: "1",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from: 15, to: 15, text: "x" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 15, to: 15,
        },
      }, {
        normalized_primitives: [{ kind: "replace_selection", from: 15, to: 16, text: "!" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 15, to: 16,
        },
      }],
    };
    const emptyAuthority = {
      receipts: 0,
      admissions: 0,
      consumed_challenges: 0,
      activities: 0,
      revision_envelopes: 0,
      commits: 0,
      actions: 0,
      counter: null,
      zero_authority_activities: 0,
    };
    const firstAuthorEditUnit = authorEditRequest.author_edit_units.at(0);
    assert.ok(firstAuthorEditUnit);
    const firstAuthorEditPrimitive = firstAuthorEditUnit.normalized_primitives.at(0);
    assert.ok(firstAuthorEditPrimitive);
    const limitCases: Array<{
      author_edit_units: ApplyAuthorEditRequest["author_edit_units"];
      name: string;
      status: number;
      suffix: string;
    }> = [
      {
        name: "unit count",
        status: 422,
        suffix: "080",
        author_edit_units: Array.from({ length: 241 }, () => firstAuthorEditUnit),
      },
      {
        name: "primitive count",
        status: 422,
        suffix: "081",
        author_edit_units: [{
          ...firstAuthorEditUnit,
          normalized_primitives: Array.from(
            { length: 241 }, () => firstAuthorEditPrimitive,
          ),
        }],
      },
      {
        name: "wire body bytes",
        status: 413,
        suffix: "082",
        author_edit_units: [{
          normalized_primitives: [{
            kind: "replace_selection", from: 15, to: 15, text: "x".repeat(1048576),
          }],
          selection_snapshot: {
            coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 15, to: 15,
          },
        }],
      },
    ];
    for (const limitCase of limitCases) {
      const request: ApplyAuthorEditRequest = {
        ...authorEditRequest,
        correlation_id: `018f0000-0000-7001-8000-000000000${limitCase.suffix}`,
        undo_group_id: `018f0000-0000-7001-8000-000000000${String(
          Number(limitCase.suffix) + 10,
        ).padStart(3, "0")}`,
        completed_intent_record_id:
          `018f0000-0000-7001-8000-000000000${String(
            Number(limitCase.suffix) + 20,
          ).padStart(3, "0")}`,
        author_edit_units: limitCase.author_edit_units,
      };
      const idempotencyKey =
        `018f0000-0000-7001-8000-000000000${String(
          Number(limitCase.suffix) + 30,
        ).padStart(3, "0")}`;
      await assert.rejects(applyAuthorEdit({
        baseUrl, projectId: PROJECT_A, request, idempotencyKey,
        antiForgery: "0".repeat(64), fetchImpl: browserFetch(baseUrl, "session-a"),
      }), (error) => protocolFailure(error, limitCase.status), limitCase.name);
      assert.deepEqual(await projectAuthoritySnapshot(), emptyAuthority, limitCase.name);
    }
    const authorEditKey = "018f0000-0000-7001-8000-000000000027";
    const authorEditChallenge = await createProjectCommandChallenge({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
        command_schema: authorEditRequest.command_schema,
        canonical_command_digest: await digestApplyAuthorEdit(authorEditRequest),
        idempotency_key: authorEditKey,
      },
    });
    let pendingOutcomeResponse: Response | undefined;
    const pendingOutcome = await getApplyAuthorEditOutcome({
      baseUrl, projectId: PROJECT_A, idempotencyKey: authorEditKey,
      antiForgery: authorEditChallenge.nonce,
      fetchImpl: async (url, options) => {
        pendingOutcomeResponse = await browserFetch(baseUrl, "session-a")(url, options);
        return pendingOutcomeResponse;
      },
    });
    const { correlation_id: pendingQueryCorrelation, ...pendingOutcomePayload } = pendingOutcome;
    assert.match(pendingQueryCorrelation, UUID);
    assert.notEqual(pendingQueryCorrelation, authorEditRequest.correlation_id);
    assert.ok(pendingOutcomeResponse);
    assert.equal(pendingOutcomeResponse.headers.get("cache-control"), "no-store");
    assert.deepEqual(pendingOutcomePayload, {
      schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
      project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
      outcome: {
        outcome_kind: "still_unknown",
        observation: {
          observation_kind: "challenge_issued",
          expires_at: authorEditChallenge.expires_at,
        },
      },
    });
    assert.deepEqual(await projectAuthoritySnapshot(), emptyAuthority);
    const pendingOutcomeUrl = new URL(
      `/api/v1/projects/${PROJECT_A}/manuscript/author-edit-outcomes/${authorEditKey}`,
      baseUrl,
    );
    const refererOnlyOutcome = await fetch(pendingOutcomeUrl, {
      headers: {
        referer: `${baseUrl}/projects/${PROJECT_A}?panel=journal`,
        cookie: "storyos_session=session-a",
        "x-storyos-anti-forgery": authorEditChallenge.nonce,
      },
    });
    assert.equal(refererOnlyOutcome.status, 200);
    assert.equal(refererOnlyOutcome.headers.get("cache-control"), "no-store");
    assert.equal((await refererOnlyOutcome.json()).outcome.outcome_kind, "still_unknown");
    const hostileOrigin = await fetch(pendingOutcomeUrl, {
      headers: {
        origin: "https://attacker.invalid",
        referer: `${baseUrl}/projects/${PROJECT_A}`,
        cookie: "storyos_session=session-a",
        "x-storyos-anti-forgery": authorEditChallenge.nonce,
      },
    });
    assert.equal(hostileOrigin.status, 403);
    assert.equal(hostileOrigin.headers.get("cache-control"), "no-store");
    assert.equal((await hostileOrigin.json()).code, "request_site_refused");
    const unauthenticatedOutcome = await fetch(pendingOutcomeUrl, {
      headers: {
        origin: baseUrl,
        "x-storyos-anti-forgery": authorEditChallenge.nonce,
      },
    });
    assert.equal(unauthenticatedOutcome.status, 401);
    assert.equal(unauthenticatedOutcome.headers.get("cache-control"), "no-store");
    assert.equal((await unauthenticatedOutcome.json()).code, "authentication_required");
    const unavailableCases = [
      {
        url: pendingOutcomeUrl,
        headers: {
          origin: baseUrl,
          cookie: "storyos_session=session-a",
          "x-storyos-anti-forgery": "0".repeat(64),
        },
      },
      {
        url: new URL(
          `/api/v1/projects/${PROJECT_A}/manuscript/author-edit-outcomes/`
            + "018f0000-0000-7001-8000-000000000121",
          baseUrl,
        ),
        headers: {
          origin: baseUrl,
          cookie: "storyos_session=session-a",
          "x-storyos-anti-forgery": authorEditChallenge.nonce,
        },
      },
      {
        url: pendingOutcomeUrl,
        headers: {
          origin: baseUrl,
          cookie: "storyos_session=session-a-alt",
          "x-storyos-anti-forgery": authorEditChallenge.nonce,
        },
      },
      {
        url: new URL(
          `/api/v1/projects/${PROJECT_B}/manuscript/author-edit-outcomes/${authorEditKey}`,
          baseUrl,
        ),
        headers: {
          origin: baseUrl,
          cookie: "storyos_session=session-b",
          "x-storyos-anti-forgery": authorEditChallenge.nonce,
        },
      },
    ];
    const unavailableProblem = {
      schema_id: "storyos.problem.v1",
      code: "resource_unavailable",
      message: "The requested resource is unavailable.",
    };
    for (const unavailableCase of unavailableCases) {
      const response = await fetch(unavailableCase.url, { headers: unavailableCase.headers });
      assert.equal(response.status, 404);
      assert.equal(response.headers.get("cache-control"), "no-store");
      const problemBody = await response.json();
      assert.deepEqual(problemBody, unavailableProblem);
      assert.ok(!JSON.stringify(problemBody).includes(authorEditChallenge.nonce));
      assert.ok(!JSON.stringify(problemBody).includes(authorEditKey));
    }
    for (const invalidProof of [undefined, "A".repeat(64)]) {
      const headers = new Headers({ origin: baseUrl, cookie: "storyos_session=session-a" });
      if (invalidProof !== undefined) headers.set("x-storyos-anti-forgery", invalidProof);
      const response = await fetch(pendingOutcomeUrl, { headers });
      assert.equal(response.status, 400);
      assert.equal(response.headers.get("cache-control"), "no-store");
      await response.arrayBuffer();
    }
    const repeatedProof = await httpGet(pendingOutcomeUrl, {
      origin: baseUrl,
      cookie: "storyos_session=session-a",
      "x-storyos-anti-forgery": [authorEditChallenge.nonce, authorEditChallenge.nonce],
    });
    assert.equal(repeatedProof.status, 400);
    assert.equal(repeatedProof.headers["cache-control"], "no-store");
    const wrongMethod = await fetch(pendingOutcomeUrl, {
      method: "POST",
      headers: {
        origin: baseUrl,
        cookie: "storyos_session=session-a",
        "x-storyos-anti-forgery": authorEditChallenge.nonce,
      },
    });
    assert.equal(wrongMethod.status, 405);
    assert.equal(wrongMethod.headers.get("cache-control"), "no-store");
    assert.deepEqual(await wrongMethod.json(), {
      schema_id: "storyos.problem.v1",
      code: "method_not_allowed",
      message: "The request method is not allowed.",
    });
    assert.deepEqual(await projectAuthoritySnapshot(), emptyAuthority);
    const rejectedKey = "018f0000-0000-7001-8000-000000000120";
    const rejectedChallenge = await createProjectCommandChallenge({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
        command_schema: authorEditRequest.command_schema,
        canonical_command_digest: await digestApplyAuthorEdit(authorEditRequest),
        idempotency_key: rejectedKey,
      },
    });
    const postgresContainer = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
    assert.ok(postgresContainer, "run through scripts/verify-project-scope.sh");
    await execFileAsync("docker", [
      "exec", postgresContainer, "psql", "-X", "-v", "ON_ERROR_STOP=1", "-U", "postgres",
      "-c", `UPDATE storyos.project_command_challenges
        SET expires_at = clock_timestamp() - interval '1 second'
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
          AND command_kind = 'applyAuthorEdit' AND idempotency_key = '${rejectedKey}'::uuid`,
    ]);
    const rejectedOutcome = await getApplyAuthorEditOutcome({
      baseUrl, projectId: PROJECT_A, idempotencyKey: rejectedKey,
      antiForgery: rejectedChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    const { correlation_id: rejectedCorrelation, ...rejectedPayload } = rejectedOutcome;
    assert.match(rejectedCorrelation, UUID);
    assert.deepEqual(rejectedPayload, {
      schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
      project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
      outcome: {
        outcome_kind: "rejected",
        reason: "challenge_expired_unconsumed",
      },
    });
    assert.deepEqual(await projectAuthoritySnapshot(), emptyAuthority);
    const authorEditUrl = new URL(
      `/api/v1/projects/${PROJECT_A}/manuscript/author-edits`, baseUrl,
    );
    const refererOnlyAuthorEdit = await fetch(authorEditUrl, {
      method: "POST",
      headers: {
        referer: `${baseUrl}/projects/${PROJECT_A}`,
        cookie: "storyos_session=session-a",
        "content-type": "application/json",
        "idempotency-key": authorEditKey,
        "x-storyos-anti-forgery": authorEditChallenge.nonce,
      },
      body: JSON.stringify(authorEditRequest),
    });
    assert.equal(refererOnlyAuthorEdit.status, 403);
    const authorEditOptions = {
      baseUrl,
      projectId: PROJECT_A,
      request: authorEditRequest,
      idempotencyKey: authorEditKey,
      antiForgery: authorEditChallenge.nonce,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    };
    await assert.rejects(applyAuthorEdit({
      ...authorEditOptions,
      fetchImpl: async (url, options) => {
        const response = await browserFetch(baseUrl, "session-a")(url, options);
        await response.arrayBuffer();
        throw new Error("simulated acknowledgement delivery loss");
      },
    }), /simulated acknowledgement delivery loss/);
    let committedOutcomeResponse: Response | undefined;
    const committedOutcome = await getApplyAuthorEditOutcome({
      baseUrl, projectId: PROJECT_A, idempotencyKey: authorEditKey,
      antiForgery: authorEditChallenge.nonce,
      fetchImpl: async (url, options) => {
        committedOutcomeResponse = await browserFetch(baseUrl, "session-a")(url, options);
        return committedOutcomeResponse;
      },
    });
    assert.ok(committedOutcomeResponse);
    assert.equal(committedOutcomeResponse.headers.get("cache-control"), "no-store");
    assert.match(committedOutcome.correlation_id, UUID);
    assert.notEqual(committedOutcome.correlation_id, authorEditRequest.correlation_id);
    if (committedOutcome.outcome.outcome_kind !== "committed") {
      throw new Error("the lost acknowledgement did not settle as committed");
    }
    const settlement = committedOutcome.outcome.response;
    assert.equal(settlement.correlation_id, authorEditRequest.correlation_id);
    assert.equal(settlement.receipt.result, "authoritative_applied");
    if (settlement.effect.kind !== "authoritative_applied") {
      throw new Error("the settled Author Edit was not authoritative");
    }
    assert.equal(settlement.effect.authoritative_revision.body, "Authoritative A!");
    assert.equal(settlement.completed_intent_record_id,
      authorEditRequest.completed_intent_record_id);
    assert.deepEqual(await applyAuthorEdit(authorEditOptions), settlement);
    const authorityAfterLostAcknowledgement = await projectAuthoritySnapshot();
    const repeatedCommitted = await getApplyAuthorEditOutcome({
      baseUrl, projectId: PROJECT_A, idempotencyKey: authorEditKey,
      antiForgery: authorEditChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    if (repeatedCommitted.outcome.outcome_kind !== "committed") {
      throw new Error("the repeated outcome did not stay committed");
    }
    assert.deepEqual(repeatedCommitted.outcome.response, settlement);
    assert.deepEqual(await projectAuthoritySnapshot(), authorityAfterLostAcknowledgement);

    const outcomeRequests: Array<{
      effect: ApplyAuthorEditEffect;
      expected: DomainReceiptResult;
      request: ApplyAuthorEditRequest;
    }> = [
      {
        expected: "conflicted",
        effect: {
          kind: "conflicted",
          reason: "stale_authoritative_head",
          current_authoritative_revision_id: settlement.effect.authoritative_revision.revision_id,
        },
        request: {
          ...authorEditRequest,
          correlation_id: "018f0000-0000-7001-8000-000000000040",
          undo_group_id: "018f0000-0000-7001-8000-000000000041",
          completed_intent_record_id: "018f0000-0000-7001-8000-000000000042",
          local_intent_sequence: "2",
        },
      },
      {
        expected: "no_effect",
        effect: { kind: "no_effect", reason: "content_unchanged" },
        request: {
          ...authorEditRequest,
          correlation_id: "018f0000-0000-7001-8000-000000000043",
          expected_authoritative_revision_id: settlement.effect.authoritative_revision.revision_id,
          undo_group_id: "018f0000-0000-7001-8000-000000000044",
          completed_intent_record_id: "018f0000-0000-7001-8000-000000000045",
          local_intent_sequence: "3",
          author_edit_units: [{
            normalized_primitives: [{ kind: "replace_selection", from: 16, to: 16, text: "" }],
            selection_snapshot: {
              coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 16, to: 16,
            },
          }],
        },
      },
      {
        expected: "refused",
        effect: { kind: "refused", reason: "invalid_selection" },
        request: {
          ...authorEditRequest,
          correlation_id: "018f0000-0000-7001-8000-000000000046",
          expected_authoritative_revision_id: settlement.effect.authoritative_revision.revision_id,
          undo_group_id: "018f0000-0000-7001-8000-000000000047",
          completed_intent_record_id: "018f0000-0000-7001-8000-000000000048",
          local_intent_sequence: "4",
          author_edit_units: [{
            normalized_primitives: [{ kind: "replace_selection", from: 16, to: 16, text: "?" }],
            selection_snapshot: {
              coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 16, to: 15,
            },
          }],
        },
      },
    ];
    for (const [index, outcome] of outcomeRequests.entries()) {
      const idempotencyKey = `018f0000-0000-7001-8000-00000000005${index}`;
      const challenge = await createProjectCommandChallenge({
        baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
        request: {
          method: "POST",
          route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
          command_schema: outcome.request.command_schema,
          canonical_command_digest: await digestApplyAuthorEdit(outcome.request),
          idempotency_key: idempotencyKey,
        },
      });
      const options = {
        baseUrl, projectId: PROJECT_A, request: outcome.request, idempotencyKey,
        antiForgery: challenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
      };
      const result = await applyAuthorEdit(options);
      assert.equal(result.receipt.result, outcome.expected);
      assert.deepEqual(result.effect, outcome.effect);
      assert.deepEqual(result.receipt.authoritative_revision_ids, []);
      assert.deepEqual(result.receipt.authoritative_commit_ids, []);
      assert.equal(result.receipt.author_action_sequence, null);
      assert.deepEqual(await applyAuthorEdit(options), result);
      const authorityBeforeOutcomeRead = await projectAuthoritySnapshot();
      const queried = await getApplyAuthorEditOutcome({
        baseUrl, projectId: PROJECT_A, idempotencyKey,
        antiForgery: challenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
      });
      assert.match(queried.correlation_id, UUID);
      assert.notEqual(queried.correlation_id, result.correlation_id);
      if (queried.outcome.outcome_kind !== "committed") {
        throw new Error("the settled zero-authority outcome was unavailable");
      }
      assert.deepEqual(queried.outcome.response, result);
      assert.deepEqual(await projectAuthoritySnapshot(), authorityBeforeOutcomeRead);
    }

    assert.deepEqual(await projectAuthoritySnapshot(), {
      receipts: 4,
      admissions: 4,
      consumed_challenges: 4,
      activities: 1,
      revision_envelopes: 1,
      commits: 1,
      actions: 1,
      counter: "1/1/1",
      zero_authority_activities: 0,
    });

    const unknownKey = rejectedKey;
    const unknownAdmission = "018f0000-0000-7001-8000-000000000065";
    const unknownCommand = "018f0000-0000-7001-8000-000000000066";
    const unknownChallenge = rejectedChallenge;
    await queryPostgres(`WITH challenge AS (
      UPDATE storyos.project_command_challenges SET consumed_at = clock_timestamp(), expires_at = clock_timestamp() + interval '5 minutes'
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid AND command_kind = 'applyAuthorEdit' AND idempotency_key = '${unknownKey}'::uuid RETURNING consumed_at, expires_at
    ), progress AS (
      UPDATE storyos.command_idempotency SET outcome_kind = 'in_progress'
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid AND command_kind = 'applyAuthorEdit' AND idempotency_key = '${unknownKey}'::uuid RETURNING 1
    ), source AS (
      SELECT admission FROM storyos.author_command_admissions AS admission
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid AND idempotency_key = '${authorEditKey}'::uuid
    ) INSERT INTO storyos.author_command_admissions
      SELECT (jsonb_populate_record(NULL::storyos.author_command_admissions,
        to_jsonb(source.admission) || jsonb_build_object(
          'author_command_admission_id', '${unknownAdmission}', 'command_id', '${unknownCommand}',
          'idempotency_key', '${unknownKey}', 'challenge_consumed_at', challenge.consumed_at,
          'challenge_expires_at', challenge.expires_at))).* FROM source, challenge, progress`);
    const unknownOptions = {
      baseUrl, projectId: PROJECT_A, idempotencyKey: unknownKey,
      antiForgery: unknownChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    };
    const unknownObservationCount = () => queryPostgres(`SELECT count(*)
      FROM storyos.author_command_admission_outcome_unknown_observations
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
        AND author_command_admission_id = '${unknownAdmission}'::uuid`);
    assert.equal(await unknownObservationCount(), "0");
    const unknownBefore = await getApplyAuthorEditOutcome(unknownOptions);
    assert.deepEqual(unknownBefore.outcome, { outcome_kind: "still_unknown", observation: {
      observation_kind: "admission_committed", command_id: unknownCommand,
      author_command_admission_id: unknownAdmission, reconciliation_required: true,
    } });
    assert.equal(await unknownObservationCount(), "0");
    const authorityBeforeObservation = await projectAuthoritySnapshot();
    await queryPostgres(`INSERT INTO storyos.author_command_admission_outcome_unknown_observations
      (owner_user_id, project_id, observation_id, author_command_admission_id, last_provable_boundary, reason)
      VALUES ('${USER_A}'::uuid, '${PROJECT_A}'::uuid, '018f0000-0000-7001-8000-000000000067'::uuid, '${unknownAdmission}'::uuid,
       'admission_committed', 'acknowledgement_missing')`);
    assert.equal(await unknownObservationCount(), "1");
    const unknownAfter = await getApplyAuthorEditOutcome(unknownOptions);
    assert.deepEqual(unknownAfter.outcome, unknownBefore.outcome);
    assert.equal(await unknownObservationCount(), "1");
    assert.deepEqual(await projectAuthoritySnapshot(), authorityBeforeObservation);

    const staleSession = sessions.find((session) => session.writer.kind === "read_only");
    assert.ok(staleSession);
    if (staleSession.writer.kind !== "read_only") {
      throw new Error("the stale Editor Session unexpectedly became current");
    }
    const staleRequest: ApplyAuthorEditRequest = {
      ...authorEditRequest,
      correlation_id: "018f0000-0000-7001-8000-000000000060",
      editor_session_id: staleSession.editor_session.editor_session_id,
      writer_generation: staleSession.writer.observed_writer_generation,
      undo_group_id: "018f0000-0000-7001-8000-000000000061",
      completed_intent_record_id: "018f0000-0000-7001-8000-000000000062",
      local_intent_sequence: "5",
    };
    const staleKey = "018f0000-0000-7001-8000-000000000063";
    const staleChallenge = await createProjectCommandChallenge({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
        command_schema: staleRequest.command_schema,
        canonical_command_digest: await digestApplyAuthorEdit(staleRequest),
        idempotency_key: staleKey,
      },
    });
    await assert.rejects(applyAuthorEdit({
      baseUrl, projectId: PROJECT_A, request: staleRequest, idempotencyKey: staleKey,
      antiForgery: "0".repeat(64), fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => protocolFailure(error, 422)
      && protocolProblem(error) === "challenge_invalid");
    await assert.rejects(applyAuthorEdit({
      baseUrl, projectId: PROJECT_A, request: staleRequest, idempotencyKey: staleKey,
      antiForgery: staleChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => protocolFailure(error, 412)
      && protocolProblem(error) === "editor_writer_stale");
  } finally {
    await stopRealServer(server);
  }
});

test("the real challenge endpoint returns a sanitized rate refusal with retry time", async () => {
  const { baseUrl, server } = await startRealServer();
  const request: CreateProjectCommandChallengeRequest = {
    method: "PATCH",
    route_template: "/api/v1/projects/{project_id}",
    command_schema: "storyos.command.update-project.request.v1",
    canonical_command_digest: {
      algorithm: "sha256",
      profile: "storyos.command.updateProject.jcs.v1",
      value_hex_lowercase: "a".repeat(64),
    },
    idempotency_key: "018f0000-0000-7001-8000-000000000900",
  };
  try {
    const options = {
      baseUrl, projectId: PROJECT_B, request, fetchImpl: browserFetch(baseUrl, "session-b"),
    };
    const results = await Promise.allSettled(Array.from({ length: 21 }, (_, offset) =>
      createProjectCommandChallenge({
        ...options,
        request: {
          ...request,
          idempotency_key: `018f0000-0000-7001-8000-${String(900 + offset).padStart(12, "0")}`,
          canonical_command_digest: {
            ...request.canonical_command_digest,
            value_hex_lowercase: (offset % 16).toString(16).repeat(64),
          },
        },
      })));
    const refusals = results.filter((result) => result.status === "rejected");
    assert.ok(refusals.length >= 1);
    for (const refusal of refusals) {
      const error = refusal.reason;
      const protocolError = requireStoryOSProtocolError(error);
      assert.equal(protocolError.status, 429);
      assert.match(String(protocolError.retryAfterSeconds), /^(?:[1-9]|[1-5][0-9]|60)$/);
      assert.deepEqual(JSON.parse(protocolError.responseBody ?? "{}"), {
        schema_id: "storyos.problem.v1",
        code: "challenge_rate_limited",
        message: "The command challenge rate limit is exceeded.",
      });
      assert.doesNotMatch(
        protocolError.responseBody ?? "",
        new RegExp(`${USER_B}|${PROJECT_B}|nonce|sha256`),
      );
    }
  } finally {
    await stopRealServer(server);
  }
});

test("a sensitive Project read accepts same-origin Referer paths and queries", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    for (const referer of [
      `${baseUrl}?view=project`,
      `${baseUrl}/projects/${PROJECT_A}?view=editor`,
    ]) {
      const response = await fetch(new URL(`/api/v1/projects/${PROJECT_A}`, baseUrl), {
        headers: { referer, cookie: "storyos_session=session-a" },
      });

      assert.equal(response.status, 200, referer);
      assert.match(await response.text(), /Project A/, referer);
    }
  } finally {
    await stopRealServer(server);
  }
});

test("hostile Origin and Referer inputs refuse without Project disclosure", async () => {
  const { baseUrl, server } = await startRealServer();
  const url = new URL(`/api/v1/projects/${PROJECT_A}`, baseUrl);
  const cookie = "storyos_session=session-a";
  try {
    const foreignPort = new URL(baseUrl);
    foreignPort.port = foreignPort.port === "65535" ? "65534" : String(Number(foreignPort.port) + 1);
    const sameOriginReferer = `${baseUrl}/projects/${PROJECT_A}`;
    const hostileHeaders: Array<readonly [string, HeadersInit]> = [
      ["missing Origin and Referer", { cookie }],
      ["normalized Origin path", { origin: `${baseUrl}/foo/..`, cookie }],
      ["invalid Origin with valid Referer", {
        origin: `${baseUrl}/%story`, referer: sameOriginReferer, cookie,
      }],
      ["empty Origin with valid Referer", { origin: "", referer: sameOriginReferer, cookie }],
      ["different non-default port", { origin: foreignPort.origin, cookie }],
      ["cross-origin Referer", { referer: "https://foreign.example/project", cookie }],
      ["relative Referer", { referer: "/relative/project", cookie }],
      ["invalid Referer port", { referer: "http://127.0.0.1:99999/project", cookie }],
      ["malformed IPv6 Referer", { referer: "http://[::1/project", cookie }],
      ["userinfo Referer", { referer: baseUrl.replace("://", "://user@"), cookie }],
      ["fragment Referer", { referer: `${baseUrl}/project#fragment`, cookie }],
      ["opaque Referer", { referer: "data:text/plain,story", cookie }],
      ["non-HTTP Referer", { referer: "ftp://example.com/project", cookie }],
      ["backslash recovery", { referer: baseUrl.replace("://", ":\\\\"), cookie }],
      ["missing slashes recovery", { referer: baseUrl.replace("://", ":"), cookie }],
      ["invalid percent escape", { referer: `${baseUrl}/%story`, cookie }],
      ["unencoded at-sign", { referer: baseUrl.replace("://", "://user@name@"), cookie }],
    ];
    const cases: Array<readonly [
      string,
      { body: string; status: number | undefined },
    ]> = [];
    for (const [name, headers] of hostileHeaders) {
      cases.push([name, await responseSnapshot(await fetch(url, { headers }))]);
    }
    cases.push(
      ["non-UTF-8 Origin with valid Referer", await responseSnapshot(await fetch(url, {
        headers: {
          origin: `${baseUrl}${String.fromCharCode(0x80)}`,
          referer: sameOriginReferer,
          cookie,
        },
      }))],
      ["repeated Origin fields", await httpGet(url, [
        "origin", baseUrl,
        "origin", "https://foreign.example",
        "cookie", cookie,
      ])],
      ["repeated Referer fields", await httpGet(url, [
        "referer", `${baseUrl}/one`,
        "referer", `${baseUrl}/two`,
        "cookie", cookie,
      ])],
    );

    for (const [name, response] of cases) {
      assert.equal(response.status, 403, name);
      assert.doesNotMatch(response.body, /Project A|Authoritative A|018f0000-0000-7001/, name);
    }
  } finally {
    await stopRealServer(server);
  }
});

test("invalid bootstrap User identity prevents the Server from becoming ready", async () => {
  await assert.rejects(
    startRealServer({ "session-invalid": "not-a-uuid" }),
    /StoryOS Server exited with 1/,
  );
});

test("missing authentication, cross-Project scope, and unknown Chapter identity use non-oracular errors", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    await assert.rejects(
      getProject({ baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl) }),
      (error) => protocolFailure(error, 401),
    );
    await assert.rejects(
      getProject({
        baseUrl, projectId: PROJECT_B, fetchImpl: browserFetch(baseUrl, "session-a"),
      }),
      (error) => sanitizedProtocolFailure(error, 404, /Project B|secret/),
    );
    const stale = await getChapter({
      baseUrl,
      projectId: PROJECT_A,
      chapterId: STALE_CHAPTER_A,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(stale.chapter.chapter_id, STALE_CHAPTER_A);
    assert.equal(stale.chapter.title, "Stale Chapter A");
    assert.equal(stale.chapter.current_revision.body, "Stale A");
    await assert.rejects(
      getChapter({
        baseUrl,
        projectId: PROJECT_A,
        chapterId: UNKNOWN_CHAPTER,
        fetchImpl: browserFetch(baseUrl, "session-a"),
      }),
      (error) => sanitizedProtocolFailure(error, 404, /Stale|body|ffff/),
    );
    const refusedOrigin = await fetch(new URL(`/api/v1/projects/${PROJECT_A}`, baseUrl), {
      headers: { origin: "https://foreign.example", cookie: "storyos_session=session-a" },
    });
    assert.equal(refusedOrigin.status, 403);
    assert.doesNotMatch(await refusedOrigin.text(), /Project A|Authoritative/);
  } finally {
    await stopRealServer(server);
  }
});
