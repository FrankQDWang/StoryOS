import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  archiveProject,
  createChapter,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  digestArchiveProject,
  digestCreateChapter,
  digestCreateVolume,
  digestUpdateProject,
  digestUpdateVolume,
  getManuscriptTree,
  getProject,
  updateProject,
  updateVolume,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ArchiveProjectRequest,
  CreateChapterRequest,
  CreateProjectChallengeRequest,
  CreateVolumeRequest,
  UpdateProjectRequest,
  UpdateVolumeRequest,
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
const MISSING_VOLUME = "018f0000-0000-7001-8000-00000000ffff";
const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

function createChallengeRequest(idempotencyKey: string, title: string): CreateProjectChallengeRequest {
  return {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: {
      title,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000c30",
    },
    idempotency_key: idempotencyKey,
  };
}

function volumeRequest(title: string, expectedTreeRevision: string, correlationId: string): CreateVolumeRequest {
  return {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: {
      title,
      expected_tree_revision: expectedTreeRevision,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
    },
  };
}

function updateRequest(
  title: string,
  order: string,
  expectedTreeRevision: string,
  correlationId: string,
): UpdateVolumeRequest {
  return {
    command_schema: "storyos.command.update-volume.request.v1",
    update_volume_input: {
      title,
      order,
      expected_tree_revision: expectedTreeRevision,
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

async function postVolume(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: CreateVolumeRequest,
) {
  const digest = await digestCreateVolume(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/volumes",
      command_schema: "storyos.command.create-volume.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const created = await createVolume({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, created };
}

async function patchVolume(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  volumeId: string,
  idempotencyKey: string,
  request: UpdateVolumeRequest,
) {
  const digest = await digestUpdateVolume(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "PATCH",
      route_template: "/api/v1/projects/{project_id}/volumes/{volume_id}",
      command_schema: "storyos.command.update-volume.request.v1",
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const updated = await updateVolume({
    baseUrl,
    projectId,
    volumeId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, updated };
}

test("updateVolume renames and reorders one Volume, replays, and fails closed", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000c31", "Empty Novel");
    const volumeA = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000c51",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000c41"),
    );
    const volumeB = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000c52",
      volumeRequest("Volume B", "2", "018f0000-0000-7001-8000-000000000c42"),
    );
    assert.equal(volumeA.created.effect.kind, "authoritative_applied");
    assert.equal(volumeB.created.effect.kind, "authoritative_applied");
    if (volumeA.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume A must apply");
    }
    const volumeId = volumeA.created.effect.volume_id;
    const request = updateRequest("Volume B", "2", "3", "018f0000-0000-7001-8000-000000000c43");
    const applied = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c53",
      request,
    );
    assert.equal(applied.updated.schema_id, "storyos.command.update-volume.response.v1");
    assert.equal(applied.updated.receipt.command_kind, "updateVolume");
    assert.equal(applied.updated.receipt.result, "authoritative_applied");
    assert.equal(applied.updated.effect.kind, "authoritative_applied");
    if (applied.updated.effect.kind !== "authoritative_applied") {
      throw new Error("Update Volume must apply");
    }
    assert.equal(applied.updated.effect.volume_id, volumeId);
    assert.equal(applied.updated.effect.title, "Volume B");
    assert.equal(applied.updated.effect.order, "2");
    assert.equal(applied.updated.effect.tree_revision, "4");
    assert.match(applied.updated.effect.project_activity_position, /^[1-9][0-9]*$/);
    assert.equal(applied.updated.receipt.authoritative_commit_ids.length, 1);
    assert.match(applied.updated.receipt.authoritative_commit_ids[0] ?? "", UUID_V7);
    assert.equal(applied.updated.receipt.author_action_sequence, "3");
    assert.match(volumeId, UUID_V7);

    const replay = await updateVolume({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000c53",
      antiForgery: applied.challenge.nonce,
      request,
    });
    assert.equal(replay.command_id, applied.updated.command_id);
    assert.equal(replay.receipt.receipt_id, applied.updated.receipt.receipt_id);
    assert.deepEqual(
      replay.receipt.authoritative_commit_ids,
      applied.updated.receipt.authoritative_commit_ids,
    );
    assert.equal(replay.receipt.author_action_sequence, "3");

    const tree = await getManuscriptTree({ baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl });
    assert.equal(tree.tree_revision, "4");
    assert.equal(tree.snapshot.project_activity_position, applied.updated.effect.project_activity_position);
    assert.equal(tree.volumes.length, 2);
    assert.equal(tree.volumes[1]?.volume_id, volumeId);
    assert.equal(tree.volumes[1]?.title, "Volume B");
    assert.equal(tree.volumes[1]?.order, "2");

    const stale = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c54",
      updateRequest("Volume B", "2", "3", "018f0000-0000-7001-8000-000000000c44"),
    );
    assert.equal(stale.updated.receipt.result, "conflicted");
    assert.equal(stale.updated.effect.kind, "conflicted");
    if (stale.updated.effect.kind !== "conflicted") {
      throw new Error("stale Update Volume must conflict");
    }
    assert.equal(stale.updated.effect.reason, "stale_tree_revision");
    assert.deepEqual(stale.updated.receipt.authoritative_commit_ids, []);
    assert.equal(stale.updated.receipt.author_action_sequence, null);
    const afterStale = await getManuscriptTree({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(afterStale.tree_revision, "4");
    assert.equal(afterStale.volumes.length, 2);

    const unchanged = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c55",
      updateRequest("Volume B", "2", "4", "018f0000-0000-7001-8000-000000000c45"),
    );
    assert.equal(unchanged.updated.receipt.result, "no_effect");
    assert.equal(unchanged.updated.effect.kind, "no_effect");
    if (unchanged.updated.effect.kind !== "no_effect") {
      throw new Error("unchanged Update Volume must have no effect");
    }
    assert.equal(unchanged.updated.effect.reason, "unchanged");
    assert.deepEqual(unchanged.updated.receipt.authoritative_commit_ids, []);
    assert.equal(unchanged.updated.receipt.author_action_sequence, null);

    const invalidJoin = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      MISSING_VOLUME,
      "018f0000-0000-7001-8000-000000000c56",
      updateRequest("Volume B", "1", "4", "018f0000-0000-7001-8000-000000000c46"),
    );
    assert.equal(invalidJoin.updated.receipt.result, "refused");
    assert.equal(invalidJoin.updated.effect.kind, "refused");
    if (invalidJoin.updated.effect.kind !== "refused") {
      throw new Error("missing Volume must refuse");
    }
    assert.equal(invalidJoin.updated.effect.reason, "invalid_volume_join");
    assert.deepEqual(invalidJoin.updated.receipt.authoritative_commit_ids, []);
    assert.equal(invalidJoin.updated.receipt.author_action_sequence, null);

    const invalidOrder = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c57",
      updateRequest("Volume B", "3", "4", "018f0000-0000-7001-8000-000000000c47"),
    );
    assert.equal(invalidOrder.updated.receipt.result, "refused");
    if (invalidOrder.updated.effect.kind !== "refused") {
      throw new Error("order beyond the Volume count must refuse");
    }
    assert.equal(invalidOrder.updated.effect.reason, "invalid_order");

    await assert.rejects(
      updateVolume({
        baseUrl,
        projectId: first.projectId,
        volumeId,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-000000000c58",
        antiForgery: applied.challenge.nonce,
        request: updateRequest("Changed Retry", "2", "4", "018f0000-0000-7001-8000-000000000c48"),
      }),
      (error) => requireStoryOSProtocolError(error).status === 422,
    );

    await assert.rejects(
      patchVolume(
        baseUrl,
        first.fetchImpl,
        first.projectId,
        volumeId,
        "018f0000-0000-7001-8000-000000000c59",
        updateRequest("", "2", "4", "018f0000-0000-7001-8000-000000000c49"),
      ),
      (error) => requireStoryOSProtocolError(error).status === 400,
    );

    const archiveDigest = await digestArchiveProject(archiveRequest("1", "018f0000-0000-7001-8000-000000000c4a"));
    const archiveChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/archival",
        command_schema: "storyos.command.archive-project.request.v1",
        canonical_command_digest: archiveDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000c5a",
      },
    }));
    const archived = await archiveProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000c5a",
      antiForgery: archiveChallenge.nonce,
      request: archiveRequest("1", "018f0000-0000-7001-8000-000000000c4a"),
    });
    assert.equal(archived.effect.kind, "authoritative_applied");

    const refused = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000c5b",
      updateRequest("Volume B", "2", "4", "018f0000-0000-7001-8000-000000000c4b"),
    );
    assert.equal(refused.updated.receipt.result, "refused");
    assert.equal(refused.updated.effect.kind, "refused");
    if (refused.updated.effect.kind !== "refused") {
      throw new Error("Update Volume on an archived Project must refuse");
    }
    assert.equal(refused.updated.effect.reason, "archived_project");

    const foreign = await createEmpty(baseUrl, "session-b", "018f0000-0000-7001-8000-000000000c32", "Other Novel");
    await assert.rejects(
      patchVolume(
        baseUrl,
        foreign.fetchImpl,
        first.projectId,
        volumeId,
        "018f0000-0000-7001-8000-000000000c5c",
        updateRequest("Stolen Volume", "1", "4", "018f0000-0000-7001-8000-000000000c4c"),
      ),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 404 && !String(protocol.responseBody).includes(USER_A);
      },
    );
  } finally {
    await stopRealServer(server);
  }
});

