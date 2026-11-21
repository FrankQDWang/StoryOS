import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client.ts";

it("selects a packaged Skill, marks two candidates, pairs a mark, decides each candidate, and gets a text answer", async () => {
  await expect(verifyProductionHost({ scenario: "author_voice_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 240_000);
