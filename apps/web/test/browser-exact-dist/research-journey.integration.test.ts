import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client.ts";

it("researches with two governed tool calls, shows the sources and the synthesis, keeps them after a rejected Proposal, and cancels a held search in the panel", async () => {
  await expect(verifyProductionHost({ scenario: "research_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 240_000);
