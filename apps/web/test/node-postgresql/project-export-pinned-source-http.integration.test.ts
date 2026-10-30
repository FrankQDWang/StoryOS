// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/takeover-late-result-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { test } from "vitest";

import {
  archiveProject,
  createVolume,
  digestArchiveProject,
  digestCreateVolume,
  digestExportHumanReadableManuscript,
  digestExportProjectArchive,
  digestUpdateProject,
  exportHumanReadableManuscript,
  exportProjectArchive,
  getExportOperation,
  getHumanReadableManuscriptExport,
  updateProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  ExportHumanReadableManuscriptRequest,
  ExportProjectArchiveRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  createEmptyProject,
  exportSettlementReceipt,
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";

import { zipStoreFiles } from "../support/archive.ts";
import { BINDING, USER_A, UUID_V7, challenged, createEmpty, issueChallenge, settleOnce, startServer, waitFor } from "../support/http-harness.ts";

const ARCHIVE_MEDIA =
  'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"';
const ADMITTED_TITLE = "Pinned Archive Novel";
const HISTORICAL_RECEIPT = "018f0000-0000-7001-8000-00000000c301";
const HISTORICAL_EVENT = "018f0000-0000-7001-8000-00000000c302";
const HISTORICAL_ROOT = `sha256:${"c".repeat(64)}`;

function exportRequest(
  correlationId: string,
  archiveProfile = "storyos.project-export.v1",
): ExportProjectArchiveRequest {
  return {
    command_schema: "storyos.command.export-project-archive.request.v1",
    export_project_archive_input: {
      ...BINDING,
      correlation_id: correlationId,
      archive_profile: archiveProfile,
      archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
    },
  };
}

async function admitArchive(
  command: { baseUrl: string; fetchImpl: typeof fetch; projectId: string },
  exportKey: string,
  correlationId: string,
): Promise<string> {
  const request = exportRequest(correlationId);
  const applied = await challenged(
    command.baseUrl, command.fetchImpl, command.projectId, "POST", "/api/v1/projects/{project_id}/exports",
    request.command_schema, await digestExportProjectArchive(request), exportKey,
    (antiForgery) => exportProjectArchive({
      ...command, idempotencyKey: exportKey, antiForgery, request,
    }),
  );
  if (applied.effect.kind !== "admitted") {
    throw new Error("Project Export Archive must admit");
  }
  return applied.effect.export_id;
}

async function admitHumanReadableExport(
  command: { baseUrl: string; fetchImpl: typeof fetch; projectId: string },
  exportKey: string,
  correlationId: string,
): Promise<string> {
  const request: ExportHumanReadableManuscriptRequest = {
    command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
    export_human_readable_manuscript_input: { ...BINDING, correlation_id: correlationId },
  };
  const applied = await challenged(
    command.baseUrl, command.fetchImpl, command.projectId, "POST", "/api/v1/projects/{project_id}/manuscript/exports",
    request.command_schema, await digestExportHumanReadableManuscript(request), exportKey,
    (antiForgery) => exportHumanReadableManuscript({
      ...command, idempotencyKey: exportKey, antiForgery, request,
    }),
  );
  if (applied.effect.kind !== "admitted") {
    throw new Error("Human-readable export must admit");
  }
  return applied.effect.export_id;
}

function exportUrlFor(baseUrl: string, projectId: string, exportId: string): string {
  return `${baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/exports/${encodeURIComponent(exportId)}`;
}

test("an admitted Project Export Archive settles the pinned families after later live changes", async () => {
  const { baseUrl, server } = await startServer();
  try {
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const projectId = await createEmptyProject({
      baseUrl,
      fetchImpl,
      createKey: "018f0000-0000-7001-8000-00000000e101",
      correlationId: "018f0000-0000-7001-8000-00000000e111",
      title: ADMITTED_TITLE,
    });
    const command = { baseUrl, fetchImpl, projectId };

    const request = exportRequest("018f0000-0000-7001-8000-00000000e114");
    const exportKey = "018f0000-0000-7001-8000-00000000e104";
    let nonce = "";
    const applied = await challenged(
      baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/exports", request.command_schema,
      await digestExportProjectArchive(request), exportKey,
      (antiForgery) => exportProjectArchive({
        ...command, idempotencyKey: exportKey, antiForgery: (nonce = antiForgery), request,
      }),
    );
    assert.equal(applied.acknowledgement, "accepted");
    if (applied.effect.kind !== "admitted") {
      throw new Error("Project Export Archive must admit");
    }
    const exportId = applied.effect.export_id;
    const sourceSnapshotId = applied.effect.source_snapshot.snapshot_id;
    const exportUrl = exportUrlFor(baseUrl, projectId, exportId);

    const waiting = await getExportOperation({ baseUrl, projectId, exportId, fetchImpl });
    assert.equal(waiting.status, "in_progress");
    assert.equal("immutable_root" in waiting, false);
    assert.equal(waiting.source_snapshot.snapshot_id, sourceSnapshotId);
    const refusedZip = await fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(refusedZip.status, 422);

    const replay = await exportProjectArchive({
      ...command, idempotencyKey: exportKey, antiForgery: nonce, request,
    });
    assert.equal(replay.command_id, applied.command_id);
    if (replay.effect.kind !== "admitted") {
      throw new Error("retry must return the same admitted operation");
    }
    assert.equal(replay.effect.export_id, exportId);

    const renameProject = {
      command_schema: "storyos.command.update-project.request.v1" as const,
      update_project_input: {
        title: "Later Archive Title",
        expected_project_revision: "1",
        ...BINDING,
        correlation_id: "018f0000-0000-7001-8000-00000000e115",
      },
    };
    const renamed = await challenged(
      baseUrl, fetchImpl, projectId, "PATCH", "/api/v1/projects/{project_id}", renameProject.command_schema,
      await digestUpdateProject(renameProject), "018f0000-0000-7001-8000-00000000e105",
      (antiForgery) => updateProject({
        ...command, idempotencyKey: "018f0000-0000-7001-8000-00000000e105", antiForgery,
        request: renameProject,
      }),
    );
    assert.equal(renamed.effect.kind, "authoritative_applied");

    const laterVolumeRequest = {
      command_schema: "storyos.command.create-volume.request.v1" as const,
      create_volume_input: {
        title: "Volume B",
        expected_tree_revision: "1",
        ...BINDING,
        correlation_id: "018f0000-0000-7001-8000-00000000e11a",
      },
    };
    const laterVolume = await challenged(
      baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/volumes", laterVolumeRequest.command_schema,
      await digestCreateVolume(laterVolumeRequest), "018f0000-0000-7001-8000-00000000e10a",
      (antiForgery) => createVolume({
        ...command, idempotencyKey: "018f0000-0000-7001-8000-00000000e10a", antiForgery,
        request: laterVolumeRequest,
      }),
    );
    assert.equal(laterVolume.effect.kind, "authoritative_applied");

    await settleOnce();
    const ready = await getExportOperation({ baseUrl, projectId, exportId, fetchImpl });
    assert.equal(ready.status, "ready");
    if (ready.status !== "ready") {
      throw new Error("the Worker must settle the pinned Project Export Archive");
    }
    assert.equal(ready.source_snapshot.snapshot_id, sourceSnapshotId);
    assert.equal(ready.export_id, exportId);

    const zipResponse = await fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(zipResponse.status, 200);
    const zipBytes = new Uint8Array(await zipResponse.arrayBuffer());
    const zipFiles = zipStoreFiles(zipBytes);
    const projects = JSON.parse(new TextDecoder().decode(zipFiles.get("canonical/projects.json")));
    assert.deepEqual(projects.map((row: { title: string }) => row.title), [ADMITTED_TITLE]);
    const objects = JSON.parse(
      new TextDecoder().decode(zipFiles.get("canonical/manuscript_objects.json")),
    );
    assert.equal(JSON.stringify(objects).includes("Volume B"), false);
  } finally {
    await stopRealServer(server);
  }
});

test("an Archive whose Pinned Export Source is missing settles failed without live fallback", async () => {
  const { baseUrl, server } = await startServer();
  try {
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const projectId = await createEmptyProject({
      baseUrl,
      fetchImpl,
      createKey: "018f0000-0000-7001-8000-00000000e201",
      correlationId: "018f0000-0000-7001-8000-00000000e211",
      title: "Fail Closed Archive",
    });
    const command = { baseUrl, fetchImpl, projectId };
    const exportId = await admitArchive(
      command, "018f0000-0000-7001-8000-00000000e204", "018f0000-0000-7001-8000-00000000e214",
    );
    const exportUrl = exportUrlFor(baseUrl, projectId, exportId);

    // An operation admitted before this source existed has no source row.
    const removed = await queryPostgres(`
      DELETE FROM storyos.pinned_export_sources
       WHERE owner_user_id = '${USER_A}'::uuid
         AND project_id = '${projectId}'::uuid
         AND export_id = '${exportId}'::uuid;
      SELECT 'ok';
    `);
    assert.match(removed, /ok$/);
    const waiting = await getExportOperation({ baseUrl, projectId, exportId, fetchImpl });
    assert.equal(waiting.status, "in_progress");

    await settleOnce();
    const failed = await getExportOperation({ baseUrl, projectId, exportId, fetchImpl });
    assert.equal(failed.status, "failed");
    assert.equal("immutable_root" in failed, false);
    assert.equal(
      await exportSettlementReceipt({
        ownerUserId: USER_A,
        projectId,
        exportId,
        operationsTable: "project_export_operations",
      }),
      "refused pinned_export_source_unavailable",
    );
    const refusedZip = await fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(refusedZip.status, 422);
    const entryRows = await queryPostgres(`
      SELECT count(*)::text
        FROM storyos.project_export_entries
       WHERE owner_user_id = '${USER_A}'::uuid
         AND project_id = '${projectId}'::uuid;
    `);
    assert.equal(entryRows, "0");
  } finally {
    await stopRealServer(server);
  }
});

test("a later Archive packs other in-progress Pinned Export Sources and not its own", async () => {
  const { baseUrl, server } = await startServer();
  try {
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const projectId = await createEmptyProject({
      baseUrl,
      fetchImpl,
      createKey: "018f0000-0000-7001-8000-00000000e301",
      correlationId: "018f0000-0000-7001-8000-00000000e311",
      title: "Nested Source Archive",
    });
    const command = { baseUrl, fetchImpl, projectId };
    const readableExportId = await admitHumanReadableExport(
      command, "018f0000-0000-7001-8000-00000000e303", "018f0000-0000-7001-8000-00000000e313",
    );
    const archiveExportId = await admitArchive(
      command, "018f0000-0000-7001-8000-00000000e304", "018f0000-0000-7001-8000-00000000e314",
    );

    // Claim order is readable-first, so two runs settle both operations.
    await settleOnce();
    await settleOnce();
    const readable = await getHumanReadableManuscriptExport({
      baseUrl, projectId, exportId: readableExportId, fetchImpl,
    });
    assert.equal(readable.status, "ready");
    const archive = await getExportOperation({
      baseUrl, projectId, exportId: archiveExportId, fetchImpl,
    });
    assert.equal(archive.status, "ready");

    const zipResponse = await fetchImpl(exportUrlFor(baseUrl, projectId, archiveExportId), {
      headers: { Accept: ARCHIVE_MEDIA },
    });
    assert.equal(zipResponse.status, 200);
    const zipFiles = zipStoreFiles(new Uint8Array(await zipResponse.arrayBuffer()));
    const sources: Array<{ export_id: string; completeness_profile: string }> = JSON.parse(
      new TextDecoder().decode(zipFiles.get("canonical/pinned_export_sources.json")),
    );
    assert.deepEqual(
      sources.map((row) => [row.export_id, row.completeness_profile]),
      [[readableExportId, "human_readable_manuscript"]],
    );
  } finally {
    await stopRealServer(server);
  }
});

function archiveRequest(expectedProjectRevision: string, correlationId: string): ArchiveProjectRequest {
  return {
    command_schema: "storyos.command.archive-project.request.v1",
    archive_project_input: {
      expected_project_revision: expectedProjectRevision,
      ...BINDING,
      correlation_id: correlationId,
    },
  };
}

async function postExport(
  baseUrl: string,
  fetchImpl: ReturnType<typeof browserFetch>,
  projectId: string,
  idempotencyKey: string,
  request: ExportProjectArchiveRequest,
) {
  let nonce = "";
  const admitted = await challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/exports",
    request.command_schema, await digestExportProjectArchive(request), idempotencyKey,
    (antiForgery) => exportProjectArchive({
      baseUrl,
      projectId,
      fetchImpl,
      idempotencyKey,
      antiForgery: (nonce = antiForgery),
      request,
    }));
  return { nonce, admitted };
}

