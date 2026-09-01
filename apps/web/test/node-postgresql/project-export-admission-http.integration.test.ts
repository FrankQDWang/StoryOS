import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  archiveProject,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  digestArchiveProject,
  digestExportProjectArchive,
  exportProjectArchive,
  getExportOperation,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateProjectChallengeRequest,
  ExportProjectArchiveRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
  withChallengeRetry,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const USER_B = "018f0000-0000-7001-8000-000000000101";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function createChallengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000c10",
    },
    idempotency_key: idempotencyKey,
  };
}

function exportRequest(
  correlationId: string,
  archiveProfile = "storyos.project-export.v1",
  archivePathProfile = "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
): ExportProjectArchiveRequest {
  return {
    command_schema: "storyos.command.export-project-archive.request.v1",
    export_project_archive_input: {
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
      archive_profile: archiveProfile,
      archive_path_profile: archivePathProfile,
    },
  };
}

function archiveRequest(expectedProjectRevision: string, correlationId: string): ArchiveProjectRequest {
  return {
    command_schema: "storyos.command.archive-project.request.v1",
    archive_project_input: {
      expected_project_revision: expectedProjectRevision,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
    },
  };
}

async function startRealServer() {
  return startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A, "session-b": USER_B },
  });
}

async function createEmpty(baseUrl: string, session: string, idempotencyKey: string, title: string) {
  const fetchImpl = browserFetch(baseUrl, session);
  const request = createChallengeRequest(idempotencyKey, title);
  const challenge = await createProjectChallenge({ baseUrl, request, fetchImpl });
  await createProject({
    baseUrl,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request: {
      command_schema: request.command_schema,
      prospective_project_id: challenge.prospective_project_id,
      create_project_input: request.create_project_input,
    },
  });
  return { fetchImpl, projectId: challenge.prospective_project_id };
}

async function postExport(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: ExportProjectArchiveRequest,
) {
  const digest = await digestExportProjectArchive(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/exports",
      command_schema: "storyos.command.export-project-archive.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const admitted = await exportProjectArchive({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, admitted };
}

test("exportProjectArchive admits one inspectable operation with an immutable root", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000c01", "Empty Novel");
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
    assert.equal(applied.admitted.receipt.command_kind, "exportProjectArchive");
    assert.equal(applied.admitted.receipt.result, "authoritative_applied");
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
    assert.equal(applied.admitted.operation_ref?.kind, "project_export");

    const inspected = await getExportOperation({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(inspected.status, "in_progress");
    assert.match(inspected.immutable_root ?? "", /^sha256:[0-9a-f]{64}$/);
    assert.equal(inspected.export_id, applied.admitted.effect.export_id);
    assert.equal(inspected.archive_profile, "storyos.project-export.v1");
    assert.equal(
      inspected.archive_path_profile,
      "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
    );

    const archiveMedia =
      'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"';
    const exportUrl = `${baseUrl}/api/v1/projects/${encodeURIComponent(first.projectId)}/exports/${encodeURIComponent(applied.admitted.effect.export_id)}`;
    const zipResponse = await first.fetchImpl(exportUrl, { headers: { Accept: archiveMedia } });
    if (zipResponse.status !== 200) {
      throw new Error(`${zipResponse.status} ${await zipResponse.text()}`);
    }
    assert.equal(zipResponse.headers.get("content-type"), archiveMedia);
    const zipBytes = new Uint8Array(await zipResponse.arrayBuffer());
    assert.deepEqual(Array.from(zipBytes.slice(0, 4)), [0x50, 0x4b, 0x03, 0x04]);
    const zipRepeatResponse = await first.fetchImpl(exportUrl, { headers: { Accept: archiveMedia } });
    assert.equal(zipRepeatResponse.status, 200);
    assert.deepEqual(
      Array.from(new Uint8Array(await zipRepeatResponse.arrayBuffer())),
      Array.from(zipBytes),
    );

    const replay = await exportProjectArchive({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000c21",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.equal(replay.command_id, applied.admitted.command_id);
    assert.equal(replay.receipt.receipt_id, applied.admitted.receipt.receipt_id);
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

    const foreign = await createEmpty(baseUrl, "session-b", "018f0000-0000-7001-8000-000000000c02", "Other Novel");
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
    const foreignZip = await foreign.fetchImpl(exportUrl, { headers: { Accept: archiveMedia } });
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
    assert.equal(stillVisible.immutable_root, inspected.immutable_root);

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

    const archiveDigest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-000000000c16"));
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000c26",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000c26",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-000000000c16"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");

    const refused = await postExport(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000c27",
      exportRequest("018f0000-0000-7001-8000-000000000c17"),
    );
    assert.equal(refused.admitted.receipt.result, "refused");
    assert.equal(refused.admitted.effect.kind, "refused");
    if (refused.admitted.effect.kind !== "refused") {
      throw new Error("Project Export on an archived Project must refuse");
    }
    assert.equal(refused.admitted.effect.reason, "archived_project");

    await assert.rejects(
      getExportOperation({
        baseUrl,
        projectId: first.projectId,
        exportId: applied.admitted.effect.export_id,
        fetchImpl: first.fetchImpl,
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );
    const archivedZip = await first.fetchImpl(exportUrl, { headers: { Accept: archiveMedia } });
    assert.equal(archivedZip.status, 422);
  } finally {
    await stopRealServer(server);
  }
});
