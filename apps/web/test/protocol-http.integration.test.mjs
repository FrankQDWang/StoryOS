import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { bootProtectedWebClient } from "../src/boot.mjs";

const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");

async function startRealServer() {
  return new Promise((resolve, reject) => {
    const server = spawn(serverBinary, ["--bind", "127.0.0.1:0"], {
      cwd: repositoryRoot, stdio: ["ignore", "pipe", "pipe"],
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
  server.kill("SIGTERM"); await exited;
}

test("the generated client boots protected Web state over the real Server HTTP boundary", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    assert.deepEqual(await bootProtectedWebClient({ baseUrl }), {
      kind: "protected-ready", profile: RELEASE_1_PROTOCOL_PROFILE,
    });
  } finally {
    await stopRealServer(server);
  }
});
