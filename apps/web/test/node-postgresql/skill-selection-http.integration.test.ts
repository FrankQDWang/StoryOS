// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import type { SkillPackageReference } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { TestServer, createStory, drainLeftoverWork, postRun, problemCode, startServer } from "../support/http-harness.ts";
import { runRequest } from "../support/acceptance.ts";
import { queryStoryOSPostgres, requireStoryOSProtocolError, stopStoryOSServer } from "../support/node-integration.ts";

// ADR 0089 sections 5 and 7: the author selects Skills of the deployment in the field skill_selections.
const FIXTURES = fileURLToPath(new URL("../../../../fixtures/skill-packages", import.meta.url));
const DIALOGUE = { name: "dialogue-style-guide",
  package_digest: "sha256:e03399e0385ea904f5c37265661480113ef0125e3c7f837cf04875d5cbe8178b" };
const COMMS = { name: "internal-comms",
  package_digest: "sha256:3ccc24f2e8e7c47369c7a2cf8456271c6338b8a83d1917756c3584b2466c67e9" };

/** The Skill requirements and the source version of each skill_instruction item of the first RunStep of the Run. */
function firstStep(runId: string): Promise<string> {
  return queryStoryOSPostgres(`
    SELECT jsonb_build_array(payload #> '{operation_requirement,skill_requirements}',
             jsonb_path_query_array(payload, '$.selected[*] ? (@.source_class == "skill_instruction").source_version'))::text
      FROM storyos.context_assembly_manifests
     WHERE run_id = '${runId}'::uuid AND manifest_role = 'decision' AND decision_position = 0`);
}

test("createAgentRun admits a selection of the Server Skills and refuses each other selection", async () => {
  await drainLeftoverWork();
  const started = await startServer({ extraEnv: { STORYOS_SKILLS_DIR: FIXTURES } });
  try {
    const story = await createStory(TestServer.at(started.baseUrl), { title: "Selection Novel", assistance: true });
    const send = (text: string, selections?: SkillPackageReference[]) => {
      const request = runRequest(story.chapterIds[0] ?? "", story.key(), text);
      if (selections !== undefined) request.create_agent_run_input.skill_selections = selections;
      return postRun(started.baseUrl, story.fetchImpl, story.projectId, story.key(), request).then(
        (created) => created.effect.kind === "admitted" ? created.effect.run_id : created.effect.kind,
        (error: unknown) => `${requireStoryOSProtocolError(error).status} ${problemCode(error) ?? ""}`);
    };
    const other = { ...COMMS, package_digest: DIALOGUE.package_digest };
    const refused = [
      await send("Help.", []),
      await send("Help.", [DIALOGUE, { ...DIALOGUE, package_digest: COMMS.package_digest }]),
      await send("Help.", [{ ...DIALOGUE, name: "Dialogue" }]),
      await send("Help.", [other]),
    ];
    const runs = await queryStoryOSPostgres(
      `SELECT count(*) FROM storyos.agent_runs WHERE project_id = '${story.projectId}'::uuid`);
    const selected = await send("Help with the dialogue.", [COMMS, DIALOGUE]);
    const named = await send("Use $internal-comms here.");

    assert.deepEqual(refused, ["422 invalid_request_shape", "422 invalid_request_shape",
      "422 invalid_request_shape", "422 skill_selection_unavailable"]);
    assert.equal(runs, "0");
    // The requirements and the items are in the order of the Skill names. The text $name selects nothing.
    assert.deepEqual(JSON.parse(await firstStep(selected)), [
      [DIALOGUE, COMMS].map(({ name, package_digest }) => ({ skill_name: name, package_digest,
        provenance: "author_selected" })),
      [DIALOGUE.package_digest, COMMS.package_digest],
    ]);
    assert.deepEqual(JSON.parse(await firstStep(named)), [null, []]);
  } finally {
    await stopStoryOSServer(started.server);
  }
  await drainLeftoverWork();
});
