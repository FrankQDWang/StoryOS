import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

it("shows bounded selected Run input and recovery evidence through the packaged production host", async () => {
  await expect(verifyProductionHost({ scenario: "run_evidence" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 240_000);