async function archiveOpenProject(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  expectedProjectRevision: string,
  correlationId: string,
  idempotencyKey: string,
) {
  const request = archiveRequest(expectedProjectRevision, correlationId);
  return challenged(baseUrl, fetchImpl, projectId, "PUT", "/api/v1/projects/{project_id}/archival",
    request.command_schema, await digestArchiveProject(request), idempotencyKey,
    (antiForgery) => archiveProject({
      baseUrl,
      projectId,
      fetchImpl,
      idempotencyKey,
      antiForgery,
      request,
    }));
}

async function insertHistoricalReady(options: {
  ownerUserId: string;
  projectId: string;
  exportId: string;
  admissionId: string;
  snapshotId: string;
  sourceActivityPosition: string;
}): Promise<void> {
  const result = await queryPostgres(`
    BEGIN;
    INSERT INTO storyos.domain_receipts
      (owner_user_id, project_id, receipt_id, author_command_admission_id,
       command_id, command_kind, command_digest, idempotency_key, producer_cause,
       expected_heads, prior_heads, resulting_heads, authoritative_revision_ids,
       proposal_revision_ids, authoritative_commit_ids, draft_artifact_refs,
       artifact_lifecycle_event_refs, condition_refs, result_kind, result_payload)
    SELECT admission.owner_user_id, admission.project_id, '${HISTORICAL_RECEIPT}'::uuid,
           admission.author_command_admission_id, admission.command_id,
           'exportProjectArchive', admission.canonical_command_digest,
           admission.idempotency_key, 'author_command_admission',
           '{}'::uuid[], '{}'::uuid[], '{}'::uuid[], '{}'::uuid[], '{}'::uuid[],
           '{}'::uuid[], '{}'::text[], '{}'::text[], '{}'::text[],
           'authoritative_applied', '{}'::jsonb
      FROM storyos.author_command_admissions AS admission
     WHERE admission.owner_user_id = '${options.ownerUserId}'::uuid
       AND admission.project_id = '${options.projectId}'::uuid
       AND admission.author_command_admission_id = '${options.admissionId}'::uuid;
    INSERT INTO storyos.author_command_admission_settlements
      (owner_user_id, project_id, author_command_admission_id, settlement_kind, receipt_id)
    VALUES (
      '${options.ownerUserId}'::uuid, '${options.projectId}'::uuid,
      '${options.admissionId}'::uuid, 'receipt_settled', '${HISTORICAL_RECEIPT}'::uuid
    );
    INSERT INTO storyos.scope_counters AS counters
      (owner_user_id, project_id, project_activity_position)
    VALUES ('${options.ownerUserId}'::uuid, '${options.projectId}'::uuid, 1)
    ON CONFLICT (owner_user_id, project_id)
    DO UPDATE SET
      project_activity_position = counters.project_activity_position + 1;
    INSERT INTO storyos.project_activity_event_payloads
      (owner_user_id, project_id, project_activity_position, project_activity_event_id,
       event_kind, receipt_id, receipt_result_kind, payload)
    SELECT counters.owner_user_id, counters.project_id, counters.project_activity_position,
           '${HISTORICAL_EVENT}'::uuid, 'project_export_settled',
           '${HISTORICAL_RECEIPT}'::uuid, 'authoritative_applied',
           jsonb_build_object(
             'kind', 'project_export_settled',
             'export_id', '${options.exportId}',
             'archive_profile', 'storyos.project-export.v1',
             'archive_path_profile', 'storyos.archive-path.utf8-nfc-unicode-16.0.0.v1'
           )
      FROM storyos.scope_counters AS counters
     WHERE counters.owner_user_id = '${options.ownerUserId}'::uuid
       AND counters.project_id = '${options.projectId}'::uuid;
    INSERT INTO storyos.project_export_manifests
      (owner_user_id, project_id, export_id, receipt_id, source_snapshot_id,
       source_activity_position, archive_profile, archive_path_profile, immutable_root)
    VALUES (
      '${options.ownerUserId}'::uuid, '${options.projectId}'::uuid,
      '${options.exportId}'::uuid, '${HISTORICAL_RECEIPT}'::uuid, '${options.snapshotId}',
      ${options.sourceActivityPosition}::bigint, 'storyos.project-export.v1',
      'storyos.archive-path.utf8-nfc-unicode-16.0.0.v1', '${HISTORICAL_ROOT}'
    );
    DELETE FROM storyos.project_export_operations
     WHERE owner_user_id = '${options.ownerUserId}'::uuid
       AND project_id = '${options.projectId}'::uuid
       AND export_id = '${options.exportId}'::uuid;
    UPDATE storyos.command_idempotency AS idempotency
       SET outcome_kind = 'settled', result_reference = '${HISTORICAL_RECEIPT}'
      FROM storyos.author_command_admissions AS admission
     WHERE idempotency.owner_user_id = admission.owner_user_id
       AND idempotency.project_id = admission.project_id
       AND idempotency.command_kind = admission.command_kind
       AND idempotency.idempotency_key = admission.idempotency_key
       AND admission.author_command_admission_id = '${options.admissionId}'::uuid;
    COMMIT;
    SELECT 'ok';
  `);
  assert.match(result, /ok$/);
}

