import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

// The journey waits for the rate of the Chapter capture once, so its time limit is longer.
it("does each step of the Stage 7 author journey in one Project in order", async () => {
  await expect(verifyProductionHost({ scenario: "stage7_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 420_000);
