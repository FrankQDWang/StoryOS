import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import path from "node:path";

const scriptsDirectory = path.dirname(fileURLToPath(import.meta.url));
export const labRoot = path.resolve(scriptsDirectory, "..");

function childProcess(label, command, args, options = {}) {
  const child = spawn(command, args, {
    cwd: labRoot,
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  });
  child.stdout.on("data", (chunk) => {
    process.stdout.write(`[${label}] ${chunk}`);
  });
  child.stderr.on("data", (chunk) => {
    process.stderr.write(`[${label}] ${chunk}`);
  });
  return child;
}

async function waitForUrl(url, timeoutMs = 15_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      // The process is still starting.
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`service_start_timeout:${url}`);
}

export async function startLab({
  coreStatePath = path.join(labRoot, ".lab-state", "core-state.json"),
} = {}) {
  let stopping = false;
  let core = null;
  let restartTimer = null;

  const startCore = () => {
    core = childProcess(
      "core",
      process.execPath,
      ["server/core-server.mjs"],
      {
        env: {
          ...process.env,
          STORYOS_LAB_CORE_STATE: coreStatePath,
        },
      },
    );
    core.once("exit", (code, signal) => {
      if (stopping) return;
      process.stderr.write(
        `[core] exited code=${code} signal=${signal}; restarting from durable state\n`,
      );
      restartTimer = setTimeout(startCore, 75);
    });
  };

  startCore();
  const vite = childProcess(
    "vite",
    process.execPath,
    ["node_modules/vite/bin/vite.js", "--host", "127.0.0.1"],
  );

  try {
    await Promise.all([
      waitForUrl("http://127.0.0.1:41770/health"),
      waitForUrl("http://127.0.0.1:41769"),
    ]);
  } catch (error) {
    stopping = true;
    core?.kill("SIGTERM");
    vite.kill("SIGTERM");
    throw error;
  }

  return {
    editorUrl: "http://127.0.0.1:41769",
    coreUrl: "http://127.0.0.1:41770",
    coreStatePath,
    async stop() {
      stopping = true;
      clearTimeout(restartTimer);
      const children = [core, vite].filter(Boolean);
      await Promise.all(
        children.map(
          (child) =>
            new Promise((resolve) => {
              if (child.exitCode !== null) {
                resolve();
                return;
              }
              child.once("exit", resolve);
              child.kill("SIGTERM");
              setTimeout(() => {
                if (child.exitCode === null) child.kill("SIGKILL");
              }, 2_000).unref();
            }),
        ),
      );
    },
  };
}