function capturingVolumePatch(inner: typeof fetch): { fetchImpl: typeof fetch; lastPatchBody: () => string } {
  let lastPatchBody = "";
  return {
    fetchImpl: async (input, init) => {
      const response = await inner(input, init);
      const url = String(input instanceof Request ? input.url : input);
      if (response.ok && (init?.method ?? "GET") === "PATCH" && url.includes("/volumes/")) {
        lastPatchBody = await response.clone().text();
      }
      return response;
    },
    lastPatchBody: () => lastPatchBody,
  };
}

function problemCode(error: unknown): string | undefined {
  const protocol = requireStoryOSProtocolError(error);
  if (protocol.responseBody === undefined) return undefined;
  try {
    return (JSON.parse(protocol.responseBody) as { code?: string }).code;
  } catch {
    return undefined;
  }
}

function renameRequest(title: string, expectedProjectRevision: string, correlationId: string): UpdateProjectRequest {
  return {
    command_schema: "storyos.command.update-project.request.v1",
    update_project_input: {
      title,
      expected_project_revision: expectedProjectRevision,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      correlation_id: correlationId,
    },
  };
}

async function renameProject(
  baseUrl: string,
  fetchImpl: typeof fetch,
  projectId: string,
  idempotencyKey: string,
  request: UpdateProjectRequest,
) {
  const digest = await digestUpdateProject(request);
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl,
    projectId,
    fetchImpl,
    request: {
      method: "PATCH",
      route_template: "/api/v1/projects/{project_id}",
      command_schema: request.command_schema,
      canonical_command_digest: digest,
      idempotency_key: idempotencyKey,
    },
  }));
  const renamed = await updateProject({
    baseUrl,
    projectId,
    fetchImpl,
    idempotencyKey,
    antiForgery: challenge.nonce,
    request,
  });
  return { challenge, renamed };
}

