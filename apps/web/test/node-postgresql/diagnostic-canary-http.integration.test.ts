// Verification: {"phase":"http-main","after":[]}
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
  searchManuscript,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest,
  CreateEditorSessionRequest,
  DigestValue,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
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
  withChallengeBudget,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const exe = process.platform === "win32" ? ".exe" : "";
const serverBinary = join(repositoryRoot, "target", "release-package", `storyos-server${exe}`);
const USER_A = "018f0000-0000-7001-8000-000000000001";
const CLIENT = RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision;
const SECURITY = "storyos.web-security-policy.release-1.v1";

function bindings() {
  return { client_contract_revision: CLIENT, security_policy_revision: SECURITY };
}

function id(suffix: string): string {
  return `018f0000-0000-7001-8000-0000000a47${suffix}`;
}

async function challenged<Result>(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  projectId: string;
  route: string;
  schema: string;
  idempotencyKey: string;
  digest: DigestValue;
  send: (antiForgery: string) => Promise<Result>;
}): Promise<Result> {
  const challenge = await withChallengeBudget(options.projectId, () => createProjectCommandChallenge({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    fetchImpl: options.fetchImpl,
    request: {
      method: "POST",
      route_template: options.route,
      command_schema: options.schema,
      canonical_command_digest: options.digest,
      idempotency_key: options.idempotencyKey,
    },
  }));
  return options.send(challenge.nonce);
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
    serverBinary,
    sessions: { "session-a": USER_A },
    databaseUrl,
    extraEnv: { STORYOS_LOG: "debug", STORYOS_CHALLENGE_SECRET: secret },
  });
  try {
    const fetchImpl = browserFetch(baseUrl, "session-a");
    const createKey = id("01");
    const createRequest = {
      command_schema: "storyos.command.create-project.request.v1" as const,
      create_project_input: { title, ...bindings(), correlation_id: id("11") },
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
      create_volume_input: { title, expected_tree_revision: "1", ...bindings(), correlation_id: id("12") },
    };
    const volume = await challenged({
      ...command,
      route: "/api/v1/projects/{project_id}/volumes",
      schema: volumeRequest.command_schema,
      idempotencyKey: id("02"),
      digest: await digestCreateVolume(volumeRequest),
      send: (antiForgery) => createVolume({
        ...command, idempotencyKey: id("02"), antiForgery, request: volumeRequest,
      }),
    });
    if (volume.effect.kind !== "authoritative_applied") throw new Error("the volume must apply");
    const volumeId = volume.effect.volume_id;
    const chapterRequest = {
      command_schema: "storyos.command.create-chapter.request.v1" as const,
      create_chapter_input: { title, expected_tree_revision: "2", ...bindings(), correlation_id: id("13") },
    };
    const chapter = await challenged({
      ...command,
      route: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
      schema: chapterRequest.command_schema,
      idempotencyKey: id("03"),
      digest: await digestCreateChapter(chapterRequest),
      send: (antiForgery) => createChapter({
        ...command, volumeId, idempotencyKey: id("03"), antiForgery,
        request: chapterRequest,
      }),
    });
    assert.equal(chapter.effect.kind, "authoritative_applied");

    const sessionRequest: CreateEditorSessionRequest = {
      command_schema: "storyos.command.create-editor-session.request.v1",
      ...bindings(),
      correlation_id: id("14"),
    };
    const session = await challenged({
      ...command,
      route: "/api/v1/projects/{project_id}/editor-sessions",
      schema: sessionRequest.command_schema,
      idempotencyKey: id("04"),
      digest: await digestCreateEditorSession(sessionRequest),
      send: (antiForgery) => createEditorSession({
        ...command, idempotencyKey: id("04"), antiForgery, request: sessionRequest,
      }),
    });
    if (session.writer.kind !== "current_writer") throw new Error("the edit needs the current writer");
    const from = session.base_snapshot.materialized_revision.body.length;
    const editRequest: ApplyAuthorEditRequest = {
      command_schema: "storyos.command.apply-author-edit.request.v1",
      ...bindings(),
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
    const edited = await challenged({
      ...command,
      route: "/api/v1/projects/{project_id}/manuscript/author-edits",
      schema: editRequest.command_schema,
      idempotencyKey: id("05"),
      digest: await digestApplyAuthorEdit(editRequest),
      send: (antiForgery) => {
        editNonce = antiForgery;
        return applyAuthorEdit({ ...command, idempotencyKey: id("05"), antiForgery, request: editRequest });
      },
    });
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
