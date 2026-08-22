import { expect, it } from "vitest";

import {
  getApplyAuthorEditOutcome,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  GetApplyAuthorEditOutcomeResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";

const PROJECT = "018f0000-0000-7001-8000-000000000002";
const KEY = "018f0000-0000-7001-8000-000000000037";
const NONCE = "a".repeat(64);

it("keeps the generated outcome Query proof header-only", async () => {
  const expected: GetApplyAuthorEditOutcomeResponse = {
    schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000081",
    project_scope: {
      owner_user_id: "018f0000-0000-7001-8000-000000000001",
      project_id: PROJECT,
    },
    outcome: {
      outcome_kind: "still_unknown",
      observation: {
        observation_kind: "admission_committed",
        command_id: "018f0000-0000-7001-8000-000000000031",
        author_command_admission_id: "018f0000-0000-7001-8000-000000000032",
        reconciliation_required: true,
      },
    },
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const parsed = new URL(input instanceof Request ? input.url : input);
    const expectedPath = `/api/v1/projects/${PROJECT}/manuscript/author-edit-outcomes/${KEY}`;
    expect(parsed.pathname).toBe(expectedPath);
    expect(parsed.href).not.toContain(NONCE);
    expect(init?.method).toBe("GET");
    expect(init?.body).toBeUndefined();
    expect(init?.credentials).toBe("same-origin");
    const headers = new Headers(init?.headers);
    expect(headers.get("accept")).toBe("application/json");
    expect(headers.get("x-storyos-anti-forgery")).toBe(NONCE);
    return new Response(JSON.stringify(expected), {
      status: 200,
      headers: { "cache-control": "no-store", "content-type": "application/json" },
    });
  };

  const missingProof = Reflect.apply(getApplyAuthorEditOutcome, undefined, [{
    baseUrl: location.origin,
    projectId: PROJECT,
    idempotencyKey: KEY,
    fetchImpl,
  }]);
  await expect(missingProof).rejects.toBeInstanceOf(TypeError);
  await expect(getApplyAuthorEditOutcome({
    baseUrl: location.origin,
    projectId: PROJECT,
    idempotencyKey: KEY,
    antiForgery: NONCE,
    fetchImpl,
  })).resolves.toEqual(expected);
});
