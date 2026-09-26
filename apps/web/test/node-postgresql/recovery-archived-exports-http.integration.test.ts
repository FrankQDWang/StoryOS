// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/project-export-admission-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  applyAuthorEdit,
  archiveProject,
  createChapter,
  createEditorSession,
  createProjectCommandChallenge,
  createVolume,
  digestApplyAuthorEdit,
  digestArchiveProject,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  digestExportHumanReadableManuscript,
  digestExportProjectArchive,
  exportHumanReadableManuscript,
  exportProjectArchive,
  getExportOperation,
  getHumanReadableManuscriptExport,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ApplyAuthorEditRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  createEmptyProject,
  queryStoryOSPostgres,
  runStoryOSWorker,
  sessionFetch,
  startStoryOSServer,
  stopStoryOSServer,
  withChallengeRetry,
} from "../support/node-integration.ts";
import { zipStoreFiles } from "../support/archive.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target/release-package/storyos-server");
const workerBinary = join(repositoryRoot, "target/release-package/storyos-worker");
const ownerUserId = "018f0000-0000-7001-8000-000000000001";
const bindings = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};
const prose = "The archived chapter keeps its own words.";

async function addProse(command: { baseUrl: string; projectId: string; fetchImpl: typeof fetch }, suffix: string) {
  const { baseUrl, projectId, fetchImpl } = command;
  const challenged = async (method: string, route_template: string, command_schema: string,
    canonical_command_digest: Awaited<ReturnType<typeof digestCreateVolume>>, idempotency_key: string) => {
    const result = await withChallengeRetry(() => createProjectCommandChallenge({
      baseUrl, projectId, fetchImpl,
      request: { method, route_template, command_schema, canonical_command_digest, idempotency_key },
    }));
    return result.nonce;
  };
  const volumeRequest = {
    command_schema: "storyos.command.create-volume.request.v1" as const,
    create_volume_input: { title: "Recovery Volume", expected_tree_revision: "1", ...bindings,
      correlation_id: `018f0000-0000-7001-8000-00000000${suffix}11` },
  };
  const volumeKey = `018f0000-0000-7001-8000-00000000${suffix}01`;
  const volume = await createVolume({ ...command, request: volumeRequest,
    idempotencyKey: volumeKey, antiForgery: await challenged("POST",
      "/api/v1/projects/{project_id}/volumes", volumeRequest.command_schema,
      await digestCreateVolume(volumeRequest), volumeKey) });
  if (volume.effect.kind !== "authoritative_applied") throw new Error("Recovery Volume was not created");
  const volumeId = volume.effect.volume_id;

  const chapterRequest = {
    command_schema: "storyos.command.create-chapter.request.v1" as const,
    create_chapter_input: { title: "Recovery Chapter", expected_tree_revision: "2", ...bindings,
      correlation_id: `018f0000-0000-7001-8000-00000000${suffix}12` },
  };
  const chapterKey = `018f0000-0000-7001-8000-00000000${suffix}02`;
  const chapter = await createChapter({ ...command, volumeId, request: chapterRequest,
    idempotencyKey: chapterKey, antiForgery: await challenged("POST",
      "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters", chapterRequest.command_schema,
      await digestCreateChapter(chapterRequest), chapterKey) });
  if (chapter.effect.kind !== "authoritative_applied") throw new Error("Recovery Chapter was not created");

  const sessionRequest = {
    command_schema: "storyos.command.create-editor-session.request.v1" as const,
    ...bindings,
    correlation_id: `018f0000-0000-7001-8000-00000000${suffix}13`,
  };
  const sessionKey = `018f0000-0000-7001-8000-00000000${suffix}03`;
  const session = await createEditorSession({ ...command, request: sessionRequest,
    idempotencyKey: sessionKey, antiForgery: await challenged("POST",
      "/api/v1/projects/{project_id}/editor-sessions", sessionRequest.command_schema,
      await digestCreateEditorSession(sessionRequest), sessionKey) });
  if (session.writer.kind !== "current_writer") throw new Error("Recovery Chapter has no current writer");
  const from = session.base_snapshot.materialized_revision.body.length;
  const editRequest: ApplyAuthorEditRequest = {
    command_schema: "storyos.command.apply-author-edit.request.v1",
    ...bindings,
    correlation_id: `018f0000-0000-7001-8000-00000000${suffix}14`,
    editor_session_id: session.editor_session.editor_session_id,
    writer_generation: session.writer.writer_generation,
    chapter_id: session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: session.base_snapshot.proposal_head_revision_ids,
    target_refs: session.base_snapshot.target_refs,
    observed_ownership_partition: session.base_snapshot.observed_ownership_partition,
    editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: `018f0000-0000-7001-8000-00000000${suffix}15`,
    completed_intent_record_id: `018f0000-0000-7001-8000-00000000${suffix}16`,
    local_intent_sequence: "1",
    author_edit_units: [{ normalized_primitives: [{ kind: "replace_selection", from, to: from, text: prose }],
      selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from } }],
  };
  const editKey = `018f0000-0000-7001-8000-00000000${suffix}04`;
  const edited = await applyAuthorEdit({ ...command, request: editRequest,
    idempotencyKey: editKey, antiForgery: await challenged("POST",
      "/api/v1/projects/{project_id}/manuscript/author-edits", editRequest.command_schema,
      await digestApplyAuthorEdit(editRequest), editKey) });
  assert.equal(edited.effect.kind, "authoritative_applied");
}

