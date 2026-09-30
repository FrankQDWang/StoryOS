import { expect, it } from "vitest";
import { verifyProductionHost } from "../support/browser-command-client.ts";

it("edits an exact Inline Proposal in the packaged writing workspace", async () => {
  await expect(verifyProductionHost({ scenario: "inline_proposal" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});
