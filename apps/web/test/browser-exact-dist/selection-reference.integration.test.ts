import { expect, it } from "vitest";
import { verifyProductionHost } from "../support/browser-command-client.ts";

it("adds an exact Chapter selection to a message and to guidance in the packaged writing workspace", async () => {
  await expect(verifyProductionHost({ scenario: "selection_reference" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});
