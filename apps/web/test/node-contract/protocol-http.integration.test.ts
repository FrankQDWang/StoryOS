import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, renameSync, rmSync, symlinkSync, unlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "vitest";

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

test("production startup refuses an absent root or invalid resource set before readiness", async () => {
  await refusesBind(["--bind", "127.0.0.1:0"], {
    ...process.env, PATH: "/__storyos_no_external_tools__",
  });
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
      await refusesBind(["--web-root", root, "--bind", "127.0.0.1:0"], {
        ...process.env, PATH: "/__storyos_no_external_tools__",
      });
    } finally {
      rmSync(temporary, { recursive: true, force: true });
    }
  }
});

test("packaged production startup refuses session mappings that are not exactly one handle", async () => {
  const args = ["--web-root", webRoot, "--bind", "127.0.0.1:0"];
  await refusesBind(args, packagedStartupEnv());
  await refusesBind(args, packagedStartupEnv("{}"));
  await refusesBind(args, packagedStartupEnv(`{"session-a":"${LOCAL_USER}","session-b":"${FOREIGN_USER}"}`));
  await refusesBind(args, packagedStartupEnv("{"));
  await refusesBind(args, packagedStartupEnv(`{"":"${LOCAL_USER}"}`));
  await refusesBind(args, packagedStartupEnv('{"session-a":"not-a-user"}'));
  await refusesBind(args, packagedStartupEnv(`{"session a":"${LOCAL_USER}"}`));
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
