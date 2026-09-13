import assert from "node:assert/strict";
import type { ChildProcess } from "node:child_process";

import {
  createChapter,
  createProjectCommandChallenge,
  createVolume,
  digestCreateChapter,
  digestCreateVolume,
  digestUpdateProject,
  getProject,
  updateProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DigestValue } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  runStoryOSWorker,
  sessionFetch,
  withChallengeRetry,
} from "../support/node-integration.ts";

const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};

type StartedServer = { baseUrl: string; server: ChildProcess };
type SessionFetch = ReturnType<typeof sessionFetch>;
type ExportAck = {
  acknowledgement: string;
  project: { title: string; open: { kind: string } };
  effect: { kind: string; export_id?: string };
};
type Replay<Request, Response> = (options: {
  baseUrl: string;
  projectId: string;
  fetchImpl: SessionFetch;
  idempotencyKey: string;
  antiForgery: string;
  request: Request;
}) => Promise<Response>;
type Post<Request, Response> = (
  baseUrl: string,
  fetchImpl: SessionFetch,
  projectId: string,
  idempotencyKey: string,
  request: Request,
) => Promise<{ challenge: { nonce: string }; admitted: Response }>;
type Family<Request, Response extends ExportAck> = {
  startServer: () => Promise<StartedServer>;
  stopServer: (server: ChildProcess) => Promise<void>;
  createEmpty: (
    baseUrl: string,
    session: string,
    idempotencyKey: string,
    title: string,
  ) => Promise<{ fetchImpl: SessionFetch; projectId: string }>;
  exportRequest: (correlationId: string) => Request;
  postExport: Post<Request, Response>;
  replayExport: Replay<Request, Response>;
  operationsTable: string;
};

