import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client.ts";

it("selects a Skill, loads and reads another, names both in the panel, refuses an unknown load and a stale chip", async () => {
  await expect(verifyProductionHost({ scenario: "skill_journey" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 240_000);