// One Project admits at most 10 Command Challenges in one 60-second window.
test("updateVolume freezes applied, no-effect, and conflict acknowledgements after later title and Current Chapter changes", async () => {
  let { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-000000000d00", "Volume Update Freeze Novel");
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000d01",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-000000000d02"),
    );
    if (volume.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const volumeId = volume.created.effect.volume_id;
    const appliedRequest = updateRequest("Volume A Renamed", "1", "2", "018f0000-0000-7001-8000-000000000d04");
    const firstCapture = capturingVolumePatch(first.fetchImpl);
    const applied = await patchVolume(
      baseUrl,
      firstCapture.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000d03",
      appliedRequest,
    );
    const appliedBody = firstCapture.lastPatchBody();
    assert.equal(applied.updated.effect.kind, "authoritative_applied");
    assert.equal(applied.updated.project.title, "Volume Update Freeze Novel");
    assert.equal(applied.updated.project.open.kind, "empty");
    const unchangedRequest = updateRequest("Volume A Renamed", "1", "3", "018f0000-0000-7001-8000-000000000d0a");
    const unchangedCapture = capturingVolumePatch(first.fetchImpl);
    const unchanged = await patchVolume(
      baseUrl,
      unchangedCapture.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000d09",
      unchangedRequest,
    );
    const unchangedBody = unchangedCapture.lastPatchBody();
    assert.equal(unchanged.updated.receipt.result, "no_effect");
    assert.equal(unchanged.updated.project.title, "Volume Update Freeze Novel");
    assert.equal(unchanged.updated.project.open.kind, "empty");
    const staleRequest = updateRequest("Stale Volume", "1", "2", "018f0000-0000-7001-8000-000000000d0c");
    const staleCapture = capturingVolumePatch(first.fetchImpl);
    const stale = await patchVolume(
      baseUrl,
      staleCapture.fetchImpl,
      first.projectId,
      volumeId,
      "018f0000-0000-7001-8000-000000000d0b",
      staleRequest,
    );
    const staleBody = staleCapture.lastPatchBody();
    assert.equal(stale.updated.receipt.result, "conflicted");
    assert.equal(stale.updated.project.title, "Volume Update Freeze Novel");
    const chapterRequest: CreateChapterRequest = {
      command_schema: "storyos.command.create-chapter.request.v1",
      create_chapter_input: {
        title: "Chapter A",
        expected_tree_revision: "3",
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000d06",
      },
    };
    const chapterDigest = await digestCreateChapter(chapterRequest);
    const chapterChallenge = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
      request: {
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
        command_schema: chapterRequest.command_schema,
        canonical_command_digest: chapterDigest,
        idempotency_key: "018f0000-0000-7001-8000-000000000d05",
      },
    }));
    const chapter = await createChapter({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: first.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000d05",
      antiForgery: chapterChallenge.nonce,
      request: chapterRequest,
    });
    assert.equal(chapter.effect.kind, "authoritative_applied");
    if (chapter.effect.kind !== "authoritative_applied") {
      throw new Error("Create Chapter must apply");
    }
    const chapterId = chapter.effect.chapter_id;
    const later = await renameProject(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-000000000d07",
      renameRequest("Later Volume Title", "1", "018f0000-0000-7001-8000-000000000d08"),
    );
    assert.equal(later.renamed.project.title, "Later Volume Title");
    const frozenCapture = capturingVolumePatch(first.fetchImpl);
    const frozen = await updateVolume({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: frozenCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000d03",
      antiForgery: applied.challenge.nonce,
      request: appliedRequest,
    });
    assert.deepEqual(frozen, applied.updated);
    assert.equal(frozenCapture.lastPatchBody(), appliedBody);
    assert.equal(frozen.project.title, "Volume Update Freeze Novel");
    assert.equal(frozen.project.open.kind, "empty");
    const frozenUnchangedCapture = capturingVolumePatch(first.fetchImpl);
    const frozenUnchanged = await updateVolume({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: frozenUnchangedCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000d09",
      antiForgery: unchanged.challenge.nonce,
      request: unchangedRequest,
    });
    assert.deepEqual(frozenUnchanged, unchanged.updated);
    assert.equal(frozenUnchangedCapture.lastPatchBody(), unchangedBody);
    const frozenStaleCapture = capturingVolumePatch(first.fetchImpl);
    const frozenStale = await updateVolume({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: frozenStaleCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000d0b",
      antiForgery: stale.challenge.nonce,
      request: staleRequest,
    });
    assert.deepEqual(frozenStale, stale.updated);
    assert.equal(frozenStaleCapture.lastPatchBody(), staleBody);
    const opened = await getProject({
      baseUrl,
      projectId: first.projectId,
      fetchImpl: first.fetchImpl,
    });
    assert.equal(opened.project.title, "Later Volume Title");
    assert.equal(opened.project.open.kind, "current_chapter");
    if (opened.project.open.kind !== "current_chapter") {
      throw new Error("GET must report Chapter A");
    }
    assert.equal(opened.project.open.current_chapter_id, chapterId);
    const receiptCount = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'updateVolume';
    `);
    assert.equal(receiptCount, "3");
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.authoritative_commits
         WHERE project_id = '${first.projectId}'::uuid
           AND receipt_id = '${applied.updated.receipt.receipt_id}'::uuid;
      `),
      "1",
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.author_action_entries
         WHERE project_id = '${first.projectId}'::uuid
           AND receipt_id = '${applied.updated.receipt.receipt_id}'::uuid;
      `),
      "1",
    );
    await stopRealServer(server);
    ({ baseUrl, server } = await startRealServer());
    const restartedFetch = browserFetch(baseUrl, "session-a");
    const afterRestartCapture = capturingVolumePatch(restartedFetch);
    const afterRestart = await updateVolume({
      baseUrl,
      projectId: first.projectId,
      volumeId,
      fetchImpl: afterRestartCapture.fetchImpl,
      idempotencyKey: "018f0000-0000-7001-8000-000000000d03",
      antiForgery: applied.challenge.nonce,
      request: appliedRequest,
    });
    assert.deepEqual(afterRestart, applied.updated);
    assert.equal(afterRestartCapture.lastPatchBody(), appliedBody);
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${first.projectId}'::uuid AND command_kind = 'updateVolume';
      `),
      receiptCount,
    );
  } finally {
    await stopRealServer(server);
  }
});

