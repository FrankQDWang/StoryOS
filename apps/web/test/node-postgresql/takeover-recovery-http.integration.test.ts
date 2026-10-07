// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/readable-export-pinned-source-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import type { ChildProcess } from "node:child_process";
import { once } from "node:events";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import {
  applyAuthorEdit, createChapter, createEditorSession, createProjectCommandChallenge,
  createVolume, digestApplyAuthorEdit, digestCreateChapter, digestCreateEditorSession,
  digestCreateVolume, digestTakeOverProjectWriter, getApplyAuthorEditOutcome, getEditorSession,
  takeOverProjectWriter,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ApplyAuthorEditRequest, CreateEditorSessionResponse, DigestValue, GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  createEmptyProject,
  queryStoryOSPostgres as queryPostgres,
  requireStoryOSProtocolError,
  sessionFetch,
  startStoryOSServer,
  stopStoryOSServer,
  withChallengeBudget,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "release-package", process.platform === "win32"
  ? "storyos-server.exe" : "storyos-server");
const USER_A = "018f0000-0000-7001-8000-000000000001";
const BINDINGS = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};
const AUTHOR_EDIT_ROUTE = "/api/v1/projects/{project_id}/manuscript/author-edits";

type Schedule = "concurrent" | "server-cut";

/** One Server process and the fetch of the bootstrap session. */
interface Host { baseUrl: string; server: ChildProcess; fetchImpl: typeof fetch }

async function startHost(): Promise<Host> {
  const { baseUrl, server } = await startStoryOSServer({
    repositoryRoot, serverBinary, sessions: { "session-a": USER_A },
  });
  return { baseUrl, server, fetchImpl: sessionFetch(baseUrl, "session-a") };
}

async function challenge(host: Host, projectId: string, route: string, schema: string,
  digest: DigestValue, idempotencyKey: string): Promise<string> {
  const issued = await withChallengeBudget(projectId, () => createProjectCommandChallenge({
    baseUrl: host.baseUrl, projectId, fetchImpl: host.fetchImpl,
    request: {
      method: "POST", route_template: route, command_schema: schema,
      canonical_command_digest: digest, idempotency_key: idempotencyKey,
    },
  }));
  return issued.nonce;
}

async function openSession(host: Host, projectId: string, id: (n: number) => string,
  n: number): Promise<CreateEditorSessionResponse> {
  const request = {
    command_schema: "storyos.command.create-editor-session.request.v1" as const,
    ...BINDINGS, correlation_id: id(n),
  };
  const nonce = await challenge(host, projectId, "/api/v1/projects/{project_id}/editor-sessions",
    request.command_schema, await digestCreateEditorSession(request), id(n + 1));
  return createEditorSession({
    baseUrl: host.baseUrl, projectId, fetchImpl: host.fetchImpl, request,
    idempotencyKey: id(n + 1), antiForgery: nonce,
  });
}

/** Creates a Project with one Chapter so that no other test shares its writer state. */
async function createChapterProject(host: Host, id: (n: number) => string): Promise<string> {
  const projectId = await createEmptyProject({
    baseUrl: host.baseUrl, fetchImpl: host.fetchImpl, createKey: id(1), correlationId: id(2),
    title: "Takeover Recovery Novel",
  });
  const command = { baseUrl: host.baseUrl, projectId, fetchImpl: host.fetchImpl };
  const volumeRequest = {
    command_schema: "storyos.command.create-volume.request.v1" as const,
    create_volume_input: {
      title: "Volume A", expected_tree_revision: "1", ...BINDINGS, correlation_id: id(3),
    },
  };
  const volume = await createVolume({
    ...command, request: volumeRequest, idempotencyKey: id(4),
    antiForgery: await challenge(host, projectId, "/api/v1/projects/{project_id}/volumes",
      volumeRequest.command_schema, await digestCreateVolume(volumeRequest), id(4)),
  });
  if (volume.effect.kind !== "authoritative_applied") throw new Error("Volume A must apply");
  const chapterRequest = {
    command_schema: "storyos.command.create-chapter.request.v1" as const,
    create_chapter_input: {
      title: "Chapter A", expected_tree_revision: "2", ...BINDINGS, correlation_id: id(5),
    },
  };
  const chapter = await createChapter({
    ...command, volumeId: volume.effect.volume_id, request: chapterRequest, idempotencyKey: id(6),
    antiForgery: await challenge(host, projectId,
      "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
      chapterRequest.command_schema, await digestCreateChapter(chapterRequest), id(6)),
  });
  if (chapter.effect.kind !== "authoritative_applied") throw new Error("Chapter A must apply");
  return projectId;
}

