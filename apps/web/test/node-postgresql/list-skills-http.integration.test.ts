// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { cp, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { getProject, listSkills } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { TestServer, USER_A, createProject, problemOf, restartServer, startServer } from "../support/http-harness.ts";
import { stopStoryOSServer } from "../support/node-integration.ts";

// ADR 0089 sections 5 and 8: the $ picker lists the name and the description of each Skill of the deployment.
const FIXTURES = fileURLToPath(new URL("../../../../fixtures/skill-packages", import.meta.url));
const DIALOGUE = {
  reference: { name: "dialogue-style-guide",
    package_digest: "sha256:e03399e0385ea904f5c37265661480113ef0125e3c7f837cf04875d5cbe8178b" },
  description: "小说对白的写法规范。当作者要求写、改写或润色人物对白，或者要求检查一段对话是否自然时，使用这个技能。",
};
const COMMS = {
  reference: { name: "internal-comms",
    package_digest: "sha256:3ccc24f2e8e7c47369c7a2cf8456271c6338b8a83d1917756c3584b2466c67e9" },
  description: "A set of resources to help me write all kinds of internal communications, using the formats that "
    + "my company likes to use. Claude should use this skill whenever asked to write some sort of internal "
    + "communications (status reports, leadership updates, 3P updates, company newsletters, FAQs, incident "
    + "reports, project updates, etc.).",
};

test("listSkills gives the Skills that the Server validated at its start to the authors of the Project", async () => {
  // The Server reads a copy of the fixtures. The copy goes after the start, so the read cannot read a directory.
  const copy = await mkdtemp(join(tmpdir(), "storyos-skills-"));
  await cp(FIXTURES, copy, { recursive: true });
  let started = await startServer({ extraEnv: { STORYOS_SKILLS_DIR: copy } });
  try {
    await rm(copy, { recursive: true });
    const project = await createProject(TestServer.at(started.baseUrl), { title: "Skill List Novel" });
    const list = async () => {
      const { correlation_id: _, ...response } = await listSkills({ baseUrl: started.baseUrl,
        fetchImpl: project.fetchImpl, projectId: project.projectId });
      return response;
    };
    const scope = { owner_user_id: USER_A, project_id: project.projectId };
    const offered = await list();
    // A second author gets the refusal of each other Project read.
    const foreign = { baseUrl: started.baseUrl, fetchImpl: project.server.fetch("session-b"),
      projectId: project.projectId };
    const refusals = [await problemOf(listSkills(foreign)), await problemOf(getProject(foreign))]
      .map(({ status, code }) => ({ status, code }));
    started = await restartServer(started);
    const none = await list();

    assert.deepEqual({ offered, refusals, none }, {
      offered: { schema_id: "storyos.query.skills.response.v1", project_scope: scope, skills: [DIALOGUE, COMMS] },
      refusals: [{ status: 404, code: "resource_unavailable" }, { status: 404, code: "resource_unavailable" }],
      none: { schema_id: "storyos.query.skills.response.v1", project_scope: scope, skills: [] },
    });
  } finally {
    await stopStoryOSServer(started.server);
  }
});
