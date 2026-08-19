import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { once } from "node:events";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { promisify } from "node:util";

import {
  activityStream, createEditorSession, createProjectCommandChallenge, digestCreateEditorSession,
  getEditorSession, getSnapshot,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32"
  ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const UNKNOWN_SNAPSHOT = "018f0000-0000-7001-8000-000000000250";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const ISO_INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/;
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

function problemCode(error) {
  return JSON.parse(error.responseBody).code;
}

async function ensureCurrentWriter(baseUrl) {
  const existing = await queryPostgres(`SELECT current_editor_session_id::text
    FROM storyos.project_writer_generations
    WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid`);
  if (existing) {
    const session = await getEditorSession({
      baseUrl, projectId: PROJECT_A, editorSessionId: existing,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(session.writer.kind, "current_writer");
    return session;
  }
  const request = {
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: "storyos.web-client.release-1.v3",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000201",
  };
  const digest = await digestCreateEditorSession(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/editor-sessions",
      command_schema: request.command_schema,
      canonical_command_digest: digest,
      idempotency_key: "018f0000-0000-7001-8000-000000000202",
    },
  }));
  const session = await createEditorSession({
    baseUrl, projectId: PROJECT_A, request,
    idempotencyKey: "018f0000-0000-7001-8000-000000000202",
    antiForgery: challenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
  });
  assert.equal(session.writer.kind, "current_writer");
  return session;
}

test("getSnapshot and activityStream use the Editor Session Snapshot identity at the public HTTP boundary", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const session = await ensureCurrentWriter(baseUrl);
    const snapshotId = session.base_snapshot.snapshot_id;
    assert.match(snapshotId, UUID);
    const snapshot = await getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(snapshot.schema_id, "storyos.query.snapshot.response.v1");
    assert.match(snapshot.correlation_id, UUID);
    assert.deepEqual(snapshot.project_scope, {
      owner_user_id: USER_A, project_id: PROJECT_A,
    });
    assert.equal(snapshot.snapshot.snapshot_id, snapshotId);
    assert.deepEqual(snapshot.snapshot.project_scope, snapshot.project_scope);
    assert.equal(snapshot.snapshot.snapshot_kind, "canonical");
    assert.equal(snapshot.snapshot.project_activity_position, session.base_snapshot.project_activity_position);
    assert.deepEqual(snapshot.snapshot.source_watermarks, {});
    assert.deepEqual(snapshot.snapshot.projection_generations, {});
    assert.equal(snapshot.snapshot.redaction_profile, "storyos.author.v1");
    assert.equal(snapshot.snapshot.schema_profile, "storyos.public.release.1");
    assert.equal(snapshot.snapshot.replay_generation, "1");
    assert.match(snapshot.snapshot.created_at, ISO_INSTANT);
    assert.equal(snapshot.snapshot.expires_at, null);

    const streamUrl = new URL(
      `/api/v1/projects/${PROJECT_A}/activity?snapshot_id=${encodeURIComponent(snapshotId)}&protocol_release=${encodeURIComponent("storyos.public.release.1")}`,
      baseUrl,
    );
    const stream = await fetch(streamUrl, {
      headers: { origin: baseUrl, cookie: "storyos_session=session-a", accept: "text/event-stream" },
    });
    assert.equal(stream.status, 200);
    assert.match(stream.headers.get("content-type") ?? "", /^text\/event-stream\b/);
    assert.equal(stream.headers.get("cache-control"), "no-store");
    assert.equal(await stream.text(), "");
    assert.equal(await activityStream({
      baseUrl, projectId: PROJECT_A, snapshotId, protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), "");

    const refererOnly = await fetch(
      new URL(`/api/v1/projects/${PROJECT_A}/snapshots/${snapshotId}`, baseUrl),
      { headers: { referer: `${baseUrl}/projects/${PROJECT_A}`, cookie: "storyos_session=session-a" } },
    );
    assert.equal(refererOnly.status, 200);
    const foreignOrigin = await fetch(
      new URL(`/api/v1/projects/${PROJECT_A}/snapshots/${snapshotId}`, baseUrl),
      { headers: { origin: "https://foreign.example", cookie: "storyos_session=session-a" } },
    );
    assert.equal(foreignOrigin.status, 403);
    assert.doesNotMatch(await foreignOrigin.text(), /Project A|Authoritative A/);

    await queryPostgres(`
      UPDATE storyos.replay_floors
         SET floor_position = ${Number(snapshot.snapshot.project_activity_position) + 1}
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
         AND replay_generation = 1`);
    await assert.rejects(activityStream({
      baseUrl, projectId: PROJECT_A, snapshotId, protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => error.status === 409 && problemCode(error) === "activity_cursor_too_old"
      && !/Project A|Authoritative A/.test(error.responseBody));
    await queryPostgres(`
      UPDATE storyos.replay_floors
         SET floor_position = 0
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
         AND replay_generation = 1`);
    assert.equal(await activityStream({
      baseUrl, projectId: PROJECT_A, snapshotId, protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), "");

    await queryPostgres(`
      INSERT INTO storyos.replay_generations (owner_user_id, project_id, replay_generation)
      VALUES ('${USER_A}'::uuid, '${PROJECT_A}'::uuid, 2)
      ON CONFLICT DO NOTHING;
      INSERT INTO storyos.replay_floors
        (owner_user_id, project_id, replay_generation, floor_position)
      VALUES ('${USER_A}'::uuid, '${PROJECT_A}'::uuid, 2, 0)
      ON CONFLICT DO NOTHING`);
    await assert.rejects(activityStream({
      baseUrl, projectId: PROJECT_A, snapshotId, protocolRelease: "storyos.public.release.1",
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => error.status === 409 && problemCode(error) === "activity_cursor_too_old"
      && !/Project A|Authoritative A/.test(error.responseBody));
    const afterFloor = await getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(afterFloor.snapshot.snapshot_id, snapshotId);
    assert.equal(afterFloor.snapshot.replay_generation, "1");

    await assert.rejects(getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId: UNKNOWN_SNAPSHOT,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => error.status === 404 && problemCode(error) === "resource_unavailable"
      && !/Project A|Authoritative A/.test(error.responseBody));
    await assert.rejects(getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId,
      fetchImpl: browserFetch(baseUrl, "session-b"),
    }), (error) => error.status === 404 && problemCode(error) === "resource_unavailable"
      && !/Project A|Authoritative A/.test(error.responseBody));

    const disallowed = await fetch(new URL(`/api/v1/projects/${PROJECT_A}/snapshots/${snapshotId}`, baseUrl), {
      method: "POST",
      headers: { origin: baseUrl, cookie: "storyos_session=session-a", "content-type": "application/json" },
      body: "{}",
    });
    assert.equal(disallowed.status, 405);
    assert.equal(JSON.parse(await disallowed.text()).code, "method_not_allowed");

    await queryPostgres(`
      UPDATE storyos.project_snapshots
         SET expires_at = clock_timestamp() - interval '1 second'
       WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${PROJECT_A}'::uuid
         AND snapshot_id = '${snapshotId}'::uuid`);
    await assert.rejects(getSnapshot({
      baseUrl, projectId: PROJECT_A, snapshotId,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => error.status === 409 && problemCode(error) === "snapshot_expired"
      && !/Project A|Authoritative A/.test(error.responseBody));
  } finally {
    await stopRealServer(server);
  }
});
