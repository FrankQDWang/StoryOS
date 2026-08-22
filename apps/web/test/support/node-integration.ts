import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { once } from "node:events";
import { promisify } from "node:util";

import { StoryOSProtocolError } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";

const execFileAsync = promisify(execFile);

export interface StoryOSServer {
  readonly baseUrl: string;
  readonly server: ChildProcess;
}

export function sessionFetch(baseUrl: string, sessionHandle?: string): typeof fetch {
  return (input, init) => {
    const headers = new Headers(init?.headers);
    headers.set("origin", baseUrl);
    if (sessionHandle !== undefined && sessionHandle.length > 0) {
      headers.set("cookie", `storyos_session=${sessionHandle}`);
    }
    return fetch(input, { ...init, headers });
  };
}

export async function startStoryOSServer(options: {
  readonly bind?: string;
  readonly repositoryRoot: string;
  readonly serverBinary: string;
  readonly sessions?: Readonly<Record<string, string>>;
}): Promise<StoryOSServer> {
  const { bind = "127.0.0.1:0", repositoryRoot, serverBinary, sessions } = options;
  const env = { ...process.env };
  if (process.env.STORYOS_TEST_DATABASE_URL !== undefined) {
    env.STORYOS_DATABASE_URL = process.env.STORYOS_TEST_DATABASE_URL;
  }
  if (sessions !== undefined) {
    env.STORYOS_BOOTSTRAP_SESSIONS = JSON.stringify(sessions);
    env.STORYOS_CHALLENGE_SECRET =
      "test-only-challenge-secret-that-is-at-least-thirty-two-bytes";
  }
  return new Promise((resolve, reject) => {
    const server = spawn(serverBinary, ["--bind", bind], {
      cwd: repositoryRoot,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    const fail = (error: Error): void => {
      clearTimeout(timeout);
      server.kill("SIGTERM");
      reject(error);
    };
    const timeout = setTimeout(
      () => fail(new Error(`StoryOS Server did not become ready: ${stderr}`)),
      5_000,
    );
    server.once("error", fail);
    server.once("exit", (code) => {
      fail(new Error(`StoryOS Server exited with ${code}: ${stderr}`));
    });
    server.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
    });
    server.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString();
      const baseUrl = stdout.match(/^STORYOS_SERVER_URL=(http:\/\/[^\s]+)$/m)?.[1];
      if (baseUrl !== undefined) {
        clearTimeout(timeout);
        resolve({ baseUrl, server });
      }
    });
  });
}

export async function stopStoryOSServer(server: ChildProcess): Promise<void> {
  if (server.exitCode !== null) return;
  const exited = once(server, "exit");
  server.kill("SIGTERM");
  await exited;
}

export async function queryStoryOSPostgres(query: string): Promise<string> {
  const container = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
  assert.ok(container, "run through scripts/verify-project-scope.sh");
  const { stdout } = await execFileAsync("docker", [
    "exec", container, "psql", "-XAt", "-U", "postgres", "-c", query,
  ]);
  return stdout.trim();
}

export async function withChallengeRetry<Result>(
  action: () => Promise<Result>,
): Promise<Result> {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    try {
      return await action();
    } catch (error) {
      if (!(error instanceof StoryOSProtocolError) || error.status !== 429 || attempt === 3) {
        throw error;
      }
      await new Promise<void>((resolve) => {
        setTimeout(resolve, ((error.retryAfterSeconds ?? 1) + 1) * 1000);
      });
    }
  }
  throw new Error("command challenge retry exhausted");
}

export function requireStoryOSProtocolError(error: unknown): StoryOSProtocolError {
  assert.ok(error instanceof StoryOSProtocolError);
  return error;
}
