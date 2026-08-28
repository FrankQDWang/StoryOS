import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

it("opens, edits, reloads, and takes over through the real production host in Chrome", async () => {
  await expect(verifyProductionHost({ scenario: "open_edit_reload_takeover" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});
