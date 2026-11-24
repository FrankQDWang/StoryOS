import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client.ts";

// The journey settles three Runs and starts the Server two times, so its time limit is longer.
it("does each step of the Stage 9 author journey in one Project in order", async () => {
  await expect(verifyProductionHost({ scenario: "stage9_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 300_000);
