import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

// The journey settles a capture, a consolidation pass, two intent passes, and two Runs, and it starts the Server two
// times, so its time limit is longer.
it("does each step of the Stage 11 author journey in one Project in order", async () => {
  await expect(verifyProductionHost({ scenario: "stage11_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 300_000);
