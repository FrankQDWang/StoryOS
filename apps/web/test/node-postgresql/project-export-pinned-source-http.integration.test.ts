import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  createProjectCommandChallenge,
  createVolume,
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
  DigestValue,
  ExportHumanReadableManuscriptRequest,
  ExportProjectArchiveRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  createEmptyProject,
  exportSettlementReceipt,
  queryStoryOSPostgres as queryPostgres,
  runStoryOSWorker,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const exe = process.platform === "win32" ? ".exe" : "";
const serverBinary = join(repositoryRoot, "target", "release-package", `storyos-server${exe}`);
const workerBinary = join(repositoryRoot, "target", "release-package", `storyos-worker${exe}`);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const CLIENT = RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision;
const SECURITY = "storyos.web-security-policy.release-1.v1";
const ARCHIVE_MEDIA =
  'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"';
const ADMITTED_TITLE = "Pinned Archive Novel";

function bindings() {
  return { client_contract_revision: CLIENT, security_policy_revision: SECURITY };
}

function exportRequest(correlationId: string): ExportProjectArchiveRequest {
  return {
    command_schema: "storyos.command.export-project-archive.request.v1",
    export_project_archive_input: {
      ...bindings(),
      correlation_id: correlationId,
      archive_profile: "storyos.project-export.v1",
      archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
    },
  };
}

async function challenged<Result>(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  projectId: string;
  method: string;
  route: string;
  schema: string;
  idempotencyKey: string;
  digest: DigestValue;
  send: (antiForgery: string) => Promise<Result>;
}): Promise<{ antiForgery: string; result: Result }> {
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    fetchImpl: options.fetchImpl,
    request: {
      method: options.method,
      route_template: options.route,
      command_schema: options.schema,
      canonical_command_digest: options.digest,
      idempotency_key: options.idempotencyKey,
    },
  }));
  return {
    antiForgery: challenge.nonce,
    result: await options.send(challenge.nonce),
  };
}

/** StoryOS Project Export ZIP files use STORE only. */
function zipStoreFiles(bytes: Uint8Array): Map<string, Uint8Array> {
  const files = new Map<string, Uint8Array>();
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let offset = 0;
  while (offset + 30 <= bytes.length) {
    if (view.getUint32(offset, true) !== 0x04034b50) {
      break;
    }
    const size = view.getUint32(offset + 18, true);
    const nameLen = view.getUint16(offset + 26, true);
    const extraLen = view.getUint16(offset + 28, true);
    const nameStart = offset + 30;
    const name = new TextDecoder().decode(bytes.subarray(nameStart, nameStart + nameLen));
    const dataStart = nameStart + nameLen + extraLen;
    files.set(name, bytes.subarray(dataStart, dataStart + size));
    offset = dataStart + size;
  }
  return files;
}

async function admitArchive(
  command: { baseUrl: string; fetchImpl: typeof fetch; projectId: string },
  exportKey: string,
  correlationId: string,
): Promise<string> {
  const request = exportRequest(correlationId);
  const applied = await challenged({
    ...command,
    method: "POST",
    route: "/api/v1/projects/{project_id}/exports",
    schema: request.command_schema,
    idempotencyKey: exportKey,
    digest: await digestExportProjectArchive(request),
    send: (antiForgery) => exportProjectArchive({
      ...command, idempotencyKey: exportKey, antiForgery, request,
    }),
  });
  if (applied.result.effect.kind !== "admitted") {
    throw new Error("Project Export Archive must admit");
  }
  return applied.result.effect.export_id;
}

async function admitHumanReadableExport(
  command: { baseUrl: string; fetchImpl: typeof fetch; projectId: string },
  exportKey: string,
  correlationId: string,
): Promise<string> {
  const request: ExportHumanReadableManuscriptRequest = {
    command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
    export_human_readable_manuscript_input: { ...bindings(), correlation_id: correlationId },
  };
  const applied = await challenged({
    ...command,
    method: "POST",
    route: "/api/v1/projects/{project_id}/manuscript/exports",
    schema: request.command_schema,
    idempotencyKey: exportKey,
    digest: await digestExportHumanReadableManuscript(request),
    send: (antiForgery) => exportHumanReadableManuscript({
      ...command, idempotencyKey: exportKey, antiForgery, request,
    }),
  });
  if (applied.result.effect.kind !== "admitted") {
    throw new Error("Human-readable export must admit");
  }
  return applied.result.effect.export_id;
}

