import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client.ts";

it("shows the three steps of the plan as done above the answer after the Run ends", async () => {
  await expect(verifyProductionHost({ scenario: "plan_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 240_000);