async function waitForExportStatus(
  baseUrl: string,
  projectId: string,
  exportId: string,
  fetchImpl: typeof fetch,
  status: "ready" | "failed" | "outcome_unknown",
) {
  return waitFor(() => getExportOperation({
    baseUrl,
    projectId,
    exportId,
    fetchImpl,
  }), (page) => page.status === status);
}

test("exportProjectArchive admits one inspectable in-progress operation", async () => {
  const { baseUrl, server } = await startServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-00001309c001", "Empty Novel",
      "018f0000-0000-7001-8000-000000000c10");
    const request = exportRequest("018f0000-0000-7001-8000-000000000c11");
    const applied = await postExport(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000c21",
      request,
    ).catch((error) => {
      const protocol = requireStoryOSProtocolError(error);
      throw new Error(`${protocol.status} ${protocol.responseBody}`);
    });
    assert.equal(applied.admitted.schema_id, "storyos.command.export-project-archive.response.v1");
    assert.equal(applied.admitted.acknowledgement, "accepted");
    assert.equal("receipt" in applied.admitted, false);
    assert.equal(applied.admitted.effect.kind, "admitted");
    if (applied.admitted.effect.kind !== "admitted") {
      throw new Error("Project Export must admit");
    }
    assert.match(applied.admitted.effect.export_id, UUID_V7);
    assert.equal(applied.admitted.effect.archive_profile, "storyos.project-export.v1");
    assert.equal(
      applied.admitted.effect.archive_path_profile,
      "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
    );
    assert.match(applied.admitted.effect.source_snapshot.snapshot_id, UUID_V7);
    assert.equal(applied.admitted.operation_ref?.kind, "project_export");

    const inspected = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(inspected.status, "in_progress");
    if (inspected.status !== "in_progress") {
      throw new Error("a new Project Export Archive must stay in progress");
    }
    assert.equal("immutable_root" in inspected, false);
    assert.equal(inspected.export_id, applied.admitted.effect.export_id);
    assert.equal(inspected.archive_profile, "storyos.project-export.v1");
    assert.equal(
      inspected.archive_path_profile,
      "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
    );
    assert.equal(
      inspected.source_snapshot.snapshot_id,
      applied.admitted.effect.source_snapshot.snapshot_id,
    );

    const exportUrl = `${baseUrl}/api/v1/projects/${encodeURIComponent(first.projectId)}/exports/${encodeURIComponent(applied.admitted.effect.export_id)}`;
    const zipResponse = await first.fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(zipResponse.status, 422);
    assert.match(await zipResponse.text(), /invalid_provenance/);

    const replay = await exportProjectArchive({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000c21",
      antiForgery: applied.nonce,
      request,
    });
    assert.equal(replay.command_id, applied.admitted.command_id);
    assert.equal(replay.author_command_admission_id, applied.admitted.author_command_admission_id);
    assert.equal("receipt" in replay, false);
    if (replay.effect.kind !== "admitted" || applied.admitted.effect.kind !== "admitted") {
      throw new Error("retry must return the same admitted operation");
    }
    assert.equal(replay.effect.export_id, applied.admitted.effect.export_id);

    await assert.rejects(
      getExportOperation({
        baseUrl,
        projectId: first.projectId,
        exportId: "018f0000-0000-7001-8000-000000000c99",
        fetchImpl: first.fetchImpl,
      }),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );

    const foreign = await createEmpty(baseUrl, "session-b", "018f0000-0000-7001-8000-00001309c002", "Other Novel",
      "018f0000-0000-7001-8000-000000000c10");
    await assert.rejects(
      getExportOperation({
        baseUrl,
        projectId: first.projectId,
        exportId: applied.admitted.effect.export_id,
        fetchImpl: foreign.fetchImpl,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
    const foreignZip = await foreign.fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(foreignZip.status, 404);
    assert.equal(String(await foreignZip.text()).includes(USER_A), false);
    await assert.rejects(
      postExport(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000c23",
        exportRequest("018f0000-0000-7001-8000-000000000c13"),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );

    const stillVisible = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(stillVisible.status, "in_progress");
    await assert.rejects(
      postExport(
        baseUrl,
        first.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-000000000c24",
        exportRequest("018f0000-0000-7001-8000-000000000c14", "storyos.project-export.other.v1"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 400,
    );

    const historical = await postExport(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000c28",
      exportRequest("018f0000-0000-7001-8000-000000000c18"),
    );
    if (historical.admitted.effect.kind !== "admitted") {
      throw new Error("the historical export must first admit");
    }
    await insertHistoricalReady({
      ownerUserId: USER_A,
      projectId: first.projectId,
      exportId: historical.admitted.effect.export_id,
      admissionId: historical.admitted.author_command_admission_id,
      snapshotId: historical.admitted.effect.source_snapshot.snapshot_id,
      sourceActivityPosition: historical.admitted.effect.source_snapshot.project_activity_position,
    });
    const ready = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: historical.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(ready.status, "ready");
    if (ready.status !== "ready") {
      throw new Error("a historical Project Export Archive must stay ready");
    }
    assert.equal(ready.immutable_root, HISTORICAL_ROOT);
    assert.equal(ready.export_id, historical.admitted.effect.export_id);

    const archived = await archiveOpenProject(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "1",
      "018f0000-0000-7001-8000-000000000c16",
      "018f0000-0000-7001-8000-000000000c26",
    );
    assert.equal(archived.effect.kind, "authoritative_applied");

    const refusedRequest = exportRequest("018f0000-0000-7001-8000-000000000c17");
    const refusedDigest = await digestExportProjectArchive(refusedRequest);
    const refusedChallenge = await issueChallenge(baseUrl, first.fetchImpl, first.projectId, "POST",
      "/api/v1/projects/{project_id}/exports", "storyos.command.export-project-archive.request.v1", refusedDigest,
      "018f0000-0000-7001-8000-000000000c27");
    await assert.rejects(
      exportProjectArchive({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000c27",
        antiForgery: refusedChallenge.nonce,
        request: refusedRequest,
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );
    await assert.rejects(
      exportProjectArchive({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000c27",
        antiForgery: refusedChallenge.nonce,
        request: refusedRequest,
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );

    const afterArchive = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(afterArchive.status, "in_progress");
    assert.equal("immutable_root" in afterArchive, false);
    const archivedZip = await first.fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(archivedZip.status, 422);

    const historicalReady = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: historical.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(historicalReady.status, "ready");
    const expireHistorical = await queryPostgres(`
      UPDATE storyos.project_snapshots
         SET expires_at = clock_timestamp() - interval '1 second'
       WHERE owner_user_id = '${USER_A}'::uuid
         AND project_id = '${first.projectId}'::uuid
         AND snapshot_id = '${historical.admitted.effect.source_snapshot.snapshot_id}'::uuid;
      SELECT 'ok';
    `);
    assert.match(expireHistorical, /ok$/);
    await assert.rejects(
      getExportOperation({
        baseUrl,
        projectId: first.projectId,
        exportId: historical.admitted.effect.export_id,
        fetchImpl: first.fetchImpl,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 409 && String(protocol.responseBody).includes("snapshot_expired");
      },
    );
  } finally {
    await stopRealServer(server);
  }
});

test("the in-process Worker settles an admitted Project Export Archive to ready", async () => {
  const { baseUrl, server } = await startServer({ extraEnv: { STORYOS_WORKER: "1" } });
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-00001309d001", "Worker Novel",
      "018f0000-0000-7001-8000-000000000c10");
    const request = exportRequest("018f0000-0000-7001-8000-00000000d011");
    const applied = await postExport(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000d021",
      request,
    );
    assert.equal(applied.admitted.acknowledgement, "accepted");
    if (applied.admitted.effect.kind !== "admitted") {
      throw new Error("Project Export must admit");
    }
    const ready = await waitForExportStatus(
      baseUrl,
      first.projectId,
      applied.admitted.effect.export_id,
      first.fetchImpl,
      "ready",
    );
    assert.equal(ready.status, "ready");
    if (ready.status !== "ready") {
      throw new Error("the Worker must settle the Project Export Archive");
    }
    assert.match(ready.immutable_root, /^sha256:[0-9a-f]{64}$/);

    const exportUrl = `${baseUrl}/api/v1/projects/${encodeURIComponent(first.projectId)}/exports/${encodeURIComponent(applied.admitted.effect.export_id)}`;
    const zipResponse = await first.fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    if (zipResponse.status !== 200) {
      throw new Error(`${zipResponse.status} ${await zipResponse.text()}`);
    }
    assert.equal(zipResponse.headers.get("content-type"), ARCHIVE_MEDIA);
    const zipBytes = new Uint8Array(await zipResponse.arrayBuffer());
    assert.deepEqual(Array.from(zipBytes.slice(0, 4)), [0x50, 0x4b, 0x03, 0x04]);
    const zipFiles = zipStoreFiles(zipBytes);
    const reconfirmation = zipFiles.get("canonical/author_command_admission_reconfirmations.json");
    if (reconfirmation === undefined) {
      throw new Error("the archive must pack Reconfirmation evidence");
    }
    assert.equal(new TextDecoder().decode(reconfirmation), "[]");
    assert.equal(zipFiles.has("canonical/project_command_challenges.json"), false);
    assert.equal(zipFiles.has("canonical/create_project_challenges.json"), false);
    const zipRepeatResponse = await first.fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(zipRepeatResponse.status, 200);
    assert.deepEqual(
      Array.from(new Uint8Array(await zipRepeatResponse.arrayBuffer())),
      Array.from(zipBytes),
    );

    const replay = await exportProjectArchive({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d021",
      antiForgery: applied.nonce,
      request,
    });
    assert.equal(replay.acknowledgement, "accepted");
    assert.equal("receipt" in replay, false);
    if (replay.effect.kind !== "admitted" || applied.admitted.effect.kind !== "admitted") {
      throw new Error("retry after settlement must return the same admitted operation");
    }
    assert.equal(replay.effect.export_id, applied.admitted.effect.export_id);
    const stillReady = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(stillReady.status, "ready");

    const archived = await archiveOpenProject(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "1",
      "018f0000-0000-7001-8000-00000000d016",
      "018f0000-0000-7001-8000-00000000d026",
    );
    assert.equal(archived.effect.kind, "authoritative_applied");
    const readyAfterArchive = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(readyAfterArchive.status, "ready");
    const archivedZip = await first.fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(archivedZip.status, 200);
  } finally {
    await stopRealServer(server);
  }
});

test("the Worker settles failed when the Project is archived after admission", async () => {
  const { baseUrl, server } = await startServer();
  try {
    await settleOnce({ STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-00001309d002", "Archive After",
      "018f0000-0000-7001-8000-000000000c10");
    const applied = await postExport(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000d022",
      exportRequest("018f0000-0000-7001-8000-00000000d012"),
    );
    if (applied.admitted.effect.kind !== "admitted") {
      throw new Error("Project Export must admit");
    }
    await archiveOpenProject(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "1",
      "018f0000-0000-7001-8000-00000000d032",
      "018f0000-0000-7001-8000-00000000d036",
    );
    const waiting = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(waiting.status, "in_progress");
    await settleOnce();
    const failed = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(failed.status, "failed");
    assert.equal("immutable_root" in failed, false);
    assert.equal(
      await exportSettlementReceipt({
        ownerUserId: USER_A,
        projectId: first.projectId,
        exportId: applied.admitted.effect.export_id,
        operationsTable: "project_export_operations",
      }),
      "refused archived_project",
    );
    const exportUrl = `${baseUrl}/api/v1/projects/${encodeURIComponent(first.projectId)}/exports/${encodeURIComponent(applied.admitted.effect.export_id)}`;
    const failedZip = await first.fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(failedZip.status, 422);

    const replay = await exportProjectArchive({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d022",
      antiForgery: applied.nonce,
      request: exportRequest("018f0000-0000-7001-8000-00000000d012"),
    });
    assert.equal(replay.acknowledgement, "accepted");
    assert.equal("receipt" in replay, false);
    if (replay.effect.kind !== "admitted") {
      throw new Error("retry after failed settlement must return the same admitted operation");
    }
    assert.equal(replay.effect.export_id, applied.admitted.effect.export_id);
    const stillFailed = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(stillFailed.status, "failed");
  } finally {
    await stopRealServer(server);
  }
});

test("the Worker settles failed when the pinned Snapshot is unavailable before output exists", async () => {
  const { baseUrl, server } = await startServer();
  try {
    await settleOnce({ STORYOS_EXPORT_LEASE_TTL_SECS: "0" });
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-00001309d003", "Expired Snapshot",
      "018f0000-0000-7001-8000-000000000c10");
    const applied = await postExport(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000d023",
      exportRequest("018f0000-0000-7001-8000-00000000d013"),
    );
    if (applied.admitted.effect.kind !== "admitted") {
      throw new Error("Project Export must admit");
    }
    const expire = await queryPostgres(`
      UPDATE storyos.project_snapshots
         SET expires_at = clock_timestamp() - interval '1 second'
       WHERE owner_user_id = '${USER_A}'::uuid
         AND project_id = '${first.projectId}'::uuid
         AND snapshot_id = '${applied.admitted.effect.source_snapshot.snapshot_id}'::uuid;
      SELECT 'ok';
    `);
    assert.match(expire, /ok$/);
    const waiting = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(waiting.status, "in_progress");
    assert.equal("immutable_root" in waiting, false);
    await settleOnce();
    const failed = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(failed.status, "failed");
    assert.equal("immutable_root" in failed, false);
    assert.equal(
      await exportSettlementReceipt({
        ownerUserId: USER_A,
        projectId: first.projectId,
        exportId: applied.admitted.effect.export_id,
        operationsTable: "project_export_operations",
      }),
      "refused pinned_export_source_unavailable",
    );
  } finally {
    await stopRealServer(server);
  }
});