function editRequest(session: GetEditorSessionResponse | CreateEditorSessionResponse,
  text: string, id: (n: number) => string, n: number): ApplyAuthorEditRequest {
  if (session.writer.kind !== "current_writer") throw new Error("the edit needs the current writer");
  const from = session.base_snapshot.materialized_revision.body.length;
  return {
    command_schema: "storyos.command.apply-author-edit.request.v1", ...BINDINGS,
    correlation_id: id(n), editor_session_id: session.editor_session.editor_session_id,
    writer_generation: session.writer.writer_generation, chapter_id: session.base_snapshot.chapter_id,
    expected_authoritative_revision_id: session.base_snapshot.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: session.base_snapshot.proposal_head_revision_ids,
    target_refs: session.base_snapshot.target_refs,
    observed_ownership_partition: session.base_snapshot.observed_ownership_partition,
    editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: id(n + 1), completed_intent_record_id: id(n + 2), local_intent_sequence: "1",
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from, to: from, text }],
      selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from },
    }],
  };
}

/** Holds the Chapter Head row lock in a separate PostgreSQL session until release. */
async function holdChapterHead(projectId: string, chapterId: string): Promise<() => Promise<void>> {
  const holder = spawn("docker", ["exec", "-i", process.env.STORYOS_TEST_POSTGRES_CONTAINER ?? "",
    "psql", "-X", "-qAt", "-U", "postgres", "-v", "ON_ERROR_STOP=1"], { stdio: ["pipe", "pipe", "pipe"] });
  holder.stdin.write(`BEGIN; SELECT 1 FROM storyos.authoritative_heads
    WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid
      AND manuscript_object_id = '${chapterId}'::uuid FOR UPDATE; SELECT 'locked';\n`);
  let output = "";
  while (!output.includes("locked")) output += (await once(holder.stdout, "data"))[0].toString();
  return async () => {
    const exited = once(holder, "exit");
    holder.stdin.end("ROLLBACK;\n");
    await exited;
  };
}

async function waitForAdmittedCoreOnHeadLock(): Promise<void> {
  for (let attempt = 0; attempt < 200; attempt += 1) {
    if (await queryPostgres(`SELECT count(*) FROM pg_stat_activity WHERE usename = 'storyos_runtime'
      AND wait_event_type = 'Lock' AND query LIKE 'SELECT head.current_revision_id::text,%'
      AND query LIKE '%FROM storyos.author_command_admissions AS admission%'`) !== "0") return;
  }
  throw new Error("the admitted Author Edit did not wait for the Chapter Head lock");
}

async function authority(projectId: string): Promise<{ commits: number; admissions: number }> {
  return JSON.parse(await queryPostgres(`SELECT json_build_object(
    'commits', (SELECT count(*) FROM storyos.authoritative_commits
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid),
    'admissions', (SELECT count(*) FROM storyos.author_command_admissions
      WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid
        AND command_kind = 'applyAuthorEdit'))::text`));
}

