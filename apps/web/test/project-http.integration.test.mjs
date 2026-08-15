import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { request as httpRequest } from "node:http";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  applyAuthorEdit, createEditorSession, createProjectCommandChallenge, digestApplyAuthorEdit,
  digestCreateEditorSession, getChapter, getEditorSession, getProject,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { runStoryOSWeb } from "../src/app.mjs";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const CHAPTER_A = "018f0000-0000-7001-8000-000000000003";
const STALE_CHAPTER_A = "018f0000-0000-7001-8000-000000000006";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B = "018f0000-0000-7001-8000-000000000102";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

class TestNode {
  constructor(tag = "div") { this.tag = tag; this.children = []; this.dataset = {}; this.attributes = {}; this.ownText = ""; }
  set textContent(value) { this.ownText = value; this.children = []; }
  get textContent() { return this.ownText + this.children.map((child) => child.textContent).join(""); }
  append(...children) { this.children.push(...children); }
  replaceChildren(...children) { this.ownText = ""; this.children = children; }
  setAttribute(name, value) { this.attributes[name] = value; }
}

function testDocument() {
  const root = new TestNode("main");
  return {
    root,
    documentElement: { dataset: {} },
    querySelector: (selector) => selector === "#app" ? root : null,
    createElement: (tag) => new TestNode(tag),
  };
}

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

