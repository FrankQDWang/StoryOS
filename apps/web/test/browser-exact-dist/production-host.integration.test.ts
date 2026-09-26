import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

it("opens, edits, reloads, and takes over through the real production host in Chrome", async () => {
  await expect(verifyProductionHost({ scenario: "open_edit_reload_takeover" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});

it("recovers one prose request through the production Web, Server, database, and Worker", async () => {
  await expect(verifyProductionHost({ scenario: "prose_request" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});

it("preserves and copies a complete mixed edit through response loss, reload and Server restart", async () => {
  await expect(verifyProductionHost({ scenario: "refused_edit" })).resolves.toEqual({ kind: "production_host_verified" });
});
