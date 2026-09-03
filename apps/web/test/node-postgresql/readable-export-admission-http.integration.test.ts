import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  archiveProject,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  digestArchiveProject,
  digestExportHumanReadableManuscript,
  exportHumanReadableManuscript,
  getHumanReadableManuscriptExport,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateProjectChallengeRequest,
  ExportHumanReadableManuscriptRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
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
const HISTORICAL_RECEIPT = "018f0000-0000-7001-8000-00000000b101";
const HISTORICAL_EVENT = "018f0000-0000-7001-8000-00000000b102";
const EMPTY_MANUSCRIPT = "\n";
const EMPTY_MANUSCRIPT_SHA256 = createHash("sha256").update(EMPTY_MANUSCRIPT).digest("hex");

function createChallengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-00000000ae10",
    },
    idempotency_key: idempotencyKey,
  };
}

function exportRequest(correlationId: string): ExportHumanReadableManuscriptRequest {
  return {
    command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
    export_human_readable_manuscript_input: {
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
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
  request: ExportHumanReadableManuscriptRequest,
) {
  const digest = await digestExportHumanReadableManuscript(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/manuscript/exports",
      command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const admitted = await exportHumanReadableManuscript({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, admitted };
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
           'exportHumanReadableManuscript', admission.canonical_command_digest,
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
           '${HISTORICAL_EVENT}'::uuid, 'human_readable_manuscript_export_settled',
           '${HISTORICAL_RECEIPT}'::uuid, 'authoritative_applied',
           jsonb_build_object(
             'kind', 'human_readable_manuscript_export_settled',
             'export_id', '${options.exportId}',
             'content_sha256', '${EMPTY_MANUSCRIPT_SHA256}'
           )
      FROM storyos.scope_counters AS counters
     WHERE counters.owner_user_id = '${options.ownerUserId}'::uuid
       AND counters.project_id = '${options.projectId}'::uuid;
    INSERT INTO storyos.human_readable_manuscript_exports
      (owner_user_id, project_id, export_id, receipt_id, source_snapshot_id,
       source_activity_position, export_profile, manuscript_utf8, content_sha256)
    VALUES (
      '${options.ownerUserId}'::uuid, '${options.projectId}'::uuid,
      '${options.exportId}'::uuid, '${HISTORICAL_RECEIPT}'::uuid, '${options.snapshotId}',
      ${options.sourceActivityPosition}::bigint, 'storyos.readable-export.utf8-lf.v1',
      E'\\n', '${EMPTY_MANUSCRIPT_SHA256}'
    );
    DELETE FROM storyos.human_readable_manuscript_export_operations
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
  assert.equal(result, "ok");
}

test("exportHumanReadableManuscript admits one inspectable in-progress operation", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-00000000ae01", "Empty Novel");
    const request = exportRequest("018f0000-0000-7001-8000-00000000ae11");
    const applied = await postExport(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000ae21",
      request,
    ).catch((error) => {
      const protocol = requireStoryOSProtocolError(error);
      throw new Error(`${protocol.status} ${protocol.responseBody}`);
    });
    assert.equal(applied.admitted.schema_id, "storyos.command.export-human-readable-manuscript.response.v1");
    assert.equal(applied.admitted.acknowledgement, "accepted");
    assert.equal("receipt" in applied.admitted, false);
    assert.equal(applied.admitted.effect.kind, "admitted");
    if (applied.admitted.effect.kind !== "admitted") {
      throw new Error("Human-readable export must admit");
    }
    assert.match(applied.admitted.effect.export_id, UUID_V7);
    assert.equal(applied.admitted.effect.export_profile, "storyos.readable-export.utf8-lf.v1");
    assert.match(applied.admitted.effect.source_snapshot.snapshot_id, UUID_V7);
    assert.equal(applied.admitted.operation_ref?.kind, "human_readable_manuscript_export");

    const inspected = await getHumanReadableManuscriptExport({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(inspected.status, "in_progress");
    if (inspected.status !== "in_progress") {
      throw new Error("a new human-readable export must stay in progress");
    }
    assert.equal("manuscript_utf8" in inspected, false);
    assert.equal("content_sha256" in inspected, false);
    assert.equal(inspected.export_id, applied.admitted.effect.export_id);
    assert.equal(inspected.export_profile, "storyos.readable-export.utf8-lf.v1");
    assert.equal(
      inspected.source_snapshot.snapshot_id,
      applied.admitted.effect.source_snapshot.snapshot_id,
    );

    const replay = await exportHumanReadableManuscript({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000ae21",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.equal(replay.command_id, applied.admitted.command_id);
    assert.equal(replay.author_command_admission_id, applied.admitted.author_command_admission_id);
    if (replay.effect.kind !== "admitted" || applied.admitted.effect.kind !== "admitted") {
      throw new Error("retry must return the same admitted operation");
    }
    assert.equal(replay.effect.export_id, applied.admitted.effect.export_id);

    await assert.rejects(
      getHumanReadableManuscriptExport({
        baseUrl,
        projectId: first.projectId,
        exportId: "018f0000-0000-7001-8000-00000000ae99",
        fetchImpl: first.fetchImpl,
      }),
      (error) => requireStoryOSProtocolError(error).status === 404,
    );

    const foreign = await createEmpty(baseUrl, "session-b", "018f0000-0000-7001-8000-00000000ae02", "Other Novel");
    await assert.rejects(
      getHumanReadableManuscriptExport({
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
    await assert.rejects(
      postExport(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        "018f0000-0000-7001-8000-00000000ae23",
        exportRequest("018f0000-0000-7001-8000-00000000ae13"),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );

    const stillVisible = await getHumanReadableManuscriptExport({
      baseUrl,
      projectId: first.projectId,
      exportId: applied.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(stillVisible.status, "in_progress");

    const historical = await postExport(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-00000000ae28",
      exportRequest("018f0000-0000-7001-8000-00000000ae18"),
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
    const ready = await getHumanReadableManuscriptExport({
      baseUrl,
      projectId: first.projectId,
      exportId: historical.admitted.effect.export_id,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(ready.status, "ready");
    if (ready.status !== "ready") {
      throw new Error("a historical human-readable export must stay ready");
    }
    assert.equal(ready.manuscript_utf8, EMPTY_MANUSCRIPT);
    assert.equal(ready.content_sha256, EMPTY_MANUSCRIPT_SHA256);
    assert.equal(ready.export_id, historical.admitted.effect.export_id);

    const archiveDigest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-00000000ae16"));
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-00000000ae26",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-00000000ae26",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-00000000ae16"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");

    const refusedRequest = exportRequest("018f0000-0000-7001-8000-00000000ae17");
    const refusedDigest = await digestExportHumanReadableManuscript(refusedRequest);
    const refusedChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/exports",
        command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
        canonical_command_digest: refusedDigest,
        idempotency_key: "018f0000-0000-7001-8000-00000000ae27",
      },
    }));
    await assert.rejects(
      exportHumanReadableManuscript({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-00000000ae27",
        antiForgery: refusedChallenge.nonce,
        request: refusedRequest,
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );
    await assert.rejects(
      exportHumanReadableManuscript({
        baseUrl,
        projectId: first.projectId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-00000000ae27",
        antiForgery: refusedChallenge.nonce,
        request: refusedRequest,
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );

    await assert.rejects(
      getHumanReadableManuscriptExport({
        baseUrl,
        projectId: first.projectId,
        exportId: applied.admitted.effect.export_id,
        fetchImpl: first.fetchImpl,
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );
  } finally {
    await stopRealServer(server);
  }
});
