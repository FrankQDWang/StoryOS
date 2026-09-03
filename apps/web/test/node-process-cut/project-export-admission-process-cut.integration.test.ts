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
  digestExportProjectArchive,
  exportProjectArchive,
  getExportOperation,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  CreateProjectChallengeRequest,
  ExportProjectArchiveRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres,
  runStoryOSWorker,
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
const ARCHIVE_MEDIA =
  'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"';

function createChallengeRequest(idempotencyKey: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title: "Archive Export Cut",
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-00000000e010",
    },
    idempotency_key: idempotencyKey,
  };
}

function exportRequest(): ExportProjectArchiveRequest {
  return {
    command_schema: "storyos.command.export-project-archive.request.v1",
    export_project_archive_input: {
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-00000000e011",
      archive_profile: "storyos.project-export.v1",
      archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
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
  await runStoryOSWorker({
    repositoryRoot,
    workerBinary,
    args,
    extraEnv,
  });
}

test("an admitted Project Export Archive stays in progress across a Server process cut", async () => {
  const { stdout } = await execFileAsync(workerBinary, ["--check"]);
  assert.equal(stdout, "");

  const first = await startRealServer("127.0.0.1:0", { STORYOS_WORKER: "0" });
  try {
    const fetchImpl = browserFetch(first.baseUrl, SESSION_HANDLE);
    const createRequest = createChallengeRequest("018f0000-0000-7001-8000-00000000e001");
    const created = await createProjectChallenge({
      baseUrl: first.baseUrl,
      request: createRequest,
      fetchImpl,
    });
    await createProject({
      baseUrl: first.baseUrl,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e001",
      antiForgery: created.nonce,
      request: {
        command_schema: createRequest.command_schema,
        prospective_project_id: created.prospective_project_id,
        create_project_input: createRequest.create_project_input,
      },
    });
    const request = exportRequest();
    const digest = await digestExportProjectArchive(request);
    const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl: first.baseUrl,
      projectId: created.prospective_project_id,
      fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/exports",
        command_schema: "storyos.command.export-project-archive.request.v1",
        canonical_command_digest: digest,
        idempotency_key: "018f0000-0000-7001-8000-00000000e021",
      },
    }));
    const admitted = await exportProjectArchive({
      baseUrl: first.baseUrl,
      projectId: created.prospective_project_id,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e021",
      antiForgery: challenge.nonce,
      request,
    });
    assert.equal(admitted.acknowledgement, "accepted");
    assert.equal(admitted.effect.kind, "admitted");
    if (admitted.effect.kind !== "admitted") {
      throw new Error("Project Export must admit");
    }
    const exportId = admitted.effect.export_id;

    const restarted = await restartServer(first.server, first.baseUrl, { STORYOS_WORKER: "0" });
    try {
      const afterCut = await getExportOperation({
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
      assert.equal("immutable_root" in afterCut, false);
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
      ...createChallengeRequest("018f0000-0000-7001-8000-00000000e002"),
    };
    createRequest.create_project_input = {
      ...createRequest.create_project_input,
      correlation_id: "018f0000-0000-7001-8000-00000000e012",
    };
    const created = await createProjectChallenge({
      baseUrl: first.baseUrl,
      request: createRequest,
      fetchImpl,
    });
    await createProject({
      baseUrl: first.baseUrl,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e002",
      antiForgery: created.nonce,
      request: {
        command_schema: createRequest.command_schema,
        prospective_project_id: created.prospective_project_id,
        create_project_input: createRequest.create_project_input,
      },
    });
    const request = {
      command_schema: "storyos.command.export-project-archive.request.v1" as const,
      export_project_archive_input: {
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1",
        correlation_id: "018f0000-0000-7001-8000-00000000e013",
        archive_profile: "storyos.project-export.v1",
        archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
      },
    };
    const digest = await digestExportProjectArchive(request);
    const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl: first.baseUrl,
      projectId: created.prospective_project_id,
      fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/exports",
        command_schema: "storyos.command.export-project-archive.request.v1",
        canonical_command_digest: digest,
        idempotency_key: "018f0000-0000-7001-8000-00000000e022",
      },
    }));
    const admitted = await exportProjectArchive({
      baseUrl: first.baseUrl,
      projectId: created.prospective_project_id,
      fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e022",
      antiForgery: challenge.nonce,
      request,
    });
    if (admitted.effect.kind !== "admitted") {
      throw new Error("Project Export must admit");
    }
    const exportId = admitted.effect.export_id;
    const projectId = created.prospective_project_id;
    await stopRealServer(first.server);

    await runWorker(["--claim-only"], { STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
    const afterClaim = await startRealServer("127.0.0.1:0", { STORYOS_WORKER: "0" });
    try {
      const unknown = await getExportOperation({
        baseUrl: afterClaim.baseUrl,
        projectId,
        exportId,
        fetchImpl: browserFetch(afterClaim.baseUrl, SESSION_HANDLE),
      });
      assert.equal(unknown.status, "outcome_unknown");
      assert.equal("immutable_root" in unknown, false);
      const receipts = await queryStoryOSPostgres(`
        SELECT count(*)::text
          FROM storyos.domain_receipts
         WHERE owner_user_id = '${USER}'::uuid
           AND project_id = '${projectId}'::uuid
           AND command_kind = 'exportProjectArchive';
      `);
      assert.match(receipts, /0$/);
    } finally {
      await stopRealServer(afterClaim.server);
    }

    await runWorker(["--once"], { STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
    const afterSettle = await startRealServer("127.0.0.1:0", { STORYOS_WORKER: "0" });
    try {
      const ready = await getExportOperation({
        baseUrl: afterSettle.baseUrl,
        projectId,
        exportId,
        fetchImpl: browserFetch(afterSettle.baseUrl, SESSION_HANDLE),
      });
      assert.equal(ready.status, "ready");
      if (ready.status !== "ready") {
        throw new Error("Worker takeover must settle the Project Export Archive");
      }
      assert.match(ready.immutable_root, /^sha256:[0-9a-f]{64}$/);
      const exportUrl = `${afterSettle.baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/exports/${encodeURIComponent(exportId)}`;
      const zipResponse = await browserFetch(afterSettle.baseUrl, SESSION_HANDLE)(exportUrl, {
        headers: { Accept: ARCHIVE_MEDIA },
      });
      assert.equal(zipResponse.status, 200);
      assert.deepEqual(
        Array.from(new Uint8Array(await zipResponse.arrayBuffer()).slice(0, 4)),
        [0x50, 0x4b, 0x03, 0x04],
      );
    } finally {
      await stopRealServer(afterSettle.server);
    }
  } catch (error) {
    await stopRealServer(first.server);
    throw error;
  }
});
