// Verification: {"phase":"project-export-process-cut","after":[]}
import assert from "node:assert/strict";
import { test } from "vitest";

import {
  digestExportProjectArchive,
  exportProjectArchive,
  getExportOperation,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ExportProjectArchiveRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING } from "../support/acceptance.ts";
import { assertAdmissionSurvivesServerCut, assertClaimWithoutSettlementTakesOver } from "../support/export-process-cut.ts";
import type { ExportFamily } from "../support/export-process-cut.ts";
import { sessionFetch } from "../support/node-integration.ts";

const ARCHIVE_MEDIA =
  'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"';

const ARCHIVE: ExportFamily<ExportProjectArchiveRequest> = {
  title: "Archive Export Cut",
  prefix: "e0",
  route: "/api/v1/projects/{project_id}/exports",
  commandKind: "exportProjectArchive",
  readyField: "immutable_root",
  request: (correlationId) => ({
    command_schema: "storyos.command.export-project-archive.request.v1",
    export_project_archive_input: {
      ...BINDING,
      correlation_id: correlationId,
      archive_profile: "storyos.project-export.v1",
      archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
    },
  }),
  digest: digestExportProjectArchive,
  send: exportProjectArchive,
  read: getExportOperation,
  async assertReady(ready, baseUrl, projectId, exportId) {
    if (ready.status !== "ready" || !("immutable_root" in ready)) {
      throw new Error("Worker takeover must settle the Project Export Archive");
    }
    assert.match(String(ready.immutable_root), /^sha256:[0-9a-f]{64}$/);
    const exportUrl = `${baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/exports/${encodeURIComponent(exportId)}`;
    const zipResponse = await sessionFetch(baseUrl, "session-a")(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(zipResponse.status, 200);
    assert.deepEqual(
      Array.from(new Uint8Array(await zipResponse.arrayBuffer()).slice(0, 4)),
      [0x50, 0x4b, 0x03, 0x04],
    );
  },
};

test("an admitted Project Export Archive stays in progress across a Server process cut", async () => {
  await assertAdmissionSurvivesServerCut(ARCHIVE);
});

test("a Worker claim without settlement is outcome_unknown, then takeover settles ready", async () => {
  await assertClaimWithoutSettlementTakesOver(ARCHIVE);
});
