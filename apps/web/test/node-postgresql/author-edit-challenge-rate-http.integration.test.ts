// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/list-open-proposals-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";
import { createProjectCommandChallenge, StoryOSProtocolError,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { requireStoryOSProtocolError, stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { id, prepare, startRealServer } from "../support/acceptance.ts";

const AUTHOR_EDIT_CAPACITY = 120;

// The new Project gives this test its own Author Edit rate window, so other tests cannot use its units.
test("the real Server refuses Author Edit Challenge requests with HTTP 429 and Retry-After after the class capacity", async () => {
  const started = await startRealServer();
  try {
    const prepared = await prepare(started.baseUrl, id("b7a00001"), "Author Edit Rate Novel", "b7b0000");
    const challenge = (index: number) => createProjectCommandChallenge({
      baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits",
        command_schema: "storyos.command.apply-author-edit.request.v1",
        canonical_command_digest: { algorithm: "sha256", profile: "storyos.command.applyAuthorEdit.jcs.v1",
          value_hex_lowercase: (index % 16).toString(16).repeat(64) },
        idempotency_key: id(`b7c${index.toString(16).padStart(5, "0")}`),
      },
    });
    // A fixed window can admit two windows of capacity near a window boundary.
    let issued = 0;
    let refusal: StoryOSProtocolError | undefined;
    while (refusal === undefined && issued <= 2 * AUTHOR_EDIT_CAPACITY) {
      try {
        await challenge(issued);
        issued += 1;
      } catch (error) {
        refusal = requireStoryOSProtocolError(error);
      }
    }
    assert.ok(refusal !== undefined, `no refusal after ${issued} Author Edit Challenge requests`);
    assert.ok(issued >= AUTHOR_EDIT_CAPACITY && issued <= 2 * AUTHOR_EDIT_CAPACITY, `issued ${issued}`);
    assert.deepEqual({
      status: refusal.status,
      retryAfter: /^(?:[1-9]|[1-5][0-9]|60)$/.test(String(refusal.retryAfterSeconds)),
      body: JSON.parse(refusal.responseBody ?? "{}"),
    }, {
      status: 429,
      retryAfter: true,
      body: { schema_id: "storyos.problem.v1", code: "challenge_rate_limited",
        message: "The command challenge rate limit is exceeded." },
    });

    // The shared class has its own counter (ADR 0038).
    const shared = await createProjectCommandChallenge({
      baseUrl: started.baseUrl, projectId: prepared.projectId, fetchImpl: prepared.fetchImpl,
      request: {
        method: "PATCH",
        route_template: "/api/v1/projects/{project_id}",
        command_schema: "storyos.command.update-project.request.v1",
        canonical_command_digest: { algorithm: "sha256", profile: "storyos.command.updateProject.jcs.v1",
          value_hex_lowercase: "a".repeat(64) },
        idempotency_key: id("b7d00001"),
      },
    });
    assert.match(shared.nonce, /^[0-9a-f]{64}$/);
  } finally {
    await stopRealServer(started.server);
  }
});
