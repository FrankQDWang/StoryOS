import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

it("shows exact captured Memory settings through Run selection, reload, and unavailable evidence", async () => {
  await expect(verifyProductionHost({ scenario: "captured_memory" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});