async function startRealServer(sessionUsers = { "session-a": USER_A, "session-b": USER_B }) {
  return new Promise((resolve, reject) => {
    const server = spawn(serverBinary, ["--bind", "127.0.0.1:0"], {
      cwd: repositoryRoot,
      env: {
        ...process.env,
        STORYOS_DATABASE_URL: process.env.STORYOS_TEST_DATABASE_URL,
        STORYOS_BOOTSTRAP_SESSIONS: JSON.stringify(sessionUsers),
        STORYOS_CHALLENGE_SECRET: "test-only-challenge-secret-that-is-at-least-thirty-two-bytes",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "", stderr = "";
    const fail = (error) => { clearTimeout(timeout); server.kill("SIGTERM"); reject(error); };
    const timeout = setTimeout(() => fail(new Error(`StoryOS Server did not become ready: ${stderr}`)), 5_000);
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

async function responseSnapshot(response) {
  return { status: response.status, body: await response.text() };
}

function httpGet(url, headers) {
  return new Promise((resolve, reject) => {
    const request = httpRequest(url, { headers }, (response) => {
      response.setEncoding("utf8");
      let body = "";
      response.on("data", (chunk) => { body += chunk; });
      response.on("end", () => resolve({ status: response.statusCode, body }));
    });
    request.on("error", reject);
    request.end();
  });
}

test("the real Web entry opens the URL-selected Project with its same-origin session cookie", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const documentImpl = testDocument();
    documentImpl.documentElement.dataset.storyosProject = PROJECT_B;
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const state = await runStoryOSWeb({
      documentImpl,
      locationImpl: { origin: baseUrl, pathname: `/projects/${PROJECT_A}` },
      fetchImpl,
    });

    assert.equal(state.kind, "project-ready");
    assert.equal(documentImpl.root.dataset.bootState, "project-ready");
    assert.equal(documentImpl.root.children[0]?.tag, "section");
    assert.match(documentImpl.root.textContent, /Project A/);
    assert.match(documentImpl.root.textContent, /Chapter A/);
    assert.match(documentImpl.root.textContent, /Authoritative A/);
    assert.match(documentImpl.root.textContent, /018f0000-0000-7001-8000-000000000005/);
  } finally {
    await stopRealServer(server);
  }
});

test("a missing Web root fails closed before any Project request", async () => {
  const documentImpl = testDocument();
  documentImpl.querySelector = () => null;
  let requestCount = 0;

  await assert.rejects(runStoryOSWeb({
    documentImpl,
    locationImpl: { origin: "http://storyos.test", pathname: `/projects/${PROJECT_A}` },
    fetchImpl: async () => { requestCount += 1; throw new Error("must not request"); },
  }), /StoryOS Web cannot start because the required #app root is missing/);
  assert.equal(requestCount, 0);
});

test("a missing or invalid Project URL fails closed before any request", async () => {
  for (const pathname of ["/", "/projects/not-a-uuid", `/projects/${PROJECT_A}/extra`]) {
    const documentImpl = testDocument();
    documentImpl.documentElement.dataset.storyosProject = PROJECT_A;
    documentImpl.root.textContent = "stale Project page";
    let requestCount = 0;
    const state = await runStoryOSWeb({
      documentImpl,
      locationImpl: { origin: "http://storyos.test", pathname },
      fetchImpl: async () => { requestCount += 1; throw new Error("must not request"); },
    });

    assert.deepEqual(state, {
      kind: "project-blocked",
      code: "project_url_invalid",
      heading: "StoryOS 无法打开项目",
      message: "项目地址缺少有效的受控项目身份。",
    }, pathname);
    assert.equal(requestCount, 0, pathname);
    assert.equal(documentImpl.root.dataset.bootState, "project-blocked", pathname);
    assert.match(documentImpl.root.textContent, /StoryOS 无法打开项目/, pathname);
    assert.match(documentImpl.root.textContent, /项目地址缺少有效的受控项目身份。/, pathname);
    assert.doesNotMatch(documentImpl.root.textContent, /stale Project page/, pathname);
  }
});

test("two authenticated Users open only their own Project and current Chapter over real HTTP", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const projectA = await getProject({
      baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    assert.equal(projectA.project_scope.owner_user_id, USER_A);
    assert.equal(projectA.project.current_chapter_id, CHAPTER_A);
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
    const chapterB = await getChapter({
      baseUrl, projectId: PROJECT_B, chapterId: projectB.project.current_chapter_id,
      fetchImpl: browserFetch(baseUrl, "session-b"),
    });
    assert.equal(chapterB.chapter.current_revision.body, "Authoritative B secret");

    await assert.rejects(
      getProject({
        baseUrl, projectId: PROJECT_B, fetchImpl: browserFetch(baseUrl, "session-a"),
      }),
      (error) => error.status === 404 && !/Project B|secret/.test(error.responseBody),
    );
  } finally {
    await stopRealServer(server);
  }
});

test("Project command challenges bind Origin, Scope, nonce, and idempotency on real PostgreSQL", async () => {
  const { baseUrl, server } = await startRealServer();
  const request = {
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
      (error) => error.status === 409,
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
      (error) => error.status === 400,
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
      (error) => error.status === 404 && !/Project B|secret/.test(error.responseBody),
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
  const { baseUrl, server } = await startRealServer();
  const requests = [20, 21].map((suffix) => ({
    command_schema: "storyos.command.create-editor-session.request.v1",
    client_contract_revision: "storyos.web-client.release-1.v1",
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: `018f0000-0000-7001-8000-${String(suffix).padStart(12, "0")}`,
  }));
  const keys = [22, 23].map((suffix) =>
    `018f0000-0000-7001-8000-${String(suffix).padStart(12, "0")}`);
  try {
    assert.deepEqual(await digestCreateEditorSession(requests[0]), await digestCreateEditorSession({
      correlation_id: requests[0].correlation_id,
      security_policy_revision: requests[0].security_policy_revision,
      command_schema: requests[0].command_schema,
      client_contract_revision: requests[0].client_contract_revision,
    }));
    const challenges = await Promise.all(requests.map(async (request, index) =>
      createProjectCommandChallenge({
        baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl, "session-a"),
        request: {
          method: "POST",
          route_template: "/api/v1/projects/{project_id}/editor-sessions",
          command_schema: request.command_schema,
          canonical_command_digest: await digestCreateEditorSession(request),
          idempotency_key: keys[index],
        },
      })));
    const sessionUrl = new URL(`/api/v1/projects/${PROJECT_A}/editor-sessions`, baseUrl);
    const refererOnly = await fetch(sessionUrl, {
      method: "POST",
      headers: { referer: `${baseUrl}/projects/${PROJECT_A}`, cookie: "storyos_session=session-a",
        "content-type": "application/json", "idempotency-key": keys[0],
        "x-storyos-anti-forgery": challenges[0].nonce },
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
      antiForgery: challenges[0].nonce, fetchImpl: browserFetch(baseUrl, "session-b"),
    }), (error) => error.status === 422 && !/Project A|Authoritative A/.test(error.responseBody));
    const sessions = await Promise.all(requests.map((request, index) => createEditorSession({
      baseUrl, projectId: PROJECT_A, request, idempotencyKey: keys[index],
      antiForgery: challenges[index].nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    })));
    assert.equal(sessions.filter((session) => session.writer.kind === "current_writer").length, 1);
    assert.equal(sessions.filter((session) => session.writer.kind === "read_only").length, 1);
    assert.deepEqual(new Set(sessions.map((session) => session.writer.writer_generation
      ?? session.writer.observed_writer_generation)), new Set(["1"]));
    const currentIndex = sessions.findIndex((session) => session.writer.kind === "current_writer");
    assert.deepEqual(await createEditorSession({
      baseUrl, projectId: PROJECT_A, request: requests[currentIndex],
      idempotencyKey: keys[currentIndex], antiForgery: challenges[currentIndex].nonce,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), sessions[currentIndex]);
    const read = await getEditorSession({
      baseUrl, projectId: PROJECT_A,
      editorSessionId: sessions[currentIndex].editor_session.editor_session_id,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    });
    const { schema_id: createSchema, correlation_id: createCorrelation, ...created } = sessions[currentIndex];
    const { schema_id: readSchema, correlation_id: readCorrelation, ...readPayload } = read;
    assert.deepEqual(readPayload, created);
    assert.equal(createSchema, "storyos.command.create-editor-session.response.v1");
    assert.equal(readSchema, "storyos.query.editor-session.response.v1");
    assert.match(createCorrelation, UUID);
    assert.match(readCorrelation, UUID);
    await assert.rejects(getEditorSession({
      baseUrl, projectId: PROJECT_A,
      editorSessionId: sessions[currentIndex].editor_session.editor_session_id,
      fetchImpl: browserFetch(baseUrl, "session-b"),
    }), (error) => error.status === 404 && !/Project A|Authoritative A/.test(error.responseBody));
    await assert.rejects(createEditorSession({
      baseUrl, projectId: PROJECT_A,
      request: { ...requests[currentIndex], correlation_id: "018f0000-0000-7001-8000-000000000099" },
      idempotencyKey: keys[currentIndex], antiForgery: challenges[currentIndex].nonce,
      fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => error.status === 422);

    const currentSession = sessions[currentIndex];
    const authorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      client_contract_revision: "storyos.web-client.release-1.v1",
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
      editor_contract_revision: "storyos.editor-contract.release-1.v1",
      undo_group_id: "018f0000-0000-7001-8000-000000000025",
      completed_intent_record_id: "018f0000-0000-7001-8000-000000000026",
      local_intent_sequence: "1",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from: 15, to: 15, text: "!" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 15, to: 15,
        },
      }],
    };
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
    const settlement = await applyAuthorEdit(authorEditOptions);
    assert.equal(settlement.receipt.result, "authoritative_applied");
    assert.equal(settlement.effect.kind, "authoritative_applied");
    assert.equal(settlement.effect.authoritative_revision.body, "Authoritative A!");
    assert.equal(settlement.completed_intent_record_id,
      authorEditRequest.completed_intent_record_id);
    assert.deepEqual(await applyAuthorEdit(authorEditOptions), settlement);

    const outcomeRequests = [
      {
        expected: "conflicted",
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
      const result = await applyAuthorEdit({
        baseUrl, projectId: PROJECT_A, request: outcome.request, idempotencyKey,
        antiForgery: challenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
      });
      assert.equal(result.receipt.result, outcome.expected);
      assert.equal(result.effect.kind, outcome.expected);
      assert.deepEqual(result.receipt.authoritative_revision_ids, []);
      assert.deepEqual(result.receipt.authoritative_commit_ids, []);
      assert.equal(result.receipt.author_action_sequence, null);
      assert.match(result.effect.project_activity_position, /^[1-9][0-9]*$/);
    }

    const staleSession = sessions.find((session) => session.writer.kind === "read_only");
    const staleRequest = {
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
    }), (error) => error.status === 422
      && JSON.parse(error.responseBody).code === "challenge_invalid");
    await assert.rejects(applyAuthorEdit({
      baseUrl, projectId: PROJECT_A, request: staleRequest, idempotencyKey: staleKey,
      antiForgery: staleChallenge.nonce, fetchImpl: browserFetch(baseUrl, "session-a"),
    }), (error) => error.status === 412
      && JSON.parse(error.responseBody).code === "editor_writer_stale");
  } finally {
    await stopRealServer(server);
  }
});

test("the real challenge endpoint returns a sanitized rate refusal with retry time", async () => {
  const { baseUrl, server } = await startRealServer();
  const request = {
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
      assert.equal(error.status, 429);
      assert.match(String(error.retryAfterSeconds), /^(?:[1-9]|[1-5][0-9]|60)$/);
      assert.deepEqual(JSON.parse(error.responseBody), {
        schema_id: "storyos.problem.v1",
        code: "challenge_rate_limited",
        message: "The command challenge rate limit is exceeded.",
      });
      assert.doesNotMatch(error.responseBody, new RegExp(`${USER_B}|${PROJECT_B}|nonce|sha256`));
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
    const hostileHeaders = [
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
    const cases = [];
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
      ["repeated Origin fields", await httpGet(url, {
        origin: [baseUrl, "https://foreign.example"], cookie,
      })],
      ["repeated Referer fields", await httpGet(url, {
        referer: [`${baseUrl}/one`, `${baseUrl}/two`], cookie,
      })],
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

test("missing authentication, cross-Project scope, and stale Chapter identity use non-oracular errors", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    await assert.rejects(
      getProject({ baseUrl, projectId: PROJECT_A, fetchImpl: browserFetch(baseUrl) }),
      (error) => error.status === 401,
    );
    await assert.rejects(
      getProject({
        baseUrl, projectId: PROJECT_B, fetchImpl: browserFetch(baseUrl, "session-a"),
      }),
      (error) => error.status === 404 && !/Project B|secret/.test(error.responseBody),
    );
    await assert.rejects(
      getChapter({
        baseUrl,
        projectId: PROJECT_A,
        chapterId: STALE_CHAPTER_A,
        fetchImpl: browserFetch(baseUrl, "session-a"),
      }),
      (error) => error.status === 404 && !/Stale|body/.test(error.responseBody),
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
