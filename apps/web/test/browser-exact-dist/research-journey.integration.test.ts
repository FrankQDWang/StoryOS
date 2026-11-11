import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client.ts";

it("researches with two governed tool calls, shows the sources and the synthesis, and keeps them after a rejected Proposal", async () => {
  await expect(verifyProductionHost({ scenario: "research_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 240_000);