test("public completed exports remain retained after lawful archive for physical recovery", async () => {
  const { baseUrl, server } = await startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": ownerUserId },
    extraEnv: { STORYOS_WORKER: "0" },
  });
  try {
    const fetchImpl = sessionFetch(baseUrl, "session-a");
    for (const [kind, key, title] of [
      ["readable", "e801", "Recovery Readable Archived"],
      ["archive", "e802", "Recovery Archive Archived"],
    ] as const) {
      const projectId = await createEmptyProject({
        baseUrl,
        fetchImpl,
        createKey: `018f0000-0000-7001-8000-00000000${key}`,
        correlationId: `018f0000-0000-7001-8000-00000000${kind === "readable" ? "e811" : "e812"}`,
        title,
      });
      const command = { baseUrl, projectId, fetchImpl };
      await addProse(command, kind === "readable" ? "e9" : "ea");
      const correlationId = `018f0000-0000-7001-8000-00000000${kind === "readable" ? "e821" : "e822"}`;
      const exportKey = `018f0000-0000-7001-8000-00000000${kind === "readable" ? "e831" : "e832"}`;
      let exportId: string;
      if (kind === "readable") {
        const request = {
          command_schema: "storyos.command.export-human-readable-manuscript.request.v1" as const,
          export_human_readable_manuscript_input: { ...bindings, correlation_id: correlationId },
        };
        const challenge = await createProjectCommandChallenge({
          ...command,
          request: {
            method: "POST",
            route_template: "/api/v1/projects/{project_id}/manuscript/exports",
            command_schema: request.command_schema,
            canonical_command_digest: await digestExportHumanReadableManuscript(request),
            idempotency_key: exportKey,
          },
        });
        const admitted = await exportHumanReadableManuscript({
          ...command, request, idempotencyKey: exportKey, antiForgery: challenge.nonce,
        });
        assert.equal(admitted.effect.kind, "admitted");
        if (admitted.effect.kind !== "admitted") throw new Error("readable export was not admitted");
        exportId = admitted.effect.export_id;
      } else {
        const request = {
          command_schema: "storyos.command.export-project-archive.request.v1" as const,
          export_project_archive_input: {
            ...bindings,
            correlation_id: correlationId,
            archive_profile: "storyos.project-export.v1",
            archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
          },
        };
        const challenge = await createProjectCommandChallenge({
          ...command,
          request: {
            method: "POST",
            route_template: "/api/v1/projects/{project_id}/exports",
            command_schema: request.command_schema,
            canonical_command_digest: await digestExportProjectArchive(request),
            idempotency_key: exportKey,
          },
        });
        const admitted = await exportProjectArchive({
          ...command, request, idempotencyKey: exportKey, antiForgery: challenge.nonce,
        });
        assert.equal(admitted.effect.kind, "admitted");
        if (admitted.effect.kind !== "admitted") throw new Error("Project Export Archive was not admitted");
        exportId = admitted.effect.export_id;
      }
      await runStoryOSWorker({ repositoryRoot, workerBinary, args: ["--once"] });
      const ready = kind === "readable"
        ? await getHumanReadableManuscriptExport({ ...command, exportId })
        : await getExportOperation({ ...command, exportId });
      assert.equal(ready.status, "ready");
      if (kind === "readable") {
        const document = await getHumanReadableManuscriptExport({ ...command, exportId });
        assert.equal(document.status, "ready");
        if (document.status !== "ready") throw new Error("readable export did not settle");
        assert.match(document.manuscript_utf8, /The archived chapter keeps its own words\./);
      } else {
        const response = await fetchImpl(`${baseUrl}/api/v1/projects/${projectId}/exports/${exportId}`, {
          headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' },
        });
        assert.equal(response.status, 200);
        const files = zipStoreFiles(new Uint8Array(await response.arrayBuffer()));
        const payloads = JSON.parse(new TextDecoder().decode(files.get("canonical/authoritative_payloads.json")));
        assert.ok(JSON.stringify(payloads).includes(Buffer.from(prose).toString("hex")));
      }

      const archiveRequest = {
        command_schema: "storyos.command.archive-project.request.v1" as const,
        archive_project_input: {
          ...bindings,
          expected_project_revision: "1",
          correlation_id: `018f0000-0000-7001-8000-00000000${kind === "readable" ? "e841" : "e842"}`,
        },
      };
      const archiveKey = `018f0000-0000-7001-8000-00000000${kind === "readable" ? "e851" : "e852"}`;
      const archiveChallenge = await createProjectCommandChallenge({
        ...command,
        request: {
          method: "PUT",
          route_template: "/api/v1/projects/{project_id}/archival",
          command_schema: archiveRequest.command_schema,
          canonical_command_digest: await digestArchiveProject(archiveRequest),
          idempotency_key: archiveKey,
        },
      });
      const archived = await archiveProject({
        ...command,
        request: archiveRequest,
        idempotencyKey: archiveKey,
        antiForgery: archiveChallenge.nonce,
      });
      assert.equal(archived.effect.kind, "authoritative_applied");
      const retainedOperation = kind === "readable"
        ? await getHumanReadableManuscriptExport({ ...command, exportId })
        : await getExportOperation({ ...command, exportId });
      assert.equal(retainedOperation.status, "ready");
      const retained = await queryStoryOSPostgres(`
        SELECT project.lifecycle_state || '/' ||
               (SELECT count(*) FROM storyos.project_archival_decisions AS decision
                 WHERE (decision.owner_user_id, decision.project_id) =
                       (project.owner_user_id, project.project_id)) || '/' ||
               (SELECT count(*) FROM storyos.${kind === "readable" ? "human_readable_manuscript_exports" : "project_export_manifests"} AS export
                 WHERE (export.owner_user_id, export.project_id) =
                       (project.owner_user_id, project.project_id))
          FROM storyos.projects AS project
         WHERE project.project_id = '${projectId}'::uuid;
      `);
      assert.equal(retained, "archived/1/1");
    }
  } finally {
    await stopStoryOSServer(server);
  }
});
