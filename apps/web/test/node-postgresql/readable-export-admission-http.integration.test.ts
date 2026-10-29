// Verification: {"phase":"http-main","after":["apps/web/test/node-postgresql/project-export-pinned-source-http.integration.test.ts"]}
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "vitest";

import { getHumanReadableManuscriptExport, getProject,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { exportSettlementReceipt, queryStoryOSPostgres } from "../support/node-integration.ts";
import { TestServer, USER_A, UUID_V7, addVolume, createProject, id, problemOf, serverForFile, settleOnce, waitFor,
} from "../support/http-harness.ts";
import type { CommandInput, TestProject } from "../support/http-harness.ts";

// The Profile Probe of export admission (ADR 0075). An export command is admit-only, so it has no conflict outcome.
const server = serverForFile();
const EMPTY_MANUSCRIPT = "\n";
const EMPTY_MANUSCRIPT_SHA256 = createHash("sha256").update(EMPTY_MANUSCRIPT).digest("hex");

function exportInput(project: TestProject): CommandInput<"exportHumanReadableManuscript"> {
  return { request: {
    command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
    export_human_readable_manuscript_input: project.binding(),
  } };
}

async function archive(project: TestProject) {
  const archived = await project.send("archiveProject", { request: {
    command_schema: "storyos.command.archive-project.request.v1",
    archive_project_input: { expected_project_revision: "1", ...project.binding() },
  } });
  assert.equal(archived.response.effect.kind, "authoritative_applied");
}

/** Admits one export and returns its exact retry and a reader of its operation. */
async function admitted(project: TestProject) {
  const sent = await project.send("exportHumanReadableManuscript", exportInput(project));
  if (sent.response.effect.kind !== "admitted") throw new Error("Human-readable export must admit");
  const exportId = sent.response.effect.export_id;
  const read = (fetchImpl = project.fetchImpl) => getHumanReadableManuscriptExport({ baseUrl: project.baseUrl,
    projectId: project.projectId, exportId, fetchImpl });
  return { sent, exportId, read };
}

/** Runs the Worker once at a time until the export leaves in_progress. Earlier tests can leave other export work. */
async function settled(read: () => ReturnType<typeof getHumanReadableManuscriptExport>) {
  return waitFor(async () => {
    await settleOnce();
    return read();
  }, (page) => page.status !== "in_progress", { attempts: 16, intervalMs: 0 });
}

async function settledActivityCount(project: TestProject): Promise<string> {
  return queryStoryOSPostgres(`
    SELECT count(*)::text FROM storyos.project_activity_event_payloads
     WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${project.projectId}'::uuid
       AND event_kind = 'human_readable_manuscript_export_settled';
  `);
}

test("exportHumanReadableManuscript admits, replays, and refuses", async () => {
  const project = await createProject(server, { title: "Readable Export Novel" });
  const { sent, exportId, read } = await admitted(project);
  const { effect } = sent.response;
  assert.equal(sent.response.schema_id, "storyos.command.export-human-readable-manuscript.response.v1");
  assert.equal(sent.response.acknowledgement, "accepted");
  assert.equal("receipt" in sent.response, false);
  if (effect.kind !== "admitted") throw new Error("Human-readable export must admit");
  assert.match(exportId, UUID_V7);
  assert.equal(effect.export_profile, "storyos.readable-export.utf8-lf.v1");
  assert.match(effect.source_snapshot.snapshot_id, UUID_V7);
  assert.equal(sent.response.operation_ref?.kind, "human_readable_manuscript_export");
  const inspected = await read();
  if (inspected.status !== "in_progress") throw new Error("a new human-readable export must stay in progress");
  assert.deepEqual([inspected.export_id, inspected.export_profile, inspected.source_snapshot.snapshot_id],
    [exportId, effect.export_profile, effect.source_snapshot.snapshot_id]);
  assert.deepEqual(["manuscript_utf8" in inspected, "content_sha256" in inspected], [false, false]);
  assert.equal(await settledActivityCount(project), "0");
  assert.deepEqual(await sent.command.send(), { response: sent.response, body: sent.body });

  const { status, code } = await problemOf(sent.command.send({ request: exportInput(project).request }));
  assert.deepEqual({ status, code }, { status: 422, code: "challenge_invalid" });
  const unknown = await problemOf(getHumanReadableManuscriptExport({ baseUrl: project.baseUrl,
    projectId: project.projectId, exportId: id("ae99"), fetchImpl: project.fetchImpl }));
  assert.equal(unknown.status, 404);
  const foreignRead = await problemOf(read(project.as("session-b").fetchImpl));
  const foreignAdmit = await problemOf(project.as("session-b").send("exportHumanReadableManuscript",
    exportInput(project)));
  for (const foreign of [foreignRead, foreignAdmit]) {
    assert.equal(foreign.status, 404);
    assert.ok(!foreign.body.includes(USER_A));
  }

  await archive(project);
  const refused = await project.challenge("exportHumanReadableManuscript", exportInput(project));
  assert.equal((await problemOf(refused.send())).status, 422);
  assert.equal((await problemOf(refused.send())).status, 422);
  assert.equal((await read()).status, "in_progress");
});

test("exportHumanReadableManuscript replays its first acknowledgement after later changes, settlement, and a Server restart",
  async () => {
    const project = await createProject(server, { title: "Readable Freeze Novel" });
    const { sent, read } = await admitted(project);
    assert.deepEqual([sent.response.project.title, sent.response.project.open.kind], ["Readable Freeze Novel", "empty"]);
    await addVolume(project, ["Chapter A"]);
    const renamed = await project.send("updateProject", { request: {
      command_schema: "storyos.command.update-project.request.v1",
      update_project_input: { title: "Later Readable Title", expected_project_revision: "1", ...project.binding() },
    } });
    assert.equal(renamed.response.effect.kind, "authoritative_applied");
    const later = (await getProject({ baseUrl: project.baseUrl, projectId: project.projectId,
      fetchImpl: project.fetchImpl })).project;
    assert.deepEqual([later.title, later.open.kind], ["Later Readable Title", "current_chapter"]);

    assert.deepEqual(await sent.command.send(), { response: sent.response, body: sent.body });
    assert.equal((await settled(read)).status, "ready");
    assert.deepEqual(await sent.command.send(), { response: sent.response, body: sent.body });
    await server.restart();
    assert.deepEqual(await sent.command.send(), { response: sent.response, body: sent.body });
  });

test("the in-process Worker settles an admitted human-readable export to ready", async () => {
  const worker = await TestServer.start({ STORYOS_WORKER: "1" });
  try {
    const project = await createProject(worker, { title: "Worker Novel" });
    const { sent, read } = await admitted(project);
    const ready = await waitFor(read, (page) => page.status === "ready");
    if (ready.status !== "ready") throw new Error("the Worker must settle the human-readable export");
    assert.deepEqual([ready.manuscript_utf8, ready.content_sha256], [EMPTY_MANUSCRIPT, EMPTY_MANUSCRIPT_SHA256]);
    assert.deepEqual(await sent.command.send(), { response: sent.response, body: sent.body });
    assert.equal((await read()).status, "ready");
    assert.equal(await settledActivityCount(project), "1");
  } finally {
    await worker.stop();
  }
});

test("the Worker settles failed when the Project is archived after admission", async () => {
  const project = await createProject(server, { title: "Archive After Novel" });
  const { sent, exportId, read } = await admitted(project);
  await archive(project);
  assert.equal((await read()).status, "in_progress");
  const failed = await settled(read);
  assert.equal(failed.status, "failed");
  assert.equal("manuscript_utf8" in failed, false);
  assert.equal(await exportSettlementReceipt({ ownerUserId: USER_A, projectId: project.projectId, exportId,
    operationsTable: "human_readable_manuscript_export_operations" }), "refused archived_project");
  assert.deepEqual(await sent.command.send(), { response: sent.response, body: sent.body });
  assert.equal((await read()).status, "failed");
});
