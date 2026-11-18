import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

it("shows an entry that the assistant wrote in the Memory section of the settings page", async () => {
  await expect(verifyProductionHost({ scenario: "memory_settings_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});
