import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, readdirSync, renameSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { bootProtectedWebClient } from "../../src/boot.ts";
import {
  startStoryOSServer,
  stopStoryOSServer,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const webRoot = join(repositoryRoot, "target/release-package/web");
const LOCAL_USER = "018f0000-0000-7001-8000-000000000001";
const FOREIGN_USER = "018f0000-0000-7001-8000-000000000101";

function packagedStartupEnv(sessions?: string): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    PATH: "/__storyos_no_external_tools__",
    STORYOS_WORKER: "0",
  };
  delete env.STORYOS_BOOTSTRAP_SESSIONS;
  delete env.STORYOS_TEST_ALLOW_MULTIPLE_BOOTSTRAP_SESSIONS;
  if (sessions !== undefined) env.STORYOS_BOOTSTRAP_SESSIONS = sessions;
  return env;
}

const CSP = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'; worker-src 'none'; require-trusted-types-for 'script'; trusted-types 'none'";
const execFileAsync = promisify(execFile);

test("the generated client boots protected Web state over the real Server HTTP boundary", async () => {
  const { baseUrl, server } = await startStoryOSServer({ repositoryRoot, serverBinary });
  try {
    assert.deepEqual(await bootProtectedWebClient({ baseUrl }), {
      kind: "protected-ready", profile: RELEASE_1_PROTOCOL_PROFILE,
    });
  } finally {
    await stopStoryOSServer(server);
  }
});

test("production startup refuses an absent root or invalid resource set before readiness", async () => {
  const refuses = async (args: string[]) => {
    await assert.rejects(execFileAsync(serverBinary, args, {
      cwd: repositoryRoot, timeout: 4_000,
      env: { ...process.env, PATH: "/__storyos_no_external_tools__" },
    }), (error: unknown) => {
      assert.ok(error instanceof Error);
      assert.equal(Reflect.get(error, "code"), 1, error.message);
      assert.doesNotMatch(String(Reflect.get(error, "stdout")), /STORYOS_SERVER_URL=/);
      return true;
    });
  };
  await refuses(["--bind", "127.0.0.1:0"]);
  const mutations: ReadonlyArray<(root: string, temporary: string) => void> = [
    (root) => unlinkSync(join(root, "index.html")),
    (root) => writeFileSync(join(root, "assets/extra-12345678.js"), "extra"),
    (root) => {
      const path = join(root, "index.html");
      const bytes = readFileSync(path);
      bytes[0] = 0;
      writeFileSync(path, bytes);
    },
    (root) => {
      const path = join(root, "manifest.json");
      writeFileSync(path, readFileSync(path, "utf8")
        .replace(/"source_commit": "[0-9a-f]{40}"/, `"source_commit": "${"c".repeat(40)}"`));
    },
    (root, temporary) => {
      const path = join(root, "index.html");
      const held = join(temporary, "held.html");
      renameSync(path, held);
      symlinkSync(held, path);
    },
  ];
  for (const mutate of mutations) {
    const temporary = mkdtempSync(join(tmpdir(), "storyos-web-startup-"));
    try {
      const root = join(temporary, "web");
      cpSync(webRoot, root, { recursive: true });
      mutate(root, temporary);
      await refuses(["--web-root", root, "--bind", "127.0.0.1:0"]);
    } finally {
      rmSync(temporary, { recursive: true, force: true });
    }
  }
});

test("packaged production startup refuses session mappings that are not exactly one handle", async () => {
  const refuses = async (sessions?: string, extra: NodeJS.ProcessEnv = {}) => {
    await assert.rejects(execFileAsync(serverBinary, ["--web-root", webRoot, "--bind", "127.0.0.1:0"], {
      cwd: repositoryRoot, timeout: 4_000,
      env: { ...packagedStartupEnv(sessions), ...extra },
    }), (error: unknown) => {
      assert.ok(error instanceof Error);
      assert.equal(Reflect.get(error, "code"), 1, error.message);
      assert.doesNotMatch(String(Reflect.get(error, "stdout")), /STORYOS_SERVER_URL=/);
      return true;
    });
  };
  await refuses();
  await refuses("{}");
  await refuses(`{"session-a":"${LOCAL_USER}","session-b":"${FOREIGN_USER}"}`);
  await refuses("{");
  await refuses(`{"":"${LOCAL_USER}"}`);
  await refuses('{"session-a":"not-a-user"}');
  await refuses(`{"session a":"${LOCAL_USER}"}`);
});

test("offline web-root check does not require session mappings", async () => {
  const env = packagedStartupEnv();
  await execFileAsync(serverBinary, ["--check-web-root", webRoot], {
    cwd: repositoryRoot, timeout: 4_000, env,
  });
});

