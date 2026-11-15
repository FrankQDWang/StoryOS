// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import type { SkillPackageReference } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { TestServer, createStory, drainLeftoverWork, postRun, settleRun, startServer } from "../support/http-harness.ts";
import { inspectRun, runRequest } from "../support/acceptance.ts";
import { queryStoryOSPostgres, stopStoryOSServer } from "../support/node-integration.ts";

// ADR 0089 section 8: the Run read names each Skill that the Run used.
const FIXTURES = fileURLToPath(new URL("../../../../fixtures/skill-packages", import.meta.url));
const COMMS = { name: "internal-comms",
  package_digest: "sha256:3ccc24f2e8e7c47369c7a2cf8456271c6338b8a83d1917756c3584b2466c67e9" };
// The reason that the fake destination gives for each load.
const REASON = "The author request fits the description of this Skill.";

test("the Run read of getAgentRun gives each Skill that the Run used", async () => {
  await drainLeftoverWork();
  const started = await startServer({ extraEnv: { STORYOS_SKILLS_DIR: FIXTURES } });
  try {
    const story = await createStory(TestServer.at(started.baseUrl), { title: "Skill Use Novel", assistance: true });
    /** Admits one Run with `text` and `selections`, and runs the Worker until the Run settles. */
    const settled = async (text: string, selections?: SkillPackageReference[]) => {
      const request = runRequest(story.chapterIds[0] ?? "", story.key(), text);
      if (selections !== undefined) Object.assign(request.create_agent_run_input, { skill_selections: selections });
      const created = await postRun(started.baseUrl, story.fetchImpl, story.projectId, story.key(), request);
      assert.equal(created.effect.kind, "admitted");
      return settleRun(started.baseUrl, story, created.effect.kind === "admitted" ? created.effect.run_id : "");
    };
    // The author selects internal-comms. The Agent loads dialogue-style-guide, and the Host refuses its load of a
    // name that the catalog does not have.
    const used = await settled("SCRIPT:load_skill dialogue-style-guide,no-such-skill", [COMMS]);
    const plain = await settled("Help with this passage.");
    const loadAttempt = await queryStoryOSPostgres(`
      SELECT model_attempt_id::text FROM storyos.model_attempts
       WHERE run_id = '${used.run_id}'::uuid AND attempt_role = 'decision' AND decision_position = 0`);
    const load = await inspectRun(started.baseUrl, story, used.run_id, loadAttempt);

    assert.deepEqual([used.status, plain.status], ["completed", "completed"]);
    assert.deepEqual(used.skill_uses, [
      { provenance: "author_selected", name: "internal-comms" },
      { provenance: "agent_selected", name: "dialogue-style-guide", reason: REASON },
    ]);
    // A Run with no Skill and a read of one Model Attempt have no such field. The ToolCall list of the load request
    // holds only research calls.
    assert.deepEqual([Object.hasOwn(plain, "skill_uses"), Object.hasOwn(load, "skill_uses")], [false, false]);
    assert.deepEqual(load.decision.kind === "tool_request" ? load.decision.calls : load.decision.kind, []);
  } finally {
    await stopStoryOSServer(started.server);
  }
  await drainLeftoverWork();
});