async function proveTakeoverRefusesAdmittedRecovery(schedule: Schedule): Promise<void> {
  const id = (n: number) => `018f0000-0000-7001-8000-0001024${schedule === "concurrent" ? 1 : 2}${
    String(n).padStart(4, "0")}`;
  let host = await startHost();
  try {
    const projectId = await createChapterProject(host, id);
    const writer = await openSession(host, projectId, id, 10);
    const observer = await openSession(host, projectId, id, 12);
    if (writer.writer.kind !== "current_writer" || observer.writer.kind !== "read_only") {
      throw new Error("the fixture needs one current writer and one read-only observer");
    }
    const oldRequest = editRequest(writer, "OLD", id, 20);
    const oldNonce = await challenge(host, projectId, AUTHOR_EDIT_ROUTE, oldRequest.command_schema,
      await digestApplyAuthorEdit(oldRequest), id(23));
    const takeoverRequest = {
      command_schema: "storyos.command.take-over-project-writer.request.v1" as const, ...BINDINGS,
      correlation_id: id(30), editor_session_id: observer.editor_session.editor_session_id,
      observed_writer_generation: writer.writer.writer_generation,
      editor_contract_revision: "storyos.editor-contract.release-1.v3",
    };
    const takeoverNonce = await challenge(host, projectId,
      "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers",
      takeoverRequest.command_schema, await digestTakeOverProjectWriter(takeoverRequest), id(31));
    const before = await authority(projectId);

    const releaseHead = await holdChapterHead(projectId, writer.base_snapshot.chapter_id);
    const oldReply = applyAuthorEdit({
      baseUrl: host.baseUrl, projectId, fetchImpl: host.fetchImpl, request: oldRequest,
      idempotencyKey: id(23), antiForgery: oldNonce,
    }).then(() => undefined, (error: unknown) => error);
    await waitForAdmittedCoreOnHeadLock();
    if (schedule === "server-cut") {
      const exited = once(host.server, "exit");
      host.server.kill("SIGKILL");
      await exited;
      const cut = await oldReply;
      assert.ok(cut instanceof Error && !("status" in cut), "the cut must end the old request");
      host = await startHost();
    }
    const takeover = await takeOverProjectWriter({
      baseUrl: host.baseUrl, projectId, fetchImpl: host.fetchImpl,
      editorSessionId: observer.editor_session.editor_session_id, request: takeoverRequest,
      idempotencyKey: id(31), antiForgery: takeoverNonce,
    });
    if (takeover.result.kind !== "takeover_applied") throw new Error("the takeover must apply");
    await releaseHead();
    if (schedule === "concurrent") {
      assert.equal(requireStoryOSProtocolError(await oldReply).status, 503);
    }

    const outcomeOptions = {
      baseUrl: host.baseUrl, projectId, fetchImpl: host.fetchImpl,
      idempotencyKey: id(23), antiForgery: oldNonce,
    };
    const admission = JSON.parse(await queryPostgres(`SELECT json_build_object(
      'command_id', command_id::text, 'author_command_admission_id', author_command_admission_id::text)::text
      FROM storyos.author_command_admissions WHERE owner_user_id = '${USER_A}'::uuid
        AND project_id = '${projectId}'::uuid AND idempotency_key = '${id(23)}'::uuid`));
    const expectedOutcome = {
      outcome_kind: "requires_reconfirmation", ...admission,
      reconfirmation_reason: "binding_changed", recovery_draft_ref: null,
    };
    assert.deepEqual((await getApplyAuthorEditOutcome(outcomeOptions)).outcome, expectedOutcome);
    assert.deepEqual((await getApplyAuthorEditOutcome(outcomeOptions)).outcome, expectedOutcome);
    assert.deepEqual(await authority(projectId), { ...before, admissions: before.admissions + 1 });

    const winner = await getEditorSession({
      baseUrl: host.baseUrl, projectId, fetchImpl: host.fetchImpl,
      editorSessionId: observer.editor_session.editor_session_id,
    });
    assert.deepEqual(winner.writer, {
      kind: "current_writer", writer_generation: takeover.result.resulting_writer_generation,
    });
    assert.deepEqual(winner.base_snapshot.authoritative_head_revision_id,
      writer.base_snapshot.authoritative_head_revision_id);
    const nextRequest = editRequest(winner, "WIN", id, 40);
    const next = await applyAuthorEdit({
      baseUrl: host.baseUrl, projectId, fetchImpl: host.fetchImpl, request: nextRequest,
      idempotencyKey: id(43), antiForgery: await challenge(host, projectId, AUTHOR_EDIT_ROUTE,
        nextRequest.command_schema, await digestApplyAuthorEdit(nextRequest), id(43)),
    });
    if (next.effect.kind !== "authoritative_applied") {
      throw new Error(`the winner's next edit must apply: ${JSON.stringify(next.effect)}`);
    }
    assert.equal(next.effect.authoritative_revision.body,
      `${writer.base_snapshot.materialized_revision.body}WIN`);
    assert.deepEqual(await authority(projectId),
      { commits: before.commits + 1, admissions: before.admissions + 2 });
  } finally {
    await stopStoryOSServer(host.server);
  }
}

test("an admitted Author Edit that waits through a concurrent takeover requires reconfirmation",
  async () => { await proveTakeoverRefusesAdmittedRecovery("concurrent"); });

test("an admitted Author Edit cut by a Server stop before a takeover requires reconfirmation",
  async () => { await proveTakeoverRefusesAdmittedRecovery("server-cut"); });
