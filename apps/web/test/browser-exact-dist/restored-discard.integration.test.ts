import { expect, it } from "vitest";
import { verifyProductionHost } from "../support/browser-command-client.ts";

it("consumes the original production Discard and local Journal after isolated physical restore", async () => {
  await expect(verifyProductionHost({ scenario: "restored_refused_edit" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});
