// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/diagnostic-canary-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createAgentRun,
  createChapter,
  createProjectCommandChallenge,
  createVolume,
  digestCreateAgentRun,
  digestCreateChapter,
  digestCreateVolume,
  digestExportHumanReadableManuscript,
  digestUpdateProjectAssistance,
  exportHumanReadableManuscript,
  getAgentRun,
  getHumanReadableManuscriptExport,
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DigestValue } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  assertNoCanary,
  canary,
  canaryDatabaseUrl,
  closedSpans,
  diagnosticLines,
} from "../support/diagnostic-canary.ts";
import {
  createEmptyProject,
  runStoryOSWorkerStderr,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeBudget,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const exe = process.platform === "win32" ? ".exe" : "";
const serverBinary = join(repositoryRoot, "target", "release-package", `storyos-server${exe}`);
const workerBinary = join(repositoryRoot, "target", "release-package", `storyos-worker${exe}`);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};

function id(suffix: string): string {
  return `018f0000-0000-7001-8000-0000000a48${suffix}`;
}

async function challenged<Result>(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  projectId: string;
  method: string;
  route: string;
  schema: string;
  idempotencyKey: string;
  digest: DigestValue;
  send: (antiForgery: string) => Promise<Result>;
}): Promise<Result> {
  const challenge = await withChallengeBudget(options.projectId, () => createProjectCommandChallenge({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    fetchImpl: options.fetchImpl,
    request: {
      method: options.method,
      route_template: options.route,
      command_schema: options.schema,
      canonical_command_digest: options.digest,
      idempotency_key: options.idempotencyKey,
    },
  }));
  return options.send(challenge.nonce);
}

test("the Diagnostic Projection of Worker claims and dispatches holds no author text", async () => {
  const title = canary("title");
  const message = canary("author-message");
  const applicationName = canary("database").replaceAll("-", "_");
  const databaseUrl = canaryDatabaseUrl(applicationName);
  const { baseUrl, server, stderr: serverStderr } = await startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A },
    databaseUrl,
    extraEnv: { STORYOS_LOG: "debug" },
  });
  let workerStderr = "";
  try {
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const projectId = await createEmptyProject({
      baseUrl, fetchImpl, createKey: id("01"), correlationId: id("11"), title,
    });
    const command = { baseUrl, fetchImpl, projectId };
    const assistance = {
      command_schema: "storyos.command.update-project-assistance.request.v1" as const,
      update_project_assistance_input: {
        availability: "available" as const,
        expected_assistance_revision: "0",
        ...BINDING,
        correlation_id: id("12"),
      },
    };
    await challenged({
      ...command,
      method: "PUT",
      route: "/api/v1/projects/{project_id}/assistance",
      schema: assistance.command_schema,
      idempotencyKey: id("02"),
      digest: await digestUpdateProjectAssistance(assistance),
      send: (antiForgery) => updateProjectAssistance({
        ...command, idempotencyKey: id("02"), antiForgery, request: assistance,
      }),
    });
    const volumeRequest = {
      command_schema: "storyos.command.create-volume.request.v1" as const,
      create_volume_input: { title, expected_tree_revision: "1", ...BINDING, correlation_id: id("13") },
    };
    const volume = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/volumes",
      schema: volumeRequest.command_schema,
      idempotencyKey: id("03"),
      digest: await digestCreateVolume(volumeRequest),
      send: (antiForgery) => createVolume({
        ...command, idempotencyKey: id("03"), antiForgery, request: volumeRequest,
      }),
    });
    if (volume.effect.kind !== "authoritative_applied") throw new Error("the volume must apply");
    const volumeId = volume.effect.volume_id;
    const chapterRequest = {
      command_schema: "storyos.command.create-chapter.request.v1" as const,
      create_chapter_input: { title, expected_tree_revision: "2", ...BINDING, correlation_id: id("14") },
    };
    const chapter = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
      schema: chapterRequest.command_schema,
      idempotencyKey: id("04"),
      digest: await digestCreateChapter(chapterRequest),
      send: (antiForgery) => createChapter({
        ...command, volumeId, idempotencyKey: id("04"), antiForgery, request: chapterRequest,
      }),
    });
    if (chapter.effect.kind !== "authoritative_applied") throw new Error("the chapter must apply");

    const runRequest = {
      command_schema: "storyos.command.create-agent-run.request.v2" as const,
      create_agent_run_input: {
        conversation: { kind: "new" as const },
        author_message: { text: message },
        working_target: { kind: "current_chapter" as const, chapter_id: chapter.effect.chapter_id },
        instruction: { kind: "absent" as const },
        cause: { kind: "author_request" as const },
        ...BINDING,
        correlation_id: id("15"),
      },
    };
    const run = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/agent-runs",
      schema: runRequest.command_schema,
      idempotencyKey: id("05"),
      digest: await digestCreateAgentRun(runRequest),
      send: (antiForgery) => createAgentRun({
        ...command, idempotencyKey: id("05"), antiForgery, request: runRequest,
      }),
    });
    if (run.effect.kind !== "admitted") throw new Error("the AgentRun must admit");
    const runId = run.effect.run_id;
    const exportRequest = {
      command_schema: "storyos.command.export-human-readable-manuscript.request.v1" as const,
      export_human_readable_manuscript_input: { ...BINDING, correlation_id: id("16") },
    };
    const exported = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/manuscript/exports",
      schema: exportRequest.command_schema,
      idempotencyKey: id("06"),
      digest: await digestExportHumanReadableManuscript(exportRequest),
      send: (antiForgery) => exportHumanReadableManuscript({
        ...command, idempotencyKey: id("06"), antiForgery, request: exportRequest,
      }),
    });
    if (exported.effect.kind !== "admitted") throw new Error("the export must admit");
    const exportId = exported.effect.export_id;

    for (let pass = 0; pass < 16; pass += 1) {
      const settledRun = (await getAgentRun({ ...command, runId })).status === "completed";
      const settledExport = (await getHumanReadableManuscriptExport({ ...command, exportId })).status !== "in_progress";
      if (settledRun && settledExport) break;
      workerStderr += await runStoryOSWorkerStderr({
        repositoryRoot,
        workerBinary,
        args: ["--once"],
        databaseUrl,
        extraEnv: { STORYOS_LOG: "debug" },
      });
    }
    assert.equal((await getAgentRun({ ...command, runId })).status, "completed");
  } finally {
    await stopRealServer(server);
  }
  assertNoCanary(workerStderr + serverStderr(), [title, message, applicationName, databaseUrl]);
  const lines = diagnosticLines(workerStderr);
  assert.deepEqual(lines.filter((line) => !line.target.startsWith("storyos")), []);
  for (const name of ["agent_run_claim", "readable_export_claim", "dispatch"]) {
    const closed = closedSpans(lines, name);
    assert.ok(closed.length > 0, `no ${name} span in ${workerStderr}`);
  }
  for (const claim of [...closedSpans(lines, "agent_run_claim"), ...closedSpans(lines, "readable_export_claim")]) {
    assert.equal(claim.span?.attempt, 1);
    assert.equal(typeof claim.span?.outcome, "string");
  }
  const dispatches = closedSpans(lines, "dispatch");
  assert.ok(dispatches.some((line) => line.span?.request_kind === "create"
    && line.span.observation === "terminal"
    && line.span.adapter === "host_fake"), workerStderr);
  for (const name of ["prepare", "exchange", "commit_dispatch_claim", "record"]) {
    assert.ok(closedSpans(lines, name).length > 0, `no ${name} span in ${workerStderr}`);
  }
});
