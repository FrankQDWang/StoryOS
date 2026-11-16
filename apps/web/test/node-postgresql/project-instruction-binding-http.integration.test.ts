// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { afterAll, test } from "vitest";

import { createStory, drainLeftoverWork, inspectRun, postRun, runRequest, selected, serverForFile, settleRun,
} from "../support/http-harness.ts";
import type { TestStory } from "../support/http-harness.ts";
import { queryStoryOSPostgres } from "../support/node-integration.ts";

// Each Run binds the current Project Instruction Revision, and getAgentRun shows it (ADR 0094 section 2).
const server = serverForFile();
afterAll(() => drainLeftoverWork());

async function save(story: TestStory, text: string, expected: string): Promise<string> {
  const saved = await story.send("updateProjectInstruction", { request: {
    command_schema: "storyos.command.update-project-instruction.request.v1",
    update_project_instruction_input: { text, expected_instruction_revision: expected, ...story.binding() },
  } });
  const { effect } = saved.response;
  if (effect.kind !== "authoritative_applied") throw new Error("the save must apply");
  return effect.instruction_revision_id;
}

/** Admits one Run in a new Conversation, and runs the Worker until the Run settles. */
async function settled(story: TestStory) {
  const request = runRequest(story.chapterIds[0] ?? "", story.key(), "Help with this passage.");
  const created = await postRun(story.baseUrl, story.fetchImpl, story.projectId, story.key(), request);
  if (created.effect.kind !== "admitted") throw new Error("the Run must be admitted");
  return settleRun(story.baseUrl, story, created.effect.run_id);
}

/** The text of each instruction_binding item of the wire payload of the first decision Attempt of the Run. */
async function sent(runId: string): Promise<string[]> {
  return JSON.parse(await queryStoryOSPostgres(`
    SELECT jsonb_path_query_array(wire, '$.context[*] ? (@.source_class == "instruction_binding").content')::text
      FROM storyos.model_attempts,
           LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
     WHERE run_id = '${runId}'::uuid AND attempt_role = 'decision' AND decision_position = 0`)) as string[];
}

test("each Run binds the current Project Instruction, and getAgentRun shows the binding", async () => {
  await drainLeftoverWork();
  const story = await createStory(server, { title: "Bound Instruction Novel", assistance: true });
  const firstText = "Write in the past tense.";
  const firstRevision = await save(story, firstText, "0");
  const first = await settled(story);
  const nextText = "Write in the present tense.";
  const nextRevision = await save(story, nextText, "1");
  const next = await settled(story);
  const source = (run: typeof first) =>
    run.context.considered.find((item) => item.source_class === "instruction_binding");

  assert.deepEqual([first.status, next.status], ["completed", "completed"]);
  assert.deepEqual(source(first), { source_class: "instruction_binding", source_version: firstRevision,
    token_count: String(firstText.length), eligible: true });
  assert.deepEqual([selected(first, "instruction_binding"), await sent(first.run_id)], [firstText, [firstText]]);
  assert.deepEqual(source(next)?.source_version, nextRevision);
  assert.deepEqual([selected(next, "instruction_binding"), await sent(next.run_id)], [nextText, [nextText]]);
  // The save after the first Run does not change its read.
  assert.deepEqual((await inspectRun(story.baseUrl, story, first.run_id)).context, first.context);
});
