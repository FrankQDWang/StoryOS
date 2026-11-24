import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client.ts";

it("shows the card of the ended Subrun and its record in the drawer", async () => {
  await expect(verifyProductionHost({ scenario: "subrun_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 240_000);
