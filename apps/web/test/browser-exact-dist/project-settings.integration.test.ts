import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

it("saves the Project Instruction on the settings page and goes back to writing past the unsaved warning", async () => {
  await expect(verifyProductionHost({ scenario: "project_settings" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});
