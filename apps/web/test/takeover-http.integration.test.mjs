import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { once } from "node:events";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { promisify } from "node:util";

import {
  applyAuthorEdit, createEditorSession, createProjectCommandChallenge, digestApplyAuthorEdit,
  digestCreateEditorSession, digestTakeOverProjectWriter, getEditorSession, getSnapshot,
  takeOverProjectWriter,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32"
  ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const ISO_INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
const DIGEST_HEX = /^[0-9a-f]{64}$/;
const execFileAsync = promisify(execFile);

function browserFetch(baseUrl, sessionHandle) {
  return (url, options = {}) => fetch(url, {
    ...options,
    headers: {
      ...options.headers,
      origin: baseUrl,
      ...(sessionHandle ? { cookie: `storyos_session=${sessionHandle}` } : {}),
    },
  });
}

async function startRealServer() {
  return new Promise((resolve, reject) => {
    const server = spawn(serverBinary, ["--bind", "127.0.0.1:0"], {
      cwd: repositoryRoot,
      env: {
        ...process.env,
        STORYOS_DATABASE_URL: process.env.STORYOS_TEST_DATABASE_URL,
        STORYOS_BOOTSTRAP_SESSIONS: JSON.stringify({ "session-a": USER_A, "session-b": USER_B }),
        STORYOS_CHALLENGE_SECRET: "test-only-challenge-secret-that-is-at-least-thirty-two-bytes",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "", stderr = "";
    const fail = (error) => { clearTimeout(timeout); server.kill("SIGTERM"); reject(error); };
    const timeout = setTimeout(
      () => fail(new Error(`StoryOS Server did not become ready: ${stderr}`)), 5_000,
    );
    server.once("error", fail);
    server.once("exit", (code) => fail(new Error(`StoryOS Server exited with ${code}: ${stderr}`)));
    server.stderr.on("data", (chunk) => { stderr += chunk; });
    server.stdout.on("data", (chunk) => {
      stdout += chunk;
      const match = stdout.match(/^STORYOS_SERVER_URL=(http:\/\/[^\s]+)$/m);
      if (match) { clearTimeout(timeout); resolve({ baseUrl: match[1], server }); }
    });
  });
}

async function stopRealServer(server) {
  if (server.exitCode !== null) return;
  const exited = once(server, "exit");
  server.kill("SIGTERM");
  await exited;
}

async function queryPostgres(query) {
  const container = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
  assert.ok(container, "run through scripts/verify-project-scope.sh");
  const { stdout } = await execFileAsync("docker", [
    "exec", container, "psql", "-XAt", "-U", "postgres", "-c", query,
  ]);
  return stdout.trim();
}

async function withChallengeRetry(action) {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    try {
      return await action();
    } catch (error) {
      if (error.status !== 429 || attempt === 3) throw error;
      await new Promise((resolve) =>
        setTimeout(resolve, ((error.retryAfterSeconds ?? 1) + 1) * 1000));
    }
  }
  throw new Error("command challenge retry exhausted");
}

async function manuscriptAuthority() {
  return JSON.parse(await queryPostgres(`SELECT json_build_object(
    'author_edit_activities', (SELECT count(*) FROM storyos.project_activity_events
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'actions', (SELECT count(*) FROM storyos.author_action_entries
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'commits', (SELECT count(*) FROM storyos.authoritative_commits
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid),
    'apply_admissions', (SELECT count(*) FROM storyos.author_command_admissions
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
        AND command_kind = 'applyAuthorEdit'),
    'takeover_payloads', (SELECT count(*) FROM storyos.project_activity_event_payloads
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid)
  )::text`));
}

async function openEditorSession(baseUrl, correlationId, idempotencyKey) {
  const request = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: "storyos.web-client.release-1.v3",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: correlationId,
  };
  const digest = await digestCreateEditorSession(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/editor-sessions",
      command_schema: request.command_schema,
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  return createEditorSession({
    baseUrl, projectId: PROJECT_A, request, idempotencyKey,
    antiForgery: challenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
  });
}

async function loadCurrentWriter(baseUrl) {
  const existing = await queryPostgres(`SELECT current_editor_session_id::text
    FROM storyos.project_writer_generations
    WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
    ORDER BY writer_generation DESC LIMIT 1`);
  if (existing) {
    const session = await getEditorSession({
      baseUrl, projectId: PROJECT_A, editorSessionId: existing,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(session.writer.kind, "current_writer");
    return session;
  }
  return openEditorSession(
    baseUrl,
    "018f0000-0000-7001-8000-000000000338",
    "018f0000-0000-7001-8000-000000000339",
  );
}

test("an observer takeOverProjectWriter fences the prior writer and refuses its Author Edit", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const writer = await loadCurrentWriter(baseUrl);
    const observer = await openEditorSession(
      baseUrl,
      "018f0000-0000-7001-8000-000000000330",
      "018f0000-0000-7001-8000-000000000331",
    );
    assert.equal(observer.writer.kind, "read_only");
    assert.equal(observer.writer.reason, "secondary_session");
    const priorGeneration = writer.writer.writer_generation;
    const resultingGeneration = String(BigInt(priorGeneration) + 1n);
    const authorityBefore = await manuscriptAuthority();

    const takeoverRequest = {
      command_schema: "storyos.command.take-over-project-writer.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000332",
      editor_session_id: observer.editor_session.editor_session_id,
      observed_writer_generation: priorGeneration,
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
    };
    const takeoverKey = "018f0000-0000-7001-8000-000000000333";
    const takeoverDigest = await digestTakeOverProjectWriter(takeoverRequest);
    const takeoverChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
      request: {
        method: "POST",
        route_template:
          "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers",
        command_schema: takeoverRequest.command_schema,
        canonical_command_digest: takeoverDigest,
        idempotency_key: takeoverKey,
      },
    }));
    const takeoverOptions = {
      baseUrl, projectId: PROJECT_A,
      editorSessionId: observer.editor_session.editor_session_id,
      request: takeoverRequest, idempotencyKey: takeoverKey,
      antiForgery: takeoverChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    };
    const takeover = await takeOverProjectWriter(takeoverOptions);
    assert.deepEqual(await takeOverProjectWriter(takeoverOptions), takeover);

    assert.match(takeover.command_id, UUID);
    assert.match(takeover.author_command_admission_id, UUID);
    assert.match(takeover.receipt.receipt_id, UUID);
    assert.match(takeover.receipt.created_at, ISO_INSTANT);
    assert.match(takeover.receipt.command_digest.value_hex_lowercase, DIGEST_HEX);
    assert.match(takeover.result.resulting_snapshot_id, UUID);
    assert.equal(takeover.result.resulting_heads.length, 1);
    assert.match(takeover.result.resulting_heads[0], UUID);
    const heads = takeover.result.resulting_heads;
    assert.deepEqual(takeover, {
      schema_id: "storyos.command.take-over-project-writer.response.v1",
      correlation_id: takeoverRequest.correlation_id,
      project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
      command_id: takeover.command_id,
      author_command_admission_id: takeover.author_command_admission_id,
      receipt: {
        receipt_id: takeover.receipt.receipt_id,
        project_scope: { owner_user_id: USER_A, project_id: PROJECT_A },
        command_kind: "takeOverProjectWriter",
        command_digest: {
          algorithm: "sha256",
          profile: "storyos.command.takeOverProjectWriter.jcs.v1",
          value_hex_lowercase: takeover.receipt.command_digest.value_hex_lowercase,
        },
        idempotency_key: takeoverKey,
        producer_cause: "author_command_admission",
        author_command_admission_id: takeover.author_command_admission_id,
        expected_heads: heads,
        prior_heads: heads,
        resulting_heads: heads,
        authoritative_revision_ids: [],
        proposal_revision_ids: [],
        authoritative_commit_ids: [],
        author_action_sequence: null,
        draft_artifact_refs: [],
        artifact_lifecycle_event_refs: [],
        condition_refs: [],
        result: "no_effect",
        created_at: takeover.receipt.created_at,
      },
      result: {
        kind: "takeover_applied",
        prior_editor_session_id: writer.editor_session.editor_session_id,
        prior_writer_generation: priorGeneration,
        resulting_editor_session_id: observer.editor_session.editor_session_id,
        resulting_writer_generation: resultingGeneration,
        resulting_snapshot_id: takeover.result.resulting_snapshot_id,
        resulting_snapshot_activity_position:
          takeover.result.resulting_snapshot_activity_position,
        resulting_heads: heads,
      },
    });
    assert.match(takeover.result.resulting_snapshot_activity_position, /^[1-9][0-9]*$/);

    const snapshot = await getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId: takeover.result.resulting_snapshot_id,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(snapshot.snapshot.snapshot_id, takeover.result.resulting_snapshot_id);
    assert.ok(
      BigInt(snapshot.snapshot.project_activity_position)
        >= BigInt(takeover.result.resulting_snapshot_activity_position),
    );

    const winner = await getEditorSession({
      baseUrl, projectId: PROJECT_A,
      editorSessionId: observer.editor_session.editor_session_id,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.deepEqual(winner.writer, {
      kind: "current_writer", writer_generation: resultingGeneration,
    });
    const fenced = await getEditorSession({
      baseUrl, projectId: PROJECT_A,
      editorSessionId: writer.editor_session.editor_session_id,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.deepEqual(fenced.writer, {
      kind: "read_only",
      observed_writer_generation: resultingGeneration,
      reason: "superseded_by_takeover",
    });

    const authorityAfterTakeover = await manuscriptAuthority();
    assert.deepEqual({
      ...authorityAfterTakeover,
      takeover_payloads: authorityBefore.takeover_payloads,
    }, authorityBefore);
    assert.equal(authorityAfterTakeover.takeover_payloads, authorityBefore.takeover_payloads + 1);

    const staleRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000334",
      editor_session_id: writer.editor_session.editor_session_id,
      writer_generation: priorGeneration,
      chapter_id: writer.base_snapshot.chapter_id,
      expected_authoritative_revision_id:
        writer.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids: writer.base_snapshot.proposal_head_revision_ids,
      target_refs: writer.base_snapshot.target_refs,
      observed_ownership_partition: writer.base_snapshot.observed_ownership_partition,
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
      undo_group_id: "018f0000-0000-7001-8000-000000000335",
      completed_intent_record_id: "018f0000-0000-7001-8000-000000000336",
      local_intent_sequence: "99",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from: 0, to: 0, text: "z" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 0,
        },
      }],
    };
    const staleKey = "018f0000-0000-7001-8000-000000000337";
    const staleDigest = await digestApplyAuthorEdit(staleRequest);
    const staleChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
        command_schema: staleRequest.command_schema,
        canonical_command_digest: staleDigest,
        idempotency_key: staleKey,
      },
    }));
    await assert.rejects(applyAuthorEdit({
      baseUrl, projectId: PROJECT_A, request: staleRequest, idempotencyKey: staleKey,
      antiForgery: staleChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => error.status === 412
      && JSON.parse(error.responseBody).code === "editor_writer_stale");
    assert.deepEqual(await manuscriptAuthority(), authorityAfterTakeover);
  } finally {
    await stopRealServer(server);
  }
});
