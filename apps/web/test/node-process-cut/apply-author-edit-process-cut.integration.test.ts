import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";
import { promisify } from "node:util";

import {
  applyAuthorEdit, createProjectCommandChallenge, digestApplyAuthorEdit,
  getApplyAuthorEditOutcome, getEditorSession,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  GetEditorSessionResponse,
  StoryOSQueryOptions,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32"
  ? "storyos-server.exe" : "storyos-server");
const USER = "018f0000-0000-7001-8000-000000000001";
const PROJECT = "018f0000-0000-7001-8000-000000000002";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const SESSION_HANDLE = "session-a";
const SESSIONS = { [SESSION_HANDLE]: USER };
const execFileAsync = promisify(execFile);

function trackingFetch(baseUrl: string, actions: string[]): typeof fetch {
  const inner = browserFetch(baseUrl, SESSION_HANDLE);
  return async (url, options) => {
    actions.push(`${options?.method ?? "GET"} ${new URL(
      url instanceof Request ? url.url : url,
    ).pathname}`);
    const nonce = new Headers(options?.headers).get("x-storyos-anti-forgery") ?? "\u0000";
    if (String(url).includes(nonce)) {
      throw new Error("nonce entered the URL");
    }
    return inner(url, options);
  };
}

async function startRealServer(bind = "127.0.0.1:0") {
  return startStoryOSServer({
    bind,
    repositoryRoot,
    serverBinary,
    sessions: SESSIONS,
  });
}

async function restartServer(server: ChildProcess, baseUrl: string) {
  const url = new URL(baseUrl);
  await stopRealServer(server);
  return startRealServer(`${url.hostname}:${url.port}`);
}

async function projectAuthoritySnapshot() {
  const query = `
    SELECT json_build_object(
      'receipts', (SELECT count(*) FROM storyos.domain_receipts
        WHERE owner_user_id = '${USER}'::uuid AND project_id = '${PROJECT}'::uuid),
      'admissions', (SELECT count(*) FROM storyos.author_command_admissions
        WHERE owner_user_id = '${USER}'::uuid AND project_id = '${PROJECT}'::uuid
          AND command_kind = 'applyAuthorEdit'),
      'consumed_challenges', (SELECT count(*) FROM storyos.project_command_challenges
        WHERE owner_user_id = '${USER}'::uuid AND project_id = '${PROJECT}'::uuid
          AND command_kind = 'applyAuthorEdit' AND consumed_at IS NOT NULL),
      'activities', (SELECT count(*) FROM storyos.project_activity_events
        WHERE owner_user_id = '${USER}'::uuid AND project_id = '${PROJECT}'::uuid)
    )::text`;
  return JSON.parse(await queryPostgres(query));
}

async function openCurrentWriter(baseUrl: string): Promise<GetEditorSessionResponse> {
  const editorSessionId = await queryPostgres(`SELECT current_editor_session_id::text
    FROM storyos.project_writer_generations
    WHERE owner_user_id = '${USER}'::uuid AND project_id = '${PROJECT}'::uuid`);
  assert.match(editorSessionId, UUID);
  const session = await getEditorSession({
    baseUrl, projectId: PROJECT, editorSessionId,
    fetchImpl: browserFetch(baseUrl, SESSION_HANDLE),
  });
  assert.equal(session.writer.kind, "current_writer");
  if (session.writer.kind !== "current_writer") {
    throw new Error("the fixture did not expose the current writer");
  }
  return session;
}

function authorEditRequest(session: GetEditorSessionResponse): ApplyAuthorEditRequest {
  if (session.writer.kind !== "current_writer") {
    throw new Error("the Author Edit requires the current writer");
  }
  const body = session.base_snapshot.materialized_revision.body;
  const from = body.length;
  return {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    client_contract_revision: "storyos.web-client.release-1.v3",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: "018f0000-0000-7001-8000-000000002100",
    editor_session_id: session.editor_session.editor_session_id,
    writer_generation: session.writer.writer_generation,
    chapter_id: session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: session.base_snapshot.proposal_head_revision_ids,
    target_refs: session.base_snapshot.target_refs,
    observed_ownership_partition: session.base_snapshot.observed_ownership_partition,
    editor_contract_revision: "storyos.editor-contract.release-1.v2",
    undo_group_id: "018f0000-0000-7001-8000-000000002101",
    completed_intent_record_id: "018f0000-0000-7001-8000-000000002102",
    local_intent_sequence: "9",
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from, to: from, text: "!" }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from,
      },
    }],
  };
}

async function issueAuthorEditChallenge(
  baseUrl: string,
  request: ApplyAuthorEditRequest,
  idempotencyKey: string,
) {
  return withChallengeRetry(async () => createProjectCommandChallenge({
    baseUrl, projectId: PROJECT, fetchImpl: browserFetch(baseUrl, SESSION_HANDLE),
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
      command_schema: request.command_schema,
      canonical_command_digest: await digestApplyAuthorEdit(request),
      idempotency_key: idempotencyKey,
    },
  }));
}

function outcomeOptions(
  baseUrl: string,
  idempotencyKey: string,
  nonce: string,
  fetchImpl: typeof fetch = browserFetch(baseUrl, SESSION_HANDLE),
): StoryOSQueryOptions & { projectId: string; idempotencyKey: string; antiForgery: string } {
  return {
    baseUrl, projectId: PROJECT, idempotencyKey, antiForgery: nonce,
    fetchImpl,
  };
}

