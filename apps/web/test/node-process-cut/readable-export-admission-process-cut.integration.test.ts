import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "vitest";

import {
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  digestExportHumanReadableManuscript,
  exportHumanReadableManuscript,
  getHumanReadableManuscriptExport,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateProjectChallengeRequest,
  ExportHumanReadableManuscriptRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const packageDir = join(repositoryRoot, "target", "release-package");
const serverBinary = join(
  packageDir,
  process.platform === "win32" ? "storyos-server.exe" : "storyos-server",
);
const workerBinary = join(
  packageDir,
  process.platform === "win32" ? "storyos-worker.exe" : "storyos-worker",
);
const USER = "018f0000-0000-7001-8000-000000000001";
const SESSION_HANDLE = "session-a";
const SESSIONS = { [SESSION_HANDLE]: USER };
const execFileAsync = promisify(execFile);

function createChallengeRequest(idempotencyKey: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title: "Readable Export Cut",
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-00000000af10",
    },
    idempotency_key: idempotencyKey,
  };
}

function exportRequest(): ExportHumanReadableManuscriptRequest {
  return {
    command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
    export_human_readable_manuscript_input: {
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-00000000af11",
    },
  };
}

async function startRealServer(bind = "127.0.0.1:0", extraEnv?: Readonly<Record<string, string>>) {
  return startStoryOSServer({
    bind,
    repositoryRoot,
    serverBinary,
    sessions: SESSIONS,
    ...(extraEnv === undefined ? {} : { extraEnv }),
  });
}

async function restartServer(
  server: ChildProcess,
  baseUrl: string,
  extraEnv?: Readonly<Record<string, string>>,
) {
  const url = new URL(baseUrl);
  await stopRealServer(server);
  return startRealServer(`${url.hostname}:${url.port}`, extraEnv);
}

async function runWorker(args: string[], extraEnv: Readonly<Record<string, string>> = {}) {
  const env = {
    ...process.env,
    ...extraEnv,
  };
  if (process.env.STORYOS_TEST_DATABASE_URL !== undefined) {
    env.STORYOS_DATABASE_URL = process.env.STORYOS_TEST_DATABASE_URL;
  }
  await execFileAsync(workerBinary, args, {
    cwd: repositoryRoot,
    env,
    timeout: 15_000,
    killSignal: "SIGKILL",
  });
}

test("an admitted human-readable export stays in progress across a Server process cut", async () => {
  const { stdout } = await execFileAsync(workerBinary, ["--check"]);
  assert.equal(stdout, "");

  const first = await startRealServer("127.0.0.1:0", { STORYOS_WORKER: "0" });
  try {
    const fetchImpl = browserFetch(first.baseUrl, SESSION_HANDLE);
    const createRequest = createChallengeRequest("018f0000-0000-7001-8000-00000000af01");
    const created = await createProjectChallenge({
      baseUrl: first.baseUrl,
      request: createRequest,
      fetchImpl,
    });
    await createProject({
      baseUrl: first.baseUrl,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000af01",
      antiForgery: created.nonce,
      request: {
        command_schema: createRequest.command_schema,
        prospective_project_id: created.prospective_project_id,
        create_project_input: createRequest.create_project_input,
      },
    });
    const request = exportRequest();
    const digest = await digestExportHumanReadableManuscript(request);
    const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl: first.baseUrl,
      projectId: created.prospective_project_id,
      fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/exports",
        command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
        canonical_command_digest: digest,
        idempotency_key: "018f0000-0000-7001-8000-00000000af21",
      },
    }));
    const admitted = await exportHumanReadableManuscript({
      baseUrl: first.baseUrl,
      projectId: created.prospective_project_id,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000af21",
      antiForgery: challenge.nonce,
      request,
    });
    assert.equal(admitted.acknowledgement, "accepted");
    assert.equal(admitted.effect.kind, "admitted");
    if (admitted.effect.kind !== "admitted") {
      throw new Error("Human-readable export must admit");
    }
    const exportId = admitted.effect.export_id;

    const restarted = await restartServer(first.server, first.baseUrl, { STORYOS_WORKER: "0" });
    try {
      const afterCut = await getHumanReadableManuscriptExport({
        baseUrl: restarted.baseUrl,
        projectId: created.prospective_project_id,
        exportId,
        fetchImpl: browserFetch(restarted.baseUrl, SESSION_HANDLE),
      });
      assert.equal(afterCut.status, "in_progress");
      if (afterCut.status !== "in_progress") {
        throw new Error("the admitted export must survive the process cut");
      }
      assert.equal(afterCut.export_id, exportId);
      assert.equal("manuscript_utf8" in afterCut, false);
    } finally {
      await stopRealServer(restarted.server);
    }
  } catch (error) {
    await stopRealServer(first.server);
    throw error;
  }
});