test("printed origin HTML GET issues the configured session cookie", async () => {
  const { baseUrl, server } = await startStoryOSServer({
    repositoryRoot, serverBinary, sessions: { "session-a": LOCAL_USER },
  });
  try {
    const response = await fetch(`${baseUrl}/`);
    assert.equal(response.status, 200);
    assert.equal(
      response.headers.get("set-cookie"),
      "storyos_session=session-a; HttpOnly; SameSite=Strict; Path=/; Max-Age=28800",
    );
    const html = await response.text();
    assert.doesNotMatch(html, /session-a/);
    const protocol = await fetch(`${baseUrl}/api/v1/protocol`);
    assert.equal(protocol.status, 200);
    assert.equal(protocol.headers.get("set-cookie"), null);
    const mismatchedProtocol = await fetch(`${baseUrl}/api/v1/protocol`, {
      headers: { cookie: "storyos_session=other" },
    });
    assert.equal(mismatchedProtocol.status, 200);
    assert.equal(mismatchedProtocol.headers.get("set-cookie"), null);
    const mismatchedHtml = await fetch(`${baseUrl}/`, {
      headers: { cookie: "storyos_session=other" },
    });
    assert.equal(mismatchedHtml.status, 200);
    assert.equal(mismatchedHtml.headers.get("set-cookie"), null);
  } finally {
    await stopStoryOSServer(server);
  }
});

test("two-User isolation mappings start without issuing a cookie", async () => {
  const { baseUrl, server } = await startStoryOSServer({
    repositoryRoot, serverBinary,
    sessions: { "session-a": LOCAL_USER, "session-b": FOREIGN_USER },
  });
  try {
    const response = await fetch(`${baseUrl}/`);
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("set-cookie"), null);
  } finally {
    await stopStoryOSServer(server);
  }
});

test("ready production responses use the validated snapshot after its directory moves", async () => {
  const temporary = mkdtempSync(join(tmpdir(), "storyos-web-snapshot-"));
  try {
    const root = join(temporary, "web");
    cpSync(webRoot, root, { recursive: true });
    const script = readdirSync(join(root, "assets")).find((name) => name.endsWith(".js"));
    assert.ok(script);
    const expected = [readFileSync(join(root, "index.html")), readFileSync(join(root, "assets", script))];
    const { baseUrl, server } = await startStoryOSServer({ repositoryRoot, serverBinary, webRoot: root });
    try {
      renameSync(root, join(temporary, "moved"));
      const actual = [];
      for (const path of ["/", `/assets/${script}`]) {
        const response = await fetch(`${baseUrl}${path}`);
        assert.equal(response.status, 200);
        actual.push(Buffer.from(await response.arrayBuffer()));
      }
      assert.deepEqual(actual, expected);
    } finally {
      await stopStoryOSServer(server);
    }
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("the production process serves exact pages and resources with restrictive HTTP policy", async () => {
  const { baseUrl, server } = await startStoryOSServer({ repositoryRoot, serverBinary });
  try {
    const script = readdirSync(join(webRoot, "assets")).find((name) => name.endsWith(".js"));
    assert.ok(script);
    const projectPath = "/projects/018f0000-0000-7001-8000-000000000002";
    for (const [path, file, mime, cache] of [
      ["/", "index.html", "text/html; charset=utf-8", "no-store"],
      [projectPath, "index.html", "text/html; charset=utf-8", "no-store"],
      [`${projectPath.toUpperCase()}/`, "index.html", "text/html; charset=utf-8", "no-store"],
      [`/assets/${script}`, `assets/${script}`, "text/javascript; charset=utf-8", "public, max-age=31536000, immutable"],
    ] as const) {
      const expected = readFileSync(join(webRoot, file));
      for (const method of ["GET", "HEAD"]) {
        const response = await fetch(`${baseUrl}${path}`, { method });
        assert.deepEqual({
          status: response.status,
          mime: response.headers.get("content-type"),
          length: response.headers.get("content-length"),
          cache: response.headers.get("cache-control"),
          csp: response.headers.get("content-security-policy"),
          sniff: response.headers.get("x-content-type-options"),
          referrer: response.headers.get("referrer-policy"),
          crossOrigin: response.headers.get("cross-origin-resource-policy"),
        }, {
          status: 200, mime, length: String(expected.length), cache, csp: CSP,
          sniff: "nosniff", referrer: "same-origin", crossOrigin: "same-origin",
        });
        assert.deepEqual(Buffer.from(await response.arrayBuffer()), method === "HEAD" ? Buffer.alloc(0) : expected);
      }
      for (const method of ["POST", "PUT", "PATCH", "DELETE", "OPTIONS"]) {
        const response = await fetch(`${baseUrl}${path}`, { method });
        assert.deepEqual({ status: response.status, allow: response.headers.get("allow") },
          { status: 405, allow: "GET, HEAD" });
      }
    }
    for (const path of ["/missing", "/src/main.ts", "/@vite/client", "/assets/", "/manifest.json",
      "/api/v1/unknown", "/projects/invalid", `${projectPath}//`, `${projectPath}/extra`]) {
      const response = await fetch(`${baseUrl}${path}`);
      assert.equal(response.status, 404, path);
      assert.equal(await response.text(), "", path);
    }
    const protectedApi = await fetch(`${baseUrl}/api/v1${projectPath}`, { headers: { origin: baseUrl } });
    assert.equal(protectedApi.status, 401);
    assert.match(protectedApi.headers.get("content-type") ?? "", /application\/json/);
  } finally {
    await stopStoryOSServer(server);
  }
});
