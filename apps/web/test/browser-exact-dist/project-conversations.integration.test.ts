import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

it("opens an earlier Project Conversation from the list and from the address, and continues it", async () => {
  await expect(verifyProductionHost({ scenario: "project_conversations" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});
