import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "vitest";

import { TEST_CHALLENGE_SECRET } from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const workerBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32" ? "storyos-worker.exe" : "storyos-worker");
const webRoot = join(repositoryRoot, "target/release-package/web");
const LOCAL_USER = "018f0000-0000-7001-8000-000000000001";
const FOREIGN_USER = "018f0000-0000-7001-8000-000000000101";
const CLOSED_POSTGRES = "postgres://storyos_runtime:runtime@127.0.0.1:1/postgres";
const CLOSED_ADMIN = "postgres://postgres:wrong@127.0.0.1:1/postgres";

function packagedStartupEnv(sessions?: string): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    PATH: "/__storyos_no_external_tools__",
    STORYOS_WORKER: "0",
  };
  delete env.STORYOS_BOOTSTRAP_SESSIONS;
  delete env.STORYOS_TEST_ALLOW_MULTIPLE_BOOTSTRAP_SESSIONS;
  delete env.STORYOS_DATABASE_URL;
  delete env.STORYOS_STORAGE_ADMIN_URL;
  if (sessions !== undefined) env.STORYOS_BOOTSTRAP_SESSIONS = sessions;
  return env;
}

const execFileAsync = promisify(execFile);

async function refusesBind(
  args: string[],
  env: NodeJS.ProcessEnv,
  stderr?: RegExp,
): Promise<void> {
  await assert.rejects(execFileAsync(serverBinary, args, {
    cwd: repositoryRoot, timeout: 4_000, env,
  }), (error: unknown) => {
    assert.ok(error instanceof Error);
    assert.equal(Reflect.get(error, "code"), 1, error.message);
    assert.doesNotMatch(String(Reflect.get(error, "stdout")), /STORYOS_SERVER_URL=/);
    if (stderr !== undefined) {
      assert.match(String(Reflect.get(error, "stderr")), stderr);
      assert.doesNotMatch(String(Reflect.get(error, "stderr")), /STORYOS_DATABASE_URL/);
    }
    return true;
  });
}

test("production startup refuses a start without a web root", async () => {
  await refusesBind(["--bind", "127.0.0.1:0"], {
    ...process.env, PATH: "/__storyos_no_external_tools__",
  }, /usage: storyos-server --web-root/);
});

test("packaged production startup refuses session mappings that are not exactly one handle", async () => {
  const args = ["--web-root", webRoot, "--bind", "127.0.0.1:0"];
  const mappings = [undefined, "{}", `{"session-a":"${LOCAL_USER}","session-b":"${FOREIGN_USER}"}`, "{",
    `{"":"${LOCAL_USER}"}`, '{"session-a":"not-a-user"}', `{"session a":"${LOCAL_USER}"}`];
  for (const sessions of mappings) {
    await refusesBind(args, {
      ...packagedStartupEnv(sessions),
      STORYOS_CHALLENGE_SECRET: TEST_CHALLENGE_SECRET,
      STORYOS_DATABASE_URL: CLOSED_POSTGRES,
      STORYOS_STORAGE_ADMIN_URL: CLOSED_ADMIN,
    }, /STORYOS_BOOTSTRAP_SESSIONS/);
  }
});

test("offline web-root check does not require session mappings or PostgreSQL", async () => {
  const env = packagedStartupEnv();
  env.STORYOS_DATABASE_URL = CLOSED_POSTGRES;
  env.STORYOS_STORAGE_ADMIN_URL = CLOSED_ADMIN;
  await execFileAsync(serverBinary, ["--check-web-root", webRoot], {
    cwd: repositoryRoot, timeout: 4_000, env,
  });
});

test("offline worker check does not require PostgreSQL", async () => {
  await execFileAsync(workerBinary, ["--check"], {
    cwd: repositoryRoot,
    timeout: 4_000,
    env: {
      ...process.env,
      PATH: "/__storyos_no_external_tools__",
      STORYOS_DATABASE_URL: CLOSED_POSTGRES,
      STORYOS_STORAGE_ADMIN_URL: CLOSED_ADMIN,
    },
  });
});

test("packaged production startup refuses an invalid public Origin before Storage Activation", async () => {
  const env = packagedStartupEnv(`{"session-a":"${LOCAL_USER}"}`);
  env.STORYOS_PUBLIC_ORIGIN = "http://example.com";
  await refusesBind(
    ["--web-root", webRoot, "--bind", "127.0.0.1:0"],
    env,
    /STORYOS_PUBLIC_ORIGIN/,
  );
});

test("packaged production startup refuses a non-loopback listen when a public Origin is set", async () => {
  const env = packagedStartupEnv(`{"session-a":"${LOCAL_USER}"}`);
  env.STORYOS_PUBLIC_ORIGIN = "https://example.com";
  await refusesBind(
    ["--web-root", webRoot, "--bind", "0.0.0.0:3000"],
    env,
    /STORYOS_PUBLIC_ORIGIN/,
  );
});

test("packaged production startup refuses bind without a reachable Active proof", async () => {
  const args = ["--web-root", webRoot, "--bind", "127.0.0.1:0"];
  const sessions = `{"session-a":"${LOCAL_USER}"}`;
  await refusesBind(args, packagedStartupEnv(sessions));
  await refusesBind(args, {
    ...packagedStartupEnv(sessions),
    STORYOS_DATABASE_URL: CLOSED_POSTGRES,
    STORYOS_STORAGE_ADMIN_URL: CLOSED_ADMIN,
  });
});
