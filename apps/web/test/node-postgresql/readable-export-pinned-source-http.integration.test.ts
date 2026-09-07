import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  applyAuthorEdit,
  createChapter,
  createEditorSession,
  createProject,
  createProjectChallenge,
  createProjectCommandChallenge,
  createVolume,
  digestApplyAuthorEdit,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  digestExportHumanReadableManuscript,
  digestUpdateChapter,
  digestUpdateProject,
  digestUpdateVolume,
  exportHumanReadableManuscript,
  getHumanReadableManuscriptExport,
  updateChapter,
  updateProject,
  updateVolume,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateEditorSessionRequest,
  ExportHumanReadableManuscriptRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
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
const ADMITTED_MANUSCRIPT = "# Volume A\n\n## Chapter A\n\n\n";

function bindings() {
  return { client_contract_revision: CLIENT, security_policy_revision: SECURITY };
}

function exportRequest(correlationId: string): ExportHumanReadableManuscriptRequest {
  return {
    command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
    export_human_readable_manuscript_input: { ...bindings(), correlation_id: correlationId },
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
  digest: string;
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

test("an admitted human-readable export settles the pinned manuscript after later live changes", async () => {
  const { baseUrl, server } = await startStoryOSServer({
    repositoryRoot,
    serverBinary,
    sessions: { "session-a": USER_A },
    extraEnv: { STORYOS_WORKER: "0" },
  });
  try {
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const createKey = "018f0000-0000-7001-8000-00000000d101";
    const createRequest = {
      command_schema: "storyos.command.create-project.request.v1" as const,
      create_project_input: {
        title: "Pinned Source Novel",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000d111",
      },
      idempotency_key: createKey,
    };
    const created = await createProjectChallenge({ baseUrl, request: createRequest, fetchImpl });
    await createProject({
      baseUrl,
      fetchImpl,
      idempotencyKey: createKey,
      antiForgery: created.nonce,
      request: {
        command_schema: createRequest.command_schema,
        prospective_project_id: created.prospective_project_id,
        create_project_input: createRequest.create_project_input,
      },
    });
    const projectId = created.prospective_project_id;
    const command = {
      baseUrl,
      fetchImpl,
      projectId,
    };

    const volumeRequest = {
      command_schema: "storyos.command.create-volume.request.v1" as const,
      create_volume_input: {
        title: "Volume A",
        expected_tree_revision: "1",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000d112",
      },
    };
    const volume = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/volumes",
      schema: volumeRequest.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d102",
      digest: await digestCreateVolume(volumeRequest),
      send: (antiForgery) => createVolume({
        ...command, idempotencyKey: "018f0000-0000-7001-8000-00000000d102", antiForgery,
        request: volumeRequest,
      }),
    });
    if (volume.result.effect.kind !== "authoritative_applied") {
      throw new Error("Volume A must apply");
    }

    const chapterRequest = {
      command_schema: "storyos.command.create-chapter.request.v1" as const,
      create_chapter_input: {
        title: "Chapter A",
        expected_tree_revision: "2",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000d113",
      },
    };
    const chapter = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
      schema: chapterRequest.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d103",
      digest: await digestCreateChapter(chapterRequest),
      send: (antiForgery) => createChapter({
        ...command, volumeId: volume.result.effect.volume_id,
        idempotencyKey: "018f0000-0000-7001-8000-00000000d103", antiForgery,
        request: chapterRequest,
      }),
    });
    if (chapter.result.effect.kind !== "authoritative_applied") {
      throw new Error("Chapter A must apply");
    }

    const request = exportRequest("018f0000-0000-7001-8000-00000000d114");
    const exportKey = "018f0000-0000-7001-8000-00000000d104";
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
    assert.equal(applied.result.acknowledgement, "accepted");
    assert.equal(applied.result.effect.kind, "admitted");
    if (applied.result.effect.kind !== "admitted") {
      throw new Error("Human-readable export must admit");
    }
    assert.equal("manuscript_utf8" in applied.result, false);
    const exportId = applied.result.effect.export_id;
    const sourceSnapshotId = applied.result.effect.source_snapshot.snapshot_id;

    const waiting = await getHumanReadableManuscriptExport({
      baseUrl, projectId, exportId, fetchImpl,
    });
    assert.equal(waiting.status, "in_progress");
    assert.equal("manuscript_utf8" in waiting, false);
    assert.equal(waiting.source_snapshot.snapshot_id, sourceSnapshotId);

    const replay = await exportHumanReadableManuscript({
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
        title: "Later Title",
        expected_project_revision: "1",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000d115",
      },
    };
    const renamed = await challenged({
      ...command,
      method: "PATCH",
      route: "/api/v1/projects/{project_id}",
      schema: renameProject.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d105",
      digest: await digestUpdateProject(renameProject),
      send: (antiForgery) => updateProject({
        ...command, idempotencyKey: "018f0000-0000-7001-8000-00000000d105", antiForgery,
        request: renameProject,
      }),
    });
    assert.equal(renamed.result.effect.kind, "authoritative_applied");

    const renameVolume = {
      command_schema: "storyos.command.update-volume.request.v1" as const,
      update_volume_input: {
        title: "Later Volume",
        order: "1",
        expected_tree_revision: "3",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000d116",
      },
    };
    const renamedVolume = await challenged({
      ...command,
      method: "PATCH",
      route: "/api/v1/projects/{project_id}/volumes/{volume_id}",
      schema: renameVolume.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d106",
      digest: await digestUpdateVolume(renameVolume),
      send: (antiForgery) => updateVolume({
        ...command, volumeId: volume.result.effect.volume_id,
        idempotencyKey: "018f0000-0000-7001-8000-00000000d106", antiForgery,
        request: renameVolume,
      }),
    });
    assert.equal(renamedVolume.result.effect.kind, "authoritative_applied");

    const renameChapter = {
      command_schema: "storyos.command.update-chapter.request.v1" as const,
      update_chapter_input: {
        title: "Later Chapter",
        order: "1",
        expected_tree_revision: "4",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000d117",
      },
    };
    const renamedChapter = await challenged({
      ...command,
      method: "PATCH",
      route: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
      schema: renameChapter.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d107",
      digest: await digestUpdateChapter(renameChapter),
      send: (antiForgery) => updateChapter({
        ...command, chapterId: chapter.result.effect.chapter_id,
        idempotencyKey: "018f0000-0000-7001-8000-00000000d107", antiForgery,
        request: renameChapter,
      }),
    });
    assert.equal(renamedChapter.result.effect.kind, "authoritative_applied");

    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      ...bindings(),
      correlation_id: "018f0000-0000-7001-8000-00000000d118",
    };
    const session = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/editor-sessions",
      schema: sessionRequest.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d108",
      digest: await digestCreateEditorSession(sessionRequest),
      send: (antiForgery) => createEditorSession({
        ...command, idempotencyKey: "018f0000-0000-7001-8000-00000000d108", antiForgery,
        request: sessionRequest,
      }),
    });
    if (session.result.writer.kind !== "current_writer") {
      throw new Error("the later Author Edit needs the current writer");
    }
    const from = session.result.base_snapshot.materialized_revision.body.length;
    const editRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      ...bindings(),
      correlation_id: "018f0000-0000-7001-8000-00000000d119",
      editor_session_id: session.result.editor_session.editor_session_id,
      writer_generation: session.result.writer.writer_generation,
      chapter_id: session.result.base_snapshot.chapter_id,
      expected_authoritative_revision_id: session.result.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids: session.result.base_snapshot.proposal_head_revision_ids,
      target_refs: session.result.base_snapshot.target_refs,
      observed_ownership_partition: session.result.base_snapshot.observed_ownership_partition,
      editor_contract_revision: "storyos.editor-contract.release-1.v2",
      undo_group_id: "018f0000-0000-7001-8000-00000000d1a1",
      completed_intent_record_id: "018f0000-0000-7001-8000-00000000d1a2",
      local_intent_sequence: "1",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from, to: from, text: "Hello world" }],
        selection_snapshot: {
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from,
        },
      }],
    };
    const edited = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/manuscript/author-edits",
      schema: editRequest.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d109",
      digest: await digestApplyAuthorEdit(editRequest),
      send: (antiForgery) => applyAuthorEdit({
        ...command, idempotencyKey: "018f0000-0000-7001-8000-00000000d109", antiForgery,
        request: editRequest,
      }),
    });
    assert.equal(edited.result.effect.kind, "authoritative_applied");

    const laterVolumeRequest = {
      command_schema: "storyos.command.create-volume.request.v1" as const,
      create_volume_input: {
        title: "Volume B",
        expected_tree_revision: "5",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000d11a",
      },
    };
    const laterVolume = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/volumes",
      schema: laterVolumeRequest.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d10a",
      digest: await digestCreateVolume(laterVolumeRequest),
      send: (antiForgery) => createVolume({
        ...command, idempotencyKey: "018f0000-0000-7001-8000-00000000d10a", antiForgery,
        request: laterVolumeRequest,
      }),
    });
    if (laterVolume.result.effect.kind !== "authoritative_applied") {
      throw new Error("Volume B must apply");
    }
    const laterChapterRequest = {
      command_schema: "storyos.command.create-chapter.request.v1" as const,
      create_chapter_input: {
        title: "Chapter B",
        expected_tree_revision: "6",
        ...bindings(),
        correlation_id: "018f0000-0000-7001-8000-00000000d11b",
      },
    };
    const laterChapter = await challenged({
      ...command,
      method: "POST",
      route: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
      schema: laterChapterRequest.command_schema,
      idempotencyKey: "018f0000-0000-7001-8000-00000000d10b",
      digest: await digestCreateChapter(laterChapterRequest),
      send: (antiForgery) => createChapter({
        ...command, volumeId: laterVolume.result.effect.volume_id,
        idempotencyKey: "018f0000-0000-7001-8000-00000000d10b", antiForgery,
        request: laterChapterRequest,
      }),
    });
    assert.equal(laterChapter.result.effect.kind, "authoritative_applied");

    await runStoryOSWorker({ repositoryRoot, workerBinary, args: ["--once"] });
    const ready = await getHumanReadableManuscriptExport({
      baseUrl, projectId, exportId, fetchImpl,
    });
    assert.equal(ready.status, "ready");
    if (ready.status !== "ready") {
      throw new Error("the Worker must settle the pinned human-readable export");
    }
    assert.equal(ready.manuscript_utf8, ADMITTED_MANUSCRIPT);
    assert.equal(ready.source_snapshot.snapshot_id, sourceSnapshotId);
    assert.equal(ready.export_id, exportId);
  } finally {
    await stopRealServer(server);
  }
});
