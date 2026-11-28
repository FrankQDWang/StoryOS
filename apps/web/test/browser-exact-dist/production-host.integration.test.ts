import { expect, it } from "vitest";

import { verifyProductionHost } from "../support/browser-command-client";

it("opens, edits, and reloads through the real production host, and a second page writes with no click", async () => {
  await expect(verifyProductionHost({ scenario: "open_edit_reload_two_pages" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});

it("sets the Chapter of unsent text current again at a new load after another page selected another Chapter", async () => {
  await expect(verifyProductionHost({ scenario: "chapter_switch_load" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});

it("sets the Chapter current again at a new load and stores the text of a save whose Command Challenge did not answer "
  + "before the page closed", async () => {
  await expect(verifyProductionHost({ scenario: "chapter_switch_load_challenge" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});

it("settles a save whose response did not come back before the page closed at a new load, opens the Chapter that "
  + "another page set current, and saves the next input on the applied Revision", async () => {
  await expect(verifyProductionHost({ scenario: "chapter_switch_load_outcome" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});

it("shows the text of a save whose response did not come back on its Chapter, not saved and with no command, at a new "
  + "load whose outcome Query fails after another page set another Chapter current", async () => {
  await expect(verifyProductionHost({ scenario: "chapter_switch_load_outcome_unavailable" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});

it("recovers one prose request through the production Web, Server, database, and Worker", async () => {
  await expect(verifyProductionHost({ scenario: "prose_request" }))
    .resolves.toEqual({ kind: "production_host_verified" });
});

it("replaces only the first owner part of a crossing edit through response loss, reload and Server restart", async () => {
  await expect(verifyProductionHost({ scenario: "first_owner_edit" })).resolves.toEqual({ kind: "production_host_verified" });
});

it("accepts a Proposal on an earlier base through reload and an explicit retry", async () => {
  await expect(verifyProductionHost({ scenario: "conflicted_proposal" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 240_000);

it("revises real cross-Chapter locations through the composer and central editor", async () => {
  await expect(verifyProductionHost({ scenario: "multi_proposal" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 120_000);


it("uses the approved composer and controls the retained Run through the production host", async () => {
  await expect(verifyProductionHost({ scenario: "composer_controls" }))
    .resolves.toEqual({ kind: "production_host_verified" });
}, 120_000);