test("a Worker claim without settlement is outcome_unknown, then takeover settles ready", async () => {
  await runWorker(["--once"], { STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
  await runWorker(["--once"], { STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
  const first = await startRealServer("127.0.0.1:0", { STORYOS_WORKER: "0" });
  try {
    const fetchImpl = browserFetch(first.baseUrl, SESSION_HANDLE);
    const createRequest = {
      ...createChallengeRequest("018f0000-0000-7001-8000-00000000af02"),
    };
    createRequest.create_project_input = {
      ...createRequest.create_project_input,
      correlation_id: "018f0000-0000-7001-8000-00000000af12",
    };
    const created = await createProjectChallenge({
      baseUrl: first.baseUrl,
      request: createRequest,
      fetchImpl,
    });
    await createProject({
      baseUrl: first.baseUrl,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000af02",
      antiForgery: created.nonce,
      request: {
        command_schema: createRequest.command_schema,
        prospective_project_id: created.prospective_project_id,
        create_project_input: createRequest.create_project_input,
      },
    });
    const request = {
      command_schema: "storyos.command.export-human-readable-manuscript.request.v1" as const,
      export_human_readable_manuscript_input: {
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1",
        correlation_id: "018f0000-0000-7001-8000-00000000af13",
      },
    };
    const digest = await digestExportHumanReadableManuscript(request);
    const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl: first.baseUrl,
      projectId: created.prospective_project_id,
      fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/exports",
        command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
        canonical_command_digest: digest,
        idempotency_key: "018f0000-0000-7001-8000-00000000af22",
      },
    }));
    const admitted = await exportHumanReadableManuscript({
      baseUrl: first.baseUrl,
      projectId: created.prospective_project_id,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000af22",
      antiForgery: challenge.nonce,
      request,
    });
    if (admitted.effect.kind !== "admitted") {
      throw new Error("Human-readable export must admit");
    }
    const exportId = admitted.effect.export_id;
    const projectId = created.prospective_project_id;
    await stopRealServer(first.server);

    await runWorker(["--claim-only"], { STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
    const afterClaim = await startRealServer("127.0.0.1:0", { STORYOS_WORKER: "0" });
    try {
      const unknown = await getHumanReadableManuscriptExport({
        baseUrl: afterClaim.baseUrl,
        projectId,
        exportId,
        fetchImpl: browserFetch(afterClaim.baseUrl, SESSION_HANDLE),
      });
      assert.equal(unknown.status, "outcome_unknown");
      assert.equal("manuscript_utf8" in unknown, false);
      const receipts = await queryStoryOSPostgres(`
        SELECT count(*)::text
          FROM storyos.domain_receipts
         WHERE owner_user_id = '${USER}'::uuid
           AND project_id = '${projectId}'::uuid
           AND command_kind = 'exportHumanReadableManuscript';
      `);
      assert.match(receipts, /0$/);
    } finally {
      await stopRealServer(afterClaim.server);
    }

    await runWorker(["--once"], { STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
    const afterSettle = await startRealServer("127.0.0.1:0", { STORYOS_WORKER: "0" });
    try {
      const ready = await getHumanReadableManuscriptExport({
        baseUrl: afterSettle.baseUrl,
        projectId,
        exportId,
        fetchImpl: browserFetch(afterSettle.baseUrl, SESSION_HANDLE),
      });
      assert.equal(ready.status, "ready");
      if (ready.status !== "ready") {
        throw new Error("Worker takeover must settle the human-readable export");
      }
      assert.equal(ready.manuscript_utf8, "\n");
    } finally {
      await stopRealServer(afterSettle.server);
    }
  } catch (error) {
    await stopRealServer(first.server);
    throw error;
  }
});
