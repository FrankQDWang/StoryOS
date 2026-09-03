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

async function startRealServer(bind = "127.0.0.1:0") {
  return startStoryOSServer({
    bind,
    repositoryRoot,
    serverBinary,
    sessions: SESSIONS,
  });
}

async function restartServer(server: ChildProcess, baseUrl: string) {
  const url = new URL(baseUrl);
  await stopRealServer(server);
  return startRealServer(`${url.hostname}:${url.port}`);
}

test("an admitted human-readable export stays in progress across a Server process cut", async () => {
  const { stdout } = await execFileAsync(workerBinary, ["--check"]);
  assert.equal(stdout, "");

  const first = await startRealServer();
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

    const restarted = await restartServer(first.server, first.baseUrl);
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
