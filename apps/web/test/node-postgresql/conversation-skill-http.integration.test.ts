// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import type { SkillPackageReference } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { TestServer, createStory, drainLeftoverWork, postRun, settleRun, startServer } from "../support/http-harness.ts";
import { runRequest } from "../support/acceptance.ts";
import { queryStoryOSPostgres, stopStoryOSServer } from "../support/node-integration.ts";

// ADR 0089 section 5: a Skill that the author selected stays in effect for the later turns of the Conversation.
const FIXTURES = fileURLToPath(new URL("../../../../fixtures/skill-packages", import.meta.url));
const DIALOGUE = { name: "dialogue-style-guide",
  package_digest: "sha256:e03399e0385ea904f5c37265661480113ef0125e3c7f837cf04875d5cbe8178b" };
const DIALOGUE_TEXT = readFileSync(`${FIXTURES}/dialogue-style-guide/SKILL.md`, "utf8");

test("a later Run of the Conversation sends the Skill that the author selected again with a full input", async () => {
  await drainLeftoverWork();
  const started = await startServer({ extraEnv: { STORYOS_SKILLS_DIR: FIXTURES } });
  try {
    const story = await createStory(TestServer.at(started.baseUrl), { title: "Conversation Skill Novel",
      assistance: true });
    /** Admits one Run with `text`, `selections`, and `conversation`, and runs the Worker until the Run settles. */
    const settled = async (text: string, selections?: SkillPackageReference[], conversationId?: string) => {
      const request = runRequest(story.chapterIds[0] ?? "", story.key(), text);
      if (selections !== undefined) Object.assign(request.create_agent_run_input, { skill_selections: selections });
      if (conversationId !== undefined) {
        Object.assign(request.create_agent_run_input, {
          conversation: { kind: "existing", conversation_id: conversationId } });
      }
      const created = await postRun(started.baseUrl, story.fetchImpl, story.projectId, story.key(), request);
      assert.equal(created.effect.kind, "admitted");
      const run = await settleRun(started.baseUrl, story,
        created.effect.kind === "admitted" ? created.effect.run_id : "");
      return { run, conversationId: created.conversation_id };
    };
    // The destination loses the closing reference of the first Run.
    const first = await settled("Help with the dialogue. SCRIPT:reference-expires", [DIALOGUE]);
    const later = await settled("Help again.", undefined, first.conversationId);
    /** The role and the text of each skill_instruction item of each request of the later Run, in order. */
    const requests = await queryStoryOSPostgres(`
      SELECT jsonb_agg(jsonb_build_array(attempt_role, jsonb_path_query_array(wire,
               '$.context[*] ? (@.source_class == "skill_instruction").content'))
               ORDER BY model_attempt_id)::text
        FROM storyos.model_attempts,
             LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
       WHERE run_id = '${later.run.run_id}'::uuid AND decision_position = 0`);

    assert.deepEqual([first.run.status, later.run.status], ["completed", "completed"]);
    // The first request continues the expired reference and sends no Skill text. The full input sends the exact
    // SKILL.md of the selection. The later turn names no Skill.
    assert.deepEqual(JSON.parse(requests), [["rejected_create", []], ["decision", [DIALOGUE_TEXT]]]);
    assert.deepEqual([first.run.skill_uses, Object.hasOwn(later.run, "skill_uses")],
      [[{ provenance: "author_selected", name: DIALOGUE.name }], false]);
  } finally {
    await stopStoryOSServer(started.server);
  }
  await drainLeftoverWork();
});
