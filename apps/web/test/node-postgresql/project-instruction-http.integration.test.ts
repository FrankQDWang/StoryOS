// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { afterAll, test } from "vitest";

import { getProjectInstruction } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, archivedTables, assertForeignRefused, createStory, drainLeftoverWork, problemOf, serverForFile,
} from "../support/http-harness.ts";
import type { CommandInput, TestProject } from "../support/http-harness.ts";
import { queryStoryOSPostgres } from "../support/node-integration.ts";

// The save and the read of the Project Instruction at the HTTP seam (ADR 0094 section 2).
const server = serverForFile();
afterAll(() => drainLeftoverWork());

// The Context item limit of ADR 0052 in counted tokens.
const CONTEXT_ITEM_TOKEN_LIMIT = 32_000;

function save(project: TestProject, text: string, expected: string): CommandInput<"updateProjectInstruction"> {
  return { request: {
    command_schema: "storyos.command.update-project-instruction.request.v1",
    update_project_instruction_input: { text, expected_instruction_revision: expected, ...project.binding() },
  } };
}

async function read(project: TestProject) {
  return (await getProjectInstruction({ baseUrl: project.baseUrl, projectId: project.projectId,
    fetchImpl: project.fetchImpl })).instruction;
}

async function revisionRows(projectId: string): Promise<string> {
  return queryStoryOSPostgres(`SELECT count(*) FROM storyos.project_instruction_revisions
    WHERE project_id = '${projectId}'::uuid`);
}

test("updateProjectInstruction saves Revisions that getProjectInstruction and the archive read", async () => {
  await drainLeftoverWork();
  const story = await createStory(server, { title: "Instruction Novel" });
  assert.equal(await read(story), null);

  const first = await story.send("updateProjectInstruction", save(story, "Write in the past tense.", "0"));
  const { effect, receipt } = first.response;
  assert.deepEqual([first.response.schema_id, receipt.command_kind, receipt.result],
    ["storyos.command.update-project-instruction.response.v1", "updateProjectInstruction", "authoritative_applied"]);
  if (effect.kind !== "authoritative_applied") throw new Error("the first save must apply");
  assert.deepEqual(effect, { kind: "authoritative_applied", instruction_revision_id: effect.instruction_revision_id,
    revision: "1", project_activity_position: effect.project_activity_position });
  assert.ok(!first.body.includes("past tense"));
  assert.deepEqual(await read(story), { instruction_revision_id: effect.instruction_revision_id, revision: "1",
    text: "Write in the past tense." });
  assert.deepEqual(await first.command.send(), { response: first.response, body: first.body });

  const stale = await story.send("updateProjectInstruction", save(story, "Stale text.", "0"));
  assert.deepEqual(stale.response.effect, { kind: "conflicted", reason: "stale_instruction_revision" });
  const over = await problemOf(story.send("updateProjectInstruction",
    save(story, "字".repeat(CONTEXT_ITEM_TOKEN_LIMIT + 1), "1")));
  assert.deepEqual([over.status, over.code], [422, "instruction_over_item_limit"]);
  const nul = await problemOf(story.send("updateProjectInstruction", save(story, "A\u0000B", "1")));
  assert.deepEqual([nul.status, nul.code], [400, "invalid_request"]);

  const second = await story.send("updateProjectInstruction", save(story, "", "1"));
  if (second.response.effect.kind !== "authoritative_applied") throw new Error("the second save must apply");
  const secondId = second.response.effect.instruction_revision_id;
  assert.notEqual(secondId, effect.instruction_revision_id);
  assert.deepEqual(await read(story), { instruction_revision_id: secondId, revision: "2", text: "" });

  await assertForeignRefused(story, "updateProjectInstruction", save(story, "Foreign text.", "2"));
  const foreign = await problemOf(read(story.as("session-b")));
  assert.deepEqual([foreign.status, foreign.code], [404, "resource_unavailable"]);
  assert.ok(!foreign.body.includes(USER_A));
  assert.equal(await revisionRows(story.projectId), "2");

  const tables = await archivedTables(story);
  const archived = tables("project_instruction_revisions") as { instruction_revision_id: string; text: string }[];
  assert.deepEqual(archived.map((row) => [row.instruction_revision_id, row.text]).sort(),
    [[effect.instruction_revision_id, "Write in the past tense."], [secondId, ""]].sort());
});
