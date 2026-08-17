import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const chromeExecutable = [
  process.env.CHROME_BIN,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome", "/usr/bin/chromium", "/usr/bin/chromium-browser",
].filter(Boolean).find(existsSync);

const harness = `<!doctype html><html><body><script type="module">
import { getApplyAuthorEditOutcome }
  from "/generated/typescript/storyos-public-release-1/client.mjs";
const PROJECT = "018f0000-0000-7001-8000-000000000002";
const KEY = "018f0000-0000-7001-8000-000000000037";
const NONCE = "a".repeat(64);
const expected = {
  schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
  correlation_id: "018f0000-0000-7001-8000-000000000081",
  project_scope: { owner_user_id: "018f0000-0000-7001-8000-000000000001",
    project_id: PROJECT },
  outcome: { outcome_kind: "still_unknown", observation: {
    observation_kind: "admission_committed",
    command_id: "018f0000-0000-7001-8000-000000000031",
    author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
    reconciliation_required: true,
  } },
};
const fail = (message) => { throw new Error(message); };
const fetchImpl = async (url, options) => {
  const parsed = new URL(url);
  const expectedPath = "/api/v1/projects/" + PROJECT
    + "/manuscript/author-edit-outcomes/" + KEY;
  if (parsed.pathname !== expectedPath) fail("unexpected path: " + parsed.pathname);
  if (parsed.href.includes(NONCE)) fail("nonce entered the URL");
  if (options.method !== "GET" || options.body !== undefined) fail("Query shape changed");
  if (options.credentials !== "same-origin") fail("credentials policy changed");
  if (options.headers.accept !== "application/json") fail("Accept header is absent");
  if (options.headers["x-storyos-anti-forgery"] !== NONCE) fail("proof header mismatch");
  return new Response(JSON.stringify(expected), {
    status: 200, headers: { "content-type": "application/json", "cache-control": "no-store" },
  });
};
const report = (result, text) => fetch("/result", {
  method: "POST", headers: { "content-type": "application/json" },
  body: JSON.stringify({ result, text }),
});
try {
  let missingProofRejected = false;
  try {
    await getApplyAuthorEditOutcome({
      baseUrl: location.origin, projectId: PROJECT, idempotencyKey: KEY, fetchImpl,
    });
  } catch (error) { missingProofRejected = error instanceof TypeError; }
  if (!missingProofRejected) fail("missing proof was accepted");
  const actual = await getApplyAuthorEditOutcome({
    baseUrl: location.origin, projectId: PROJECT, idempotencyKey: KEY,
    antiForgery: NONCE, fetchImpl,
  });
  if (JSON.stringify(actual) !== JSON.stringify(expected)) fail("response changed");
  await report("pass", "protected outcome GET passed");
} catch (error) { await report("fail", error?.stack ?? String(error)); }
</script></body></html>`;

test("the generated outcome Query keeps its proof header-only in a real browser", {
  skip: chromeExecutable ? false : "Chrome or Chromium is unavailable", timeout: 30_000,
}, async () => {
  let resolveReport;
  const reported = new Promise((resolve) => { resolveReport = resolve; });
  const server = createServer(async (request, response) => {
    if (request.url === "/harness") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" }).end(harness);
      return;
    }
    if (request.url === "/result" && request.method === "POST") {
      let body = "";
      for await (const chunk of request) body += chunk;
      response.writeHead(204).end();
      resolveReport(JSON.parse(body));
      return;
    }
    const path = "/generated/typescript/storyos-public-release-1/client.mjs";
    if (request.url !== path) { response.writeHead(404).end(); return; }
    response.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
    response.end(await readFile(join(repositoryRoot, path)));
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const profileDirectory = mkdtempSync(join(tmpdir(), "storyos-outcome-browser-"));
  const url = `http://127.0.0.1:${server.address().port}/harness`;
  const browser = spawn(chromeExecutable, [
    "--headless=new", "--disable-gpu", "--disable-breakpad", "--disable-crash-reporter",
    "--disable-dev-shm-usage", "--no-first-run", `--user-data-dir=${profileDirectory}`, url,
  ], { stdio: "ignore" });
  try {
    const result = await Promise.race([
      reported,
      once(browser, "exit").then(([code]) => { throw new Error(`Chrome exited early: ${code}`); }),
      new Promise((_, reject) => setTimeout(
        () => reject(new Error("real-browser outcome Query timed out")), 10_000,
      ).unref()),
    ]);
    assert.deepEqual(result, { result: "pass", text: "protected outcome GET passed" });
  } finally {
    server.close();
    if (browser.exitCode === null) browser.kill("SIGTERM");
    await Promise.race([once(browser, "exit"), new Promise((resolve) => setTimeout(resolve, 2_000))]);
    if (browser.exitCode === null) browser.kill("SIGKILL");
    rmSync(profileDirectory, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
});
