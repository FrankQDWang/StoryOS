// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  applyAuthorEdit,
  createChapter,
  createEditorSession,
  createProject,
  createProjectChallenge,
  createVolume,
  digestApplyAuthorEdit,
  digestCreateChapter,
  digestCreateEditorSession,
  digestCreateVolume,
  searchManuscript,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateEditorSessionRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  assertNoCanary,
  canary,
  canaryDatabaseUrl,
  closedSpans,
  diagnosticLines,
} from "../support/diagnostic-canary.ts";
import {
  requireStoryOSProtocolError,
  sessionFetch as browserFetch,
  startStoryOSServer,
  stopStoryOSServer as stopRealServer,
} from "../support/node-integration.ts";
import { BINDING, USER_A, challenged, packaged } from "../support/http-harness.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));

function id(suffix: string): string {
  return `018f0000-0000-7001-8000-0000000a47${suffix}`;
}

test("the Diagnostic Projection of HTTP requests holds no author text and no secret", async () => {
  const title = canary("title");
  const text = canary("text");
  const query = canary("query");
  const secret = canary("challenge-secret");
  const applicationName = canary("database").replaceAll("-", "_");
  const databaseUrl = canaryDatabaseUrl(applicationName);
  let editCommandId = "";
  const { baseUrl, server, stderr } = await startStoryOSServer({
    repositoryRoot,
    serverBinary: packaged("storyos-server"),
    sessions: { "session-a": USER_A },
    databaseUrl,
    extraEnv: { STORYOS_LOG: "debug", STORYOS_CHALLENGE_SECRET: secret },
  });
  try {
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const createKey = id("01");
    const createRequest = {
      command_schema: "storyos.command.create-project.request.v1" as const,
      create_project_input: { title, ...BINDING, correlation_id: id("11") },
      idempotency_key: createKey,
    };
    const created = await createProjectChallenge({ baseUrl, request: createRequest, fetchImpl });
    const create = (antiForgery: string) => createProject({
      baseUrl,
      fetchImpl,
      idempotencyKey: createKey,
      antiForgery,
      request: {
        command_schema: createRequest.command_schema,
        prospective_project_id: created.prospective_project_id,
        create_project_input: createRequest.create_project_input,
      },
    });
    const first = await create(created.nonce);
    const retried = await create(created.nonce);
    assert.deepEqual(retried, first);
    await assert.rejects(create(canary("nonce")), (error) => (requireStoryOSProtocolError(error).status ?? 0) >= 400);
    const projectId = created.prospective_project_id;
    const command = { baseUrl, fetchImpl, projectId };

    const volumeRequest = {
      command_schema: "storyos.command.create-volume.request.v1" as const,
      create_volume_input: { title, expected_tree_revision: "1", ...BINDING, correlation_id: id("12") },
    };
    const volume = await challenged(
      baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/volumes", volumeRequest.command_schema,
      await digestCreateVolume(volumeRequest), id("02"),
      (antiForgery) => createVolume({
        ...command, idempotencyKey: id("02"), antiForgery, request: volumeRequest,
      }),
    );
    if (volume.effect.kind !== "authoritative_applied") throw new Error("the volume must apply");
    const volumeId = volume.effect.volume_id;
    const chapterRequest = {
      command_schema: "storyos.command.create-chapter.request.v1" as const,
      create_chapter_input: { title, expected_tree_revision: "2", ...BINDING, correlation_id: id("13") },
    };
    const chapter = await challenged(
      baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
      chapterRequest.command_schema, await digestCreateChapter(chapterRequest), id("03"),
      (antiForgery) => createChapter({
        ...command, volumeId, idempotencyKey: id("03"), antiForgery,
        request: chapterRequest,
      }),
    );
    assert.equal(chapter.effect.kind, "authoritative_applied");

    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      ...BINDING,
      correlation_id: id("14"),
    };
    const session = await challenged(
      baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/editor-sessions",
      sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest), id("04"),
      (antiForgery) => createEditorSession({
        ...command, idempotencyKey: id("04"), antiForgery, request: sessionRequest,
      }),
    );
    if (session.writer.kind !== "current_writer") throw new Error("the edit needs the current writer");
    const from = session.base_snapshot.materialized_revision.body.length;
    const editRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      ...BINDING,
      correlation_id: id("15"),
      editor_session_id: session.editor_session.editor_session_id,
      writer_generation: session.writer.writer_generation,
      chapter_id: session.base_snapshot.chapter_id,
      expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
      expected_proposal_head_revision_ids: session.base_snapshot.proposal_head_revision_ids,
      target_refs: session.base_snapshot.target_refs,
      observed_ownership_partition: session.base_snapshot.observed_ownership_partition,
      editor_contract_revision: "storyos.editor-contract.release-1.v3",
      undo_group_id: id("21"),
      completed_intent_record_id: id("22"),
      local_intent_sequence: "1",
      author_edit_units: [{
        normalized_primitives: [{ kind: "replace_selection", from, to: from, text }],
        selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from },
      }],
    };
    let editNonce = "";
    const edited = await challenged(
      baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/manuscript/author-edits",
      editRequest.command_schema, await digestApplyAuthorEdit(editRequest), id("05"),
      (antiForgery) => {
        editNonce = antiForgery;
        return applyAuthorEdit({ ...command, idempotencyKey: id("05"), antiForgery, request: editRequest });
      },
    );
    assert.equal(edited.effect.kind, "authoritative_applied");
    const replayedEdit = await applyAuthorEdit({
      ...command, idempotencyKey: id("05"), antiForgery: editNonce, request: editRequest,
    });
    assert.equal(replayedEdit.command_id, edited.command_id);
    editCommandId = edited.command_id;

    await searchManuscript({
      ...command,
      request: {
        schema_id: "storyos.query.manuscript-search.request.v1",
        selection: "manuscript",
        query_text: query,
        required_watermark: null,
      },
    });
  } finally {
    await stopRealServer(server);
  }
  const output = stderr();
  assertNoCanary(output, [title, text, query, secret, applicationName, databaseUrl]);
  const lines = diagnosticLines(output);
  assert.deepEqual(lines.filter((line) => !line.target.startsWith("storyos")), []);
  const requests = closedSpans(lines, "request_span");
  const routes = requests.map((line) => line.span?.route);
  for (const route of [
    "/api/v1/projects",
    "/api/v1/projects/{project_id}/manuscript/author-edits",
    "/api/v1/projects/{project_id}/queries/manuscript-search",
  ]) {
    assert.ok(routes.includes(route), `no request span for ${route} in ${JSON.stringify(routes)}`);
  }
  assert.ok(requests.every((line) => typeof line.span?.status === "number"), output);
  // The Author Edit admits and settles in one span, and its exact retry replays the stored identifiers.
  const edits = closedSpans(lines, "admit_and_settle")
    .filter((line) => line.span?.correlation_id === id("15"));
  assert.deepEqual(
    edits.map((line) => [line.span?.outcome, line.span?.command_id]),
    [["authoritative_applied", editCommandId], ["replayed", editCommandId]],
  );
  assert.deepEqual(closedSpans(lines, "settle_admitted_command"), []);
  assert.deepEqual(closedSpans(lines, "replay_settled"), []);
});