test("updateVolume distinguishes historical absence from damaged new-format evidence", async () => {
  const { baseUrl, server } = await startRealServer();
  try {
    const first = await createEmpty(baseUrl, "session-a", "018f0000-0000-7001-8000-0000000008b0", "Volume History Novel");
    const volume = await postVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      "018f0000-0000-7001-8000-0000000008b1",
      volumeRequest("Volume A", "1", "018f0000-0000-7001-8000-0000000008b2"),
    );
    if (volume.created.effect.kind !== "authoritative_applied") {
      throw new Error("Create Volume must apply");
    }
    const request = updateRequest("Volume A Renamed", "1", "2", "018f0000-0000-7001-8000-0000000008b4");
    const applied = await patchVolume(
      baseUrl,
      first.fetchImpl,
      first.projectId,
      volume.created.effect.volume_id,
      "018f0000-0000-7001-8000-0000000008b3",
      request,
    );
    const novelsBefore = await queryPostgres(`
      SELECT title || ' ' || count(*)::text FROM storyos.projects
       WHERE project_id = '${first.projectId}'::uuid GROUP BY title;
    `);
    const receiptsBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.domain_receipts
       WHERE project_id = '${first.projectId}'::uuid;
    `);
    const volumesBefore = await queryPostgres(`
      SELECT count(*) FROM storyos.manuscript_objects
       WHERE project_id = '${first.projectId}'::uuid AND object_kind = 'volume';
    `);
    await queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = NULL, response_project = NULL
       WHERE project_id = '${first.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-0000000008b3'::uuid;
    `);
    await assert.rejects(
      updateVolume({
        baseUrl,
        projectId: first.projectId,
        volumeId: volume.created.effect.volume_id,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-0000000008b3",
        antiForgery: applied.challenge.nonce,
        request,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 409
          && problemCode(error) === "historical_acknowledgement_unavailable";
      },
    );
    await queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = 'command_response_project.v1',
             response_project = '{"broken":true}'::jsonb
       WHERE project_id = '${first.projectId}'::uuid
         AND idempotency_key = '018f0000-0000-7001-8000-0000000008b3'::uuid;
    `);
    await assert.rejects(
      updateVolume({
        baseUrl,
        projectId: first.projectId,
        volumeId: volume.created.effect.volume_id,
        fetchImpl: first.fetchImpl,
        idempotencyKey: "018f0000-0000-7001-8000-0000000008b3",
        antiForgery: applied.challenge.nonce,
        request,
      }),
      (error) => {
        const protocol = requireStoryOSProtocolError(error);
        return protocol.status === 503
          && problemCode(error) !== "historical_acknowledgement_unavailable";
      },
    );
    assert.equal(
      await queryPostgres(`
        SELECT title || ' ' || count(*)::text FROM storyos.projects
         WHERE project_id = '${first.projectId}'::uuid GROUP BY title;
      `),
      novelsBefore,
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.domain_receipts
         WHERE project_id = '${first.projectId}'::uuid;
      `),
      receiptsBefore,
    );
    assert.equal(
      await queryPostgres(`
        SELECT count(*) FROM storyos.manuscript_objects
         WHERE project_id = '${first.projectId}'::uuid AND object_kind = 'volume';
      `),
      volumesBefore,
    );
  } finally {
    await stopRealServer(server);
  }
});
