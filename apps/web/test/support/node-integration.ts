import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { once } from "node:events";
import { dirname, join } from "node:path";
import { promisify } from "node:util";

import {
  StoryOSProtocolError,
  createProject,
  createProjectChallenge,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";

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
  readonly webRoot?: string;
  readonly sessions?: Readonly<Record<string, string>>;
  readonly extraEnv?: Readonly<Record<string, string>>;
}): Promise<StoryOSServer> {
  const { bind = "127.0.0.1:0", repositoryRoot, serverBinary, sessions } = options;
  const webRoot = options.webRoot ?? join(dirname(serverBinary), "web");
  const env: NodeJS.ProcessEnv = { ...process.env, STORYOS_WORKER: "0", ...options.extraEnv };
  if (process.env.STORYOS_TEST_DATABASE_URL !== undefined) {
    env.STORYOS_DATABASE_URL = process.env.STORYOS_TEST_DATABASE_URL;
  }
  if (sessions !== undefined) {
    env.STORYOS_BOOTSTRAP_SESSIONS = JSON.stringify(sessions);
    env.STORYOS_CHALLENGE_SECRET =
      "test-only-challenge-secret-that-is-at-least-thirty-two-bytes";
    if (Object.keys(sessions).length !== 1) {
      env.STORYOS_TEST_ALLOW_MULTIPLE_BOOTSTRAP_SESSIONS = "1";
    }
  } else if (env.STORYOS_BOOTSTRAP_SESSIONS === undefined) {
    env.STORYOS_BOOTSTRAP_SESSIONS = JSON.stringify({
      "session-a": "018f0000-0000-7001-8000-000000000001",
    });
  }
  return new Promise((resolve, reject) => {
    const server = spawn(serverBinary, ["--bind", bind, "--web-root", webRoot], {
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

export async function runStoryOSWorker(options: {
  readonly repositoryRoot: string;
  readonly workerBinary: string;
  readonly args: readonly string[];
  readonly extraEnv?: Readonly<Record<string, string>>;
}): Promise<void> {
  const env = { ...process.env, ...options.extraEnv };
  if (process.env.STORYOS_TEST_DATABASE_URL !== undefined) {
    env.STORYOS_DATABASE_URL = process.env.STORYOS_TEST_DATABASE_URL;
  }
  await execFileAsync(options.workerBinary, [...options.args], {
    cwd: options.repositoryRoot,
    env,
    timeout: 15_000,
    killSignal: "SIGKILL",
  });
}

/** Creates one empty Project through the public protocol and returns its Project ID. */
export async function createEmptyProject(options: {
  readonly baseUrl: string;
  readonly fetchImpl: typeof fetch;
  readonly createKey: string;
  readonly correlationId: string;
  readonly title: string;
}): Promise<string> {
  const { baseUrl, fetchImpl, createKey } = options;
  const createRequest = {
    command_schema: "storyos.command.create-project.request.v1" as const,
    create_project_input: {
      title: options.title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: options.correlationId,
    },
    idempotency_key: createKey,
  };
  const created = await createProjectChallenge({ baseUrl, request: createRequest, fetchImpl });
  await createProject({
    baseUrl,
    fetchImpl,
    idempotencyKey: createKey,
    antiForgery: created.nonce,
    request: {
      command_schema: createRequest.command_schema,
      prospective_project_id: created.prospective_project_id,
      create_project_input: createRequest.create_project_input,
    },
  });
  return created.prospective_project_id;
}

export async function queryStoryOSPostgres(query: string): Promise<string> {
  const container = process.env.STORYOS_TEST_POSTGRES_CONTAINER;
  assert.ok(container, "run through scripts/verify-project-scope.sh");
  const { stdout } = await execFileAsync("docker", [
    "exec", container, "psql", "-XAt", "-U", "postgres", "-c", query,
  ]);
  return stdout.trim();
}

export async function exportSettlementReceipt(options: {
  readonly ownerUserId: string;
  readonly projectId: string;
  readonly exportId: string;
  readonly operationsTable:
    | "human_readable_manuscript_export_operations"
    | "project_export_operations";
}): Promise<string> {
  return queryStoryOSPostgres(`
    SELECT receipt.result_kind || ' ' || (receipt.result_payload->>'reason')
      FROM storyos.domain_receipts AS receipt
      JOIN storyos.${options.operationsTable} AS operation
        ON operation.owner_user_id = receipt.owner_user_id
       AND operation.project_id = receipt.project_id
       AND operation.author_command_admission_id = receipt.author_command_admission_id
     WHERE operation.owner_user_id = '${options.ownerUserId}'::uuid
       AND operation.project_id = '${options.projectId}'::uuid
       AND operation.export_id = '${options.exportId}'::uuid;
  `);
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
