// Verification: {"phase":"readable-export-process-cut","after":[]}
import assert from "node:assert/strict";
import { test } from "vitest";

import {
  digestExportHumanReadableManuscript,
  exportHumanReadableManuscript,
  getHumanReadableManuscriptExport,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ExportHumanReadableManuscriptRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING } from "../support/acceptance.ts";
import { assertClaimWithoutSettlementTakesOver } from "../support/export-process-cut.ts";
import type { ExportFamily } from "../support/export-process-cut.ts";

const READABLE: ExportFamily<ExportHumanReadableManuscriptRequest> = {
  title: "Readable Export Cut",
  prefix: "af",
  route: "/api/v1/projects/{project_id}/manuscript/exports",
  commandKind: "exportHumanReadableManuscript",
  readyField: "manuscript_utf8",
  request: (correlationId) => ({
    command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
    export_human_readable_manuscript_input: { ...BINDING, correlation_id: correlationId },
  }),
  digest: digestExportHumanReadableManuscript,
  send: exportHumanReadableManuscript,
  read: getHumanReadableManuscriptExport,
  async assertReady(ready) {
    if (ready.status !== "ready" || !("manuscript_utf8" in ready)) {
      throw new Error("Worker takeover must settle the human-readable export");
    }
    assert.equal(ready.manuscript_utf8, "\n");
  },
};

test("a Worker claim without settlement is outcome_unknown, then takeover settles ready", async () => {
  await assertClaimWithoutSettlementTakesOver(READABLE);
});
