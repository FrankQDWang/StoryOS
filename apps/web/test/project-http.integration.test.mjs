import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { getChapter, getProject } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const PROJECT_A = "018f0000-0000-7001-8000-000000000002";
const CHAPTER_A = "018f0000-0000-7001-8000-000000000003";
const STALE_CHAPTER_A = "018f0000-0000-7001-8000-000000000006";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const PROJECT_B = "018f0000-0000-7001-8000-000000000102";

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