function exportUrlFor(baseUrl: string, projectId: string, exportId: string): string {
  return `${baseUrl}/api/v1/projects/${encodeURIComponent(projectId)}/exports/${encodeURIComponent(exportId)}`;
}

test("an admitted Project Export Archive settles the pinned families after later live changes", async () => {
  const { baseUrl, server } = await startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A },
    extraEnv: { STORYOS_WORKER: "0" },
  });
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
    const applied = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/exports",
      schema: request.command_schema,
      idempotencyKey: exportKey,
      digest: await digestExportProjectArchive(request),
      send: (antiForgery) => exportProjectArchive({
        ...command, idempotencyKey: exportKey, antiForgery, request,
      }),
    });
    assert.equal(applied.result.acknowledgement, "accepted");
    if (applied.result.effect.kind !== "admitted") {
      throw new Error("Project Export Archive must admit");
    }
    const exportId = applied.result.effect.export_id;
    const sourceSnapshotId = applied.result.effect.source_snapshot.snapshot_id;
    const exportUrl = exportUrlFor(baseUrl, projectId, exportId);

    const waiting = await getExportOperation({ baseUrl, projectId, exportId, fetchImpl });
    assert.equal(waiting.status, "in_progress");
    assert.equal("immutable_root" in waiting, false);
    assert.equal(waiting.source_snapshot.snapshot_id, sourceSnapshotId);
    const refusedZip = await fetchImpl(exportUrl, { headers: { Accept: ARCHIVE_MEDIA } });
    assert.equal(refusedZip.status, 422);

    const replay = await exportProjectArchive({
      ...command, idempotencyKey: exportKey, antiForgery: applied.antiForgery, request,
    });
    assert.equal(replay.command_id, applied.result.command_id);
    if (replay.effect.kind !== "admitted") {
      throw new Error("retry must return the same admitted operation");
    }
    assert.equal(replay.effect.export_id, exportId);

    const renameProject = {
      command_schema: "storyos.command.update-project.request.v1" as const,
      update_project_input: {
        title: "Later Archive Title",
        expected_project_revision: "1",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000e115",
      },
    };
    const renamed = await challenged({
      ...command,
      method: "PATCH",
      route: "/api/v1/projects/{project_id}",
      schema: renameProject.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e105",
      digest: await digestUpdateProject(renameProject),
      send: (antiForgery) => updateProject({
        ...command, idempotencyKey: "018f0000-0000-7001-8000-00000000e105", antiForgery,
        request: renameProject,
      }),
    });
    assert.equal(renamed.result.effect.kind, "authoritative_applied");

    const laterVolumeRequest = {
      command_schema: "storyos.command.create-volume.request.v1" as const,
      create_volume_input: {
        title: "Volume B",
        expected_tree_revision: "1",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000e11a",
      },
    };
    const laterVolume = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/volumes",
      schema: laterVolumeRequest.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000e10a",
      digest: await digestCreateVolume(laterVolumeRequest),
      send: (antiForgery) => createVolume({
        ...command, idempotencyKey: "018f0000-0000-7001-8000-00000000e10a", antiForgery,
        request: laterVolumeRequest,
      }),
    });
    assert.equal(laterVolume.result.effect.kind, "authoritative_applied");

    await runStoryOSWorker({ repositoryRoot, workerBinary, args: ["--once"] });
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
  const { baseUrl, server } = await startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A },
    extraEnv: { STORYOS_WORKER: "0" },
  });
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

    await runStoryOSWorker({ repositoryRoot, workerBinary, args: ["--once"] });
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
  const { baseUrl, server } = await startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A },
    extraEnv: { STORYOS_WORKER: "0" },
  });
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
    await runStoryOSWorker({ repositoryRoot, workerBinary, args: ["--once"] });
    await runStoryOSWorker({ repositoryRoot, workerBinary, args: ["--once"] });
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
