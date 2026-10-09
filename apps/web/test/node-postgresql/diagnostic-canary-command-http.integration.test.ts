// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/diagnostic-canary-worker-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createAgentRun,
  createProjectCommandChallenge,
  createVolume,
  digestCreateAgentRun,
  digestCreateVolume,
  digestExportHumanReadableManuscript,
  exportHumanReadableManuscript,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DigestValue } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  assertNoCanary,
  canary,
  closedSpans,
  diagnosticLines,
} from "../support/diagnostic-canary.ts";
import {
  createEmptyProject,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeBudget,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const exe = process.platform === "win32" ? ".exe" : "";
const serverBinary = join(repositoryRoot, "target", "release-package", `storyos-server${exe}`);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};

function id(suffix: string): string {
  return `018f0000-0000-7001-8000-0000000a49${suffix}`;
}

test("each command settlement writes one span with its identifiers and outcome", async () => {
  const title = canary("volume-title");
  const message = canary("author-message");
  const { baseUrl, server, stderr } = await startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A },
    extraEnv: { STORYOS_LOG: "debug" },
  });
  let volumeIds: string[] = [];
  let exportIds: string[] = [];
  try {
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const projectId = await createEmptyProject({
      baseUrl, fetchImpl, createKey: id("01"), correlationId: id("11"), title: "Command Canary",
    });
    const challenge = async (route: string, schema: string, digest: DigestValue, key: string) => (
      await withChallengeBudget(projectId, () => createProjectCommandChallenge({
        baseUrl,
        projectId,
        fetchImpl,
        request: {
          method: "POST",
          route_template: route,
          command_schema: schema,
          canonical_command_digest: digest,
          idempotency_key: key,
        },
      }))
    ).nonce;
    const send = async (key: string, correlationId: string, expectedTreeRevision: string) => {
      const request = {
        command_schema: "storyos.command.create-volume.request.v1" as const,
        create_volume_input: {
          title,
          expected_tree_revision: expectedTreeRevision,
          ...BINDING,
          correlation_id: correlationId,
        },
      };
      const nonce = await challenge(
        "/api/v1/projects/{project_id}/volumes", request.command_schema, await digestCreateVolume(request), key,
      );
      const created = () => createVolume({
        baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery: nonce, request,
      });
      return { first: await created(), retry: created };
    };
    const applied = await send(id("02"), id("12"), "1");
    assert.equal(applied.first.effect.kind, "authoritative_applied");
    assert.deepEqual(await applied.retry(), applied.first);
    const stale = await send(id("03"), id("13"), "1");
    assert.equal(stale.first.effect.kind, "conflicted");
    volumeIds = [applied.first.command_id, applied.first.command_id, stale.first.command_id];

    const exportRequest = {
      command_schema: "storyos.command.export-human-readable-manuscript.request.v1" as const,
      export_human_readable_manuscript_input: { ...BINDING, correlation_id: id("14") },
    };
    const exportNonce = await challenge(
      "/api/v1/projects/{project_id}/manuscript/exports",
      exportRequest.command_schema,
      await digestExportHumanReadableManuscript(exportRequest),
      id("04"),
    );
    const admit = () => exportHumanReadableManuscript({
      baseUrl, projectId, fetchImpl, idempotencyKey: id("04"), antiForgery: exportNonce, request: exportRequest,
    });
    const admitted = await admit();
    assert.equal((await admit()).command_id, admitted.command_id);
    exportIds = [admitted.command_id, admitted.command_id];

    const runRequest = {
      command_schema: "storyos.command.create-agent-run.request.v2" as const,
      create_agent_run_input: {
        conversation: { kind: "new" as const },
        author_message: { text: message },
        working_target: { kind: "current_chapter" as const, chapter_id: id("21") },
        instruction: { kind: "absent" as const },
        cause: { kind: "author_request" as const },
        ...BINDING,
        correlation_id: id("15"),
      },
    };
    const runNonce = await challenge(
      "/api/v1/projects/{project_id}/agent-runs", runRequest.command_schema, await digestCreateAgentRun(runRequest), id("05"),
    );
    await assert.rejects(
      createAgentRun({ baseUrl, projectId, fetchImpl, idempotencyKey: id("05"), antiForgery: runNonce, request: runRequest }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );
  } finally {
    await stopRealServer(server);
  }
  const output = stderr();
  assertNoCanary(output, [title, message]);
  const lines = diagnosticLines(output);
  const settled = closedSpans(lines, "settle_project_command")
    .filter((line) => line.span?.command_kind === "createVolume");
  assert.deepEqual(
    settled.map((line) => [line.span?.correlation_id, line.span?.outcome, line.span?.command_id]),
    [
      [id("12"), "authoritative_applied", volumeIds[0]],
      [id("12"), "replayed", volumeIds[1]],
      [id("13"), "stale_tree_revision", volumeIds[2]],
    ],
  );
  const admissions = closedSpans(lines, "admit_project_command")
    .filter((line) => line.span?.correlation_id === id("14"));
  assert.deepEqual(
    admissions.map((line) => [line.span?.outcome, line.span?.command_id]),
    [["admitted", exportIds[0]], ["replayed", exportIds[1]]],
  );
  const refused = closedSpans(lines, "settle_project_command")
    .filter((line) => line.span?.correlation_id === id("15"));
  assert.deepEqual(refused.map((line) => line.span?.outcome), ["assistance_unavailable"]);
  for (const line of [...settled, ...admissions]) {
    assert.equal(typeof line.span?.author_command_admission_id, "string");
  }
});