function assertFirstActionIsOutcomeGet(actions: string[], idempotencyKey: string): void {
  assert.match(actions[0] ?? "", new RegExp(
    `^GET /api/v1/projects/${PROJECT}/manuscript/author-edit-outcomes/${idempotencyKey}$`,
  ));
  assert.equal(actions.filter((action) => action.startsWith("POST ")).length, 0);
}

async function pausePostgres() {
  const container = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
  assert.ok(container);
  await execFileAsync("docker", ["pause", container]);
}

async function unpausePostgres() {
  const container = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
  assert.ok(container);
  await execFileAsync("docker", ["unpause", container]);
  await execFileAsync("docker", ["exec", container, "pg_isready", "-U", "postgres"]);
}

test("Server restart and bounded PostgreSQL interruption keep GET-first ApplyAuthorEdit recovery",
  async () => {
    assert.ok(process.env.STORYOS_TEST_DATABASE_URL, "run through scripts/verify-project-scope.sh");
    assert.ok(
      process.env.STORYOS_TEST_POSTGRES_CONTAINER, "run through scripts/verify-project-scope.sh",
    );
    let { baseUrl, server } = await startRealServer();
    let postgresPaused = false;
    try {
      const session = await openCurrentWriter(baseUrl);
      const request = authorEditRequest(session);
      const idempotencyKey = "018f0000-0000-7001-8000-000000002103";
      const challenge = await issueAuthorEditChallenge(baseUrl, request, idempotencyKey);
      const authorityBefore = await projectAuthoritySnapshot();
      let actions: string[] = [];
      ({ baseUrl, server } = await restartServer(server, baseUrl));
      const unknownAfterRestart = await getApplyAuthorEditOutcome(
        outcomeOptions(baseUrl, idempotencyKey, challenge.nonce, trackingFetch(baseUrl, actions)),
      );
      assertFirstActionIsOutcomeGet(actions, idempotencyKey);
      assert.match(unknownAfterRestart.correlation_id, UUID);
      assert.deepEqual({
        schema_id: unknownAfterRestart.schema_id,
        project_scope: unknownAfterRestart.project_scope,
        outcome: unknownAfterRestart.outcome,
      }, {
        schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
        project_scope: { owner_user_id: USER, project_id: PROJECT },
        outcome: {
          outcome_kind: "still_unknown",
          observation: {
            observation_kind: "challenge_issued",
            expires_at: challenge.expires_at,
          },
        },
      });
      assert.deepEqual(await projectAuthoritySnapshot(), authorityBefore);

      await assert.rejects(applyAuthorEdit({
        baseUrl, projectId: PROJECT, request, idempotencyKey, antiForgery: challenge.nonce,
        fetchImpl: async (url, options) => {
          const response = await browserFetch(baseUrl, SESSION_HANDLE)(url, options);
          await response.arrayBuffer();
          throw new Error("simulated acknowledgement delivery loss");
        },
      }), /simulated acknowledgement delivery loss/);
      const committedBeforeCut = await getApplyAuthorEditOutcome(
        outcomeOptions(baseUrl, idempotencyKey, challenge.nonce),
      );
      if (committedBeforeCut.outcome.outcome_kind !== "committed") {
        throw new Error("the committed Author Edit outcome was unavailable");
      }
      if (committedBeforeCut.outcome.response.effect.kind !== "authoritative_applied") {
        throw new Error("the committed Author Edit was not authoritative");
      }
      assert.equal(
        committedBeforeCut.outcome.response.effect.authoritative_revision.body,
        `${session.base_snapshot.materialized_revision.body}!`,
      );
      const authorityAfterCommit = await projectAuthoritySnapshot();
      assert.equal(authorityAfterCommit.receipts, authorityBefore.receipts + 1);
      assert.equal(authorityAfterCommit.admissions, authorityBefore.admissions + 1);
      assert.equal(authorityAfterCommit.activities, authorityBefore.activities + 1);

      actions = [];
      ({ baseUrl, server } = await restartServer(server, baseUrl));
      const committedAfterRestart = await getApplyAuthorEditOutcome(
        outcomeOptions(baseUrl, idempotencyKey, challenge.nonce, trackingFetch(baseUrl, actions)),
      );
      assertFirstActionIsOutcomeGet(actions, idempotencyKey);
      assert.deepEqual(committedAfterRestart.outcome, committedBeforeCut.outcome);
      assert.deepEqual(await projectAuthoritySnapshot(), authorityAfterCommit);

      await pausePostgres();
      postgresPaused = true;
      let fabricatedCommit = false;
      try {
        const interrupted = await getApplyAuthorEditOutcome({
          ...outcomeOptions(baseUrl, idempotencyKey, challenge.nonce),
          fetchImpl: (url, options) => fetch(url, {
            ...options,
            headers: {
              ...options?.headers,
              origin: baseUrl,
              cookie: `storyos_session=${SESSION_HANDLE}`,
            },
            signal: AbortSignal.timeout(2_000),
          }),
        });
        fabricatedCommit = interrupted.outcome?.outcome_kind === "committed";
      } catch {
        fabricatedCommit = false;
      }
      assert.equal(fabricatedCommit, false);
      await unpausePostgres();
      postgresPaused = false;
      actions = [];
      const recovered = await getApplyAuthorEditOutcome(
        outcomeOptions(baseUrl, idempotencyKey, challenge.nonce, trackingFetch(baseUrl, actions)),
      );
      assertFirstActionIsOutcomeGet(actions, idempotencyKey);
      assert.deepEqual(recovered.outcome, committedBeforeCut.outcome);
      assert.deepEqual(await projectAuthoritySnapshot(), authorityAfterCommit);
    } finally {
      if (postgresPaused) await unpausePostgres().catch(() => {});
      await stopRealServer(server);
    }
  });