function capturingPost(inner: SessionFetch, exportPath: RegExp) {
  let lastPostBody = "";
  return {
    fetchImpl: (async (input, init) => {
      const response = await inner(input, init);
      const url = String(input instanceof Request ? input.url : input);
      if (response.ok && (init?.method ?? "GET") === "POST" && exportPath.test(url)) {
        lastPostBody = await response.clone().text();
      }
      return response;
    }) as SessionFetch,
    lastPostBody: () => lastPostBody,
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

async function challenged<Result>(options: {
  baseUrl: string;
  fetchImpl: SessionFetch;
  projectId: string;
  method: string;
  route: string;
  schema: string;
  digest: DigestValue;
  key: string;
  send: (nonce: string) => Promise<Result>;
}): Promise<Result> {
  const challenge = await withChallengeRetry(() => createProjectCommandChallenge({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    fetchImpl: options.fetchImpl,
    request: {
      method: options.method,
      route_template: options.route,
      command_schema: options.schema,
      canonical_command_digest: options.digest,
      idempotency_key: options.key,
    },
  }));
  return options.send(challenge.nonce);
}

async function applyFirstCurrentChapter(
  baseUrl: string,
  fetchImpl: SessionFetch,
  projectId: string,
  ids: { volumeCorrelation: string; volumeKey: string; chapterCorrelation: string; chapterKey: string },
): Promise<void> {
  const volumeRequest = {
    command_schema: "storyos.command.create-volume.request.v1" as const,
    create_volume_input: { title: "Volume A", expected_tree_revision: "1", ...BINDING, correlation_id: ids.volumeCorrelation },
  };
  const volume = await challenged({
    baseUrl, fetchImpl, projectId,
    method: "POST", route: "/api/v1/projects/{project_id}/volumes",
    schema: volumeRequest.command_schema, digest: await digestCreateVolume(volumeRequest),
    key: ids.volumeKey,
    send: (antiForgery) => createVolume({
      baseUrl, projectId, fetchImpl, idempotencyKey: ids.volumeKey, antiForgery, request: volumeRequest,
    }),
  });
  if (volume.effect.kind !== "authoritative_applied") throw new Error("Create Volume must apply");
  const volumeId = volume.effect.volume_id;
  const chapterRequest = {
    command_schema: "storyos.command.create-chapter.request.v1" as const,
    create_chapter_input: { title: "Chapter A", expected_tree_revision: "2", ...BINDING, correlation_id: ids.chapterCorrelation },
  };
  const chapter = await challenged({
    baseUrl, fetchImpl, projectId,
    method: "POST", route: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
    schema: chapterRequest.command_schema, digest: await digestCreateChapter(chapterRequest),
    key: ids.chapterKey,
    send: (antiForgery) => createChapter({
      baseUrl, projectId, volumeId, fetchImpl,
      idempotencyKey: ids.chapterKey, antiForgery, request: chapterRequest,
    }),
  });
  if (chapter.effect.kind !== "authoritative_applied") throw new Error("Create Chapter must apply");
}

async function countRows(table: string, projectId: string, extra = ""): Promise<string> {
  return queryPostgres(`SELECT count(*) FROM storyos.${table} WHERE project_id = '${projectId}'::uuid${extra};`);
}

export async function assertExportAdmissionFreezes<Request, Response extends ExportAck>(options: Family<Request, Response> & {
  exportPath: RegExp;
  getOperation: (options: {
    baseUrl: string;
    projectId: string;
    exportId: string;
    fetchImpl: SessionFetch;
  }) => Promise<{ status: string }>;
  admitMessage: string;
  titles: { original: string; later: string };
  ids: {
    projectKey: string;
    exportCorrelation: string;
    exportKey: string;
    volumeCorrelation: string;
    volumeKey: string;
    chapterCorrelation: string;
    chapterKey: string;
    renameCorrelation: string;
    renameKey: string;
  };
  repositoryRoot: string;
  workerBinary: string;
}): Promise<void> {
  let { baseUrl, server } = await options.startServer();
  try {
    const first = await options.createEmpty(baseUrl, "session-a", options.ids.projectKey, options.titles.original);
    const request = options.exportRequest(options.ids.exportCorrelation);
    const firstCapture = capturingPost(first.fetchImpl, options.exportPath);
    const applied = await options.postExport(
      baseUrl, firstCapture.fetchImpl, first.projectId, options.ids.exportKey, request,
    );
    const firstBody = firstCapture.lastPostBody();
    const exportId = applied.admitted.effect.export_id;
    assert.equal(applied.admitted.acknowledgement, "accepted");
    assert.equal(applied.admitted.project.title, options.titles.original);
    assert.equal(applied.admitted.project.open.kind, "empty");
    if (applied.admitted.effect.kind !== "admitted" || exportId === undefined) {
      throw new Error(options.admitMessage);
    }
    await applyFirstCurrentChapter(baseUrl, first.fetchImpl, first.projectId, options.ids);
    const renameRequest = {
      command_schema: "storyos.command.update-project.request.v1" as const,
      update_project_input: {
        title: options.titles.later, expected_project_revision: "1", ...BINDING,
        correlation_id: options.ids.renameCorrelation,
      },
    };
    const renamed = await challenged({
      baseUrl, fetchImpl: first.fetchImpl, projectId: first.projectId,
      method: "PATCH", route: "/api/v1/projects/{project_id}",
      schema: renameRequest.command_schema, digest: await digestUpdateProject(renameRequest),
      key: options.ids.renameKey,
      send: (antiForgery) => updateProject({
        baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl,
        idempotencyKey: options.ids.renameKey, antiForgery, request: renameRequest,
      }),
    });
    assert.equal(renamed.project.title, options.titles.later);
    const opened = await getProject({ baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl });
    assert.equal(opened.project.title, options.titles.later);
    assert.equal(opened.project.open.kind, "current_chapter");
    const replay = async (fetchImpl: SessionFetch) => {
      const capture = capturingPost(fetchImpl, options.exportPath);
      const response = await options.replayExport({
        baseUrl, projectId: first.projectId, fetchImpl: capture.fetchImpl,
        idempotencyKey: options.ids.exportKey, antiForgery: applied.challenge.nonce, request,
      });
      assert.deepEqual(response, applied.admitted);
      assert.equal(capture.lastPostBody(), firstBody);
      return response;
    };
    await replay(first.fetchImpl);
    assert.equal((await options.getOperation({
      baseUrl, projectId: first.projectId, exportId, fetchImpl: first.fetchImpl,
    })).status, "in_progress");
    assert.equal(await countRows(options.operationsTable, first.projectId), "1");
    assert.equal(await countRows("pinned_export_sources", first.projectId), "1");
    await runStoryOSWorker({
      repositoryRoot: options.repositoryRoot, workerBinary: options.workerBinary, args: ["--once"],
    });
    const settled = await replay(first.fetchImpl);
    assert.equal(settled.acknowledgement, "accepted");
    assert.equal(settled.project.title, options.titles.original);
    assert.equal(settled.project.open.kind, "empty");
    assert.equal((await options.getOperation({
      baseUrl, projectId: first.projectId, exportId, fetchImpl: first.fetchImpl,
    })).status, "ready");
    assert.equal(await countRows(options.operationsTable, first.projectId), "1");
    await options.stopServer(server);
    ({ baseUrl, server } = await options.startServer());
    await replay(sessionFetch(baseUrl, "session-a"));
  } finally {
    await options.stopServer(server);
  }
}

export async function assertExportHistoricalEvidence<Request, Response extends ExportAck>(options: Family<Request, Response> & {
  commandKind: string;
  title: string;
  ids: { projectKey: string; exportCorrelation: string; exportKey: string };
}): Promise<void> {
  const { baseUrl, server } = await options.startServer();
  try {
    const first = await options.createEmpty(baseUrl, "session-a", options.ids.projectKey, options.title);
    const request = options.exportRequest(options.ids.exportCorrelation);
    const applied = await options.postExport(
      baseUrl, first.fetchImpl, first.projectId, options.ids.exportKey, request,
    );
    assert.equal(applied.admitted.project.title, options.title);
    const novels = `SELECT title || ' ' || count(*)::text FROM storyos.projects WHERE project_id = '${first.projectId}'::uuid GROUP BY title;`;
    const novelsBefore = await queryPostgres(novels);
    const operationsBefore = await countRows(options.operationsTable, first.projectId);
    const keysExtra = ` AND command_kind = '${options.commandKind}'`;
    const keysBefore = await countRows("command_idempotency", first.projectId, keysExtra);
    const pinnedBefore = await countRows("pinned_export_sources", first.projectId);
    const replay = () => options.replayExport({
      baseUrl, projectId: first.projectId, fetchImpl: first.fetchImpl,
      idempotencyKey: options.ids.exportKey, antiForgery: applied.challenge.nonce, request,
    });
    const writeCapture = (format: string | null, payload: string) => queryPostgres(`
      UPDATE storyos.command_idempotency
         SET acknowledgement_format = ${format === null ? "NULL" : `'${format}'`},
             response_project = ${payload}
       WHERE project_id = '${first.projectId}'::uuid
         AND idempotency_key = '${options.ids.exportKey}'::uuid;
    `);
    await writeCapture(null, "NULL");
    await assert.rejects(replay(), (error) => {
      const protocol = requireStoryOSProtocolError(error);
      return protocol.status === 409 && problemCode(error) === "historical_acknowledgement_unavailable";
    });
    await writeCapture("command_response_project.v1", `'{"broken":true}'::jsonb`);
    await assert.rejects(replay(), (error) => {
      const protocol = requireStoryOSProtocolError(error);
      return protocol.status === 503 && problemCode(error) !== "historical_acknowledgement_unavailable";
    });
    assert.equal(await queryPostgres(novels), novelsBefore);
    assert.equal(await countRows(options.operationsTable, first.projectId), operationsBefore);
    assert.equal(await countRows("command_idempotency", first.projectId, keysExtra), keysBefore);
    assert.equal(await countRows("pinned_export_sources", first.projectId), pinnedBefore);
  } finally {
    await options.stopServer(server);
  }
}
