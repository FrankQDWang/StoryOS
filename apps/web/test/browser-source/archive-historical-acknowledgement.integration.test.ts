import { expect, it } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { archiveOwnedProject } from "../../src/archive-project.ts";
import { historicalAcknowledgementUnavailable } from "../../src/historical-acknowledgement.ts";
import { jsonResponse } from "./scenario.ts";

const OWNER = "018f0000-0000-7001-8000-000000000001";
const PROJECT = "018f0000-0000-7001-8000-000000000012";

it("explains a historical Archive Project acknowledgement and does not retry it", async () => {
  const methods: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    methods.push(`${method} ${path}`);
    if (path === "/api/v1/protocol") return jsonResponse(RELEASE_1_PROTOCOL_PROFILE);
    if (path === "/api/v1/projects") {
      return jsonResponse({
        schema_id: "storyos.query.project-list.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000013",
        owner_user_id: OWNER,
        projects: [{
          project_scope: { owner_user_id: OWNER, project_id: PROJECT },
          title: "Listed Novel",
          lifecycle: { kind: "active" },
          revision: "2",
          open: { kind: "empty" },
        }],
      });
    }
    if (path === `/api/v1/projects/${PROJECT}/anti-forgery-challenges`) {
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-09-12T00:00:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path === `/api/v1/projects/${PROJECT}/archival` && method === "PUT") {
      return jsonResponse({
        schema_id: "storyos.problem.v1",
        code: "historical_acknowledgement_unavailable",
        message: "The original Archive Project acknowledgement cannot be recovered.",
      }, 409);
    }
    throw new Error(`unexpected request: ${method} ${path}`);
  };

  document.body.innerHTML = '<main id="app"></main>';
  try {
    const unavailable = await archiveOwnedProject({ baseUrl: location.origin, fetchImpl, cryptoImpl: crypto,
      projectId: PROJECT, expectedProjectRevision: "2" }).catch(historicalAcknowledgementUnavailable);
    expect(unavailable).toBe(true);
    expect(methods.filter((entry) => entry.startsWith("PUT ") && entry.endsWith("/archival"))).toEqual([
      `PUT /api/v1/projects/${PROJECT}/archival`,
    ]);
    expect(methods.filter((entry) => entry.includes("anti-forgery-challenges"))).toHaveLength(1);
  } finally {
    document.body.replaceChildren();
  }
});
