// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/diagnostic-canary-worker-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createProjectCommandChallenge,
  createVolume,
  digestCreateVolume,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  assertNoCanary,
  canary,
  closedSpans,
  diagnosticLines,
} from "../support/diagnostic-canary.ts";
import {
  createEmptyProject,
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
  const { baseUrl, server, stderr } = await startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A },
    extraEnv: { STORYOS_LOG: "debug" },
  });
  try {
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const projectId = await createEmptyProject({
      baseUrl, fetchImpl, createKey: id("01"), correlationId: id("11"), title: "Command Canary",
    });
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
      const digest = await digestCreateVolume(request);
      const challenge = await withChallengeBudget(projectId, () => createProjectCommandChallenge({
        baseUrl,
        projectId,
        fetchImpl,
        request: {
          method: "POST",
          route_template: "/api/v1/projects/{project_id}/volumes",
          command_schema: request.command_schema,
          canonical_command_digest: digest,
          idempotency_key: key,
        },
      }));
      const created = () => createVolume({
        baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery: challenge.nonce, request,
      });
      return { first: await created(), retry: created };
    };
    const applied = await send(id("02"), id("12"), "1");
    assert.equal(applied.first.effect.kind, "authoritative_applied");
    assert.deepEqual(await applied.retry(), applied.first);
    const stale = await send(id("03"), id("13"), "1");
    assert.equal(stale.first.effect.kind, "conflicted");
  } finally {
    await stopRealServer(server);
  }
  const output = stderr();
  assertNoCanary(output, [title]);
  const settled = closedSpans(diagnosticLines(output), "settle_project_command")
    .filter((line) => line.span?.command_kind === "createVolume");
  assert.deepEqual(
    settled.map((line) => [line.span?.correlation_id, line.span?.outcome]),
    [[id("12"), "authoritative_applied"], [id("12"), "replayed"], [id("13"), "stale_tree_revision"]],
  );
  for (const line of settled) {
    assert.equal(line.span?.project_id, settled[0]?.span?.project_id);
    assert.equal(typeof line.span?.command_id, "string");
    assert.equal(typeof line.span?.author_command_admission_id, "string");
  }
});
