import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext, Page } from "playwright";
import { expect } from "playwright/test";

import { getChapter } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunRequest, CreateAgentRunResponse, GetAgentRunResponse, ListSkillsResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, drainLeftoverWork, inspectRun, prepare, settleRun } from "./acceptance.ts";
import { queryStoryOSPostgres, startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const FIXTURES = join(repositoryRoot, "fixtures/skill-packages");
const COMMS = { name: "internal-comms",
  package_digest: "sha256:3ccc24f2e8e7c47369c7a2cf8456271c6338b8a83d1917756c3584b2466c67e9" };
const DIALOGUE = { name: "dialogue-style-guide",
  package_digest: "sha256:e03399e0385ea904f5c37265661480113ef0125e3c7f837cf04875d5cbe8178b" };
// A digest that the deployment does not have, as after a change of the package by the operator.
const EARLIER_DIGEST = `sha256:${"0".repeat(64)}`;
// The reason that the fake destination gives for each load.
const REASON = "The author request fits the description of this Skill.";
const UNAVAILABLE = "所选技能现在不可用，消息没有发送。请重新选择技能，或直接发送。";
const PARAGRAPH = "“船到了。”林舟说。";
// The fake destination reads its script from the author message (S6-03b, S6-03d).
const READ = "SCRIPT:read_skill_resource load dialogue-style-guide references/examples.md";
const MISSING = "SCRIPT:load_skill no-such-skill";
const PLAIN = "这一段怎么写得更安静？";

/** The Skill ToolCalls of a Run in call order: the function, the state, the refusal, and whether a Destination Attempt exists. */
async function skillCalls(runId: string) {
  return JSON.parse(await queryStoryOSPostgres(`
    SELECT coalesce(json_agg(json_build_array(function_name, arguments->>'skill', state, refusal,
      destination_attempt_id IS NOT NULL) ORDER BY decision_position, call_position), '[]')::text
      FROM storyos.tool_calls WHERE run_id = '${runId}'::uuid
       AND function_name IN ('load_skill', 'read_skill_resource')`)) as unknown[];
}

/** The decision Model Attempt of the Run at this RunStep. */
function attemptAt(runId: string, position: number) {
  return queryStoryOSPostgres(`SELECT model_attempt_id::text FROM storyos.model_attempts
    WHERE run_id = '${runId}'::uuid AND attempt_role = 'decision' AND decision_position = ${position}`);
}

/** The Skill items that the request of one Model Attempt selected: the class, the source version, and the text. */
function skillItems(run: GetAgentRunResponse) {
  return run.context.selected.filter((item) => item.source_class.startsWith("skill_"))
    .map((item) => [item.source_class, item.source_version, item.content]);
}

/**
 * Types `$` in the composer, chooses the Skill in the picker, and then types the message with keyboard events. The
 * typed text must show in the composer after the chip (finding 2 of the live record of S6-03c).
 */
async function selectThenType(page: Page, skill: string, message: string) {
  const composer = page.locator('textarea[name="assistant-message"]');
  await composer.click();
  await page.keyboard.type("$");
  await page.locator(`#skill-picker [data-skill-option="${skill}"]`).click();
  await expect(page.locator(`[data-skill-chip="${skill}"]`)).toHaveText(`$${skill}×`);
  await expect(composer).toHaveValue("");
  await page.keyboard.type(message);
  await expect(composer).toHaveValue(message);
}

/** Clicks the send button and returns the createAgentRun request and response that the click sent. */
async function send(page: Page) {
  const response = page.waitForResponse((reply) =>
    reply.request().method() === "POST" && new URL(reply.url()).pathname.endsWith("/agent-runs"));
  await page.locator(".composer .send-button").click();
  const reply = await response;
  return { request: reply.request().postDataJSON() as CreateAgentRunRequest, status: reply.status(),
    body: await reply.json() as CreateAgentRunResponse };
}

function admitted(sent: Awaited<ReturnType<typeof send>>) {
  assert.equal(sent.status, 202);
  assert.ok(sent.body.effect.kind === "admitted");
  return sent.body.effect.run_id;
}

export async function verifyProductionSkillJourney(context: BrowserContext): Promise<void> {
  const started: (() => Promise<unknown>)[] = [];
  try {
    const server = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A },
      serverBinary: join(repositoryRoot, "target/release-package/storyos-server"),
      extraEnv: { STORYOS_SKILLS_DIR: FIXTURES } });
    started.push(() => stopStoryOSServer(server.server));
    const page = await context.newPage();
    started.push(() => page.close());
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.setDefaultTimeout(10_000);
    await page.setViewportSize({ width: 1487, height: 1058 });
    // Each `settleOnce` claims the oldest queued Run, so the Runs that earlier journeys left settle first.
    await drainLeftoverWork();
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-0000006e0a00", "Skill journey", "6e0a1");
    await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
    await editor.click();
    await page.keyboard.insertText(PARAGRAPH);
    await expect.poll(async () => (await getChapter({ baseUrl: server.baseUrl, projectId: setup.projectId,
      fetchImpl: setup.fetchImpl, chapterId: setup.chapterId })).chapter.current_revision.blocks
      .map((block) => block.text)).toEqual([PARAGRAPH]);
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();

    // The author selects internal-comms with the `$` picker and then types the message. The Agent loads
    // dialogue-style-guide and reads a file that its SKILL.md names, in the same Run.
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await selectThenType(page, COMMS.name, READ);
    const selected = await send(page);
    // The author message is exactly the typed text, one time.
    assert.equal(selected.request.create_agent_run_input.author_message.text, READ);
    assert.deepEqual(selected.request.create_agent_run_input.skill_selections, [COMMS]);
    const readRun = admitted(selected);
    await expect(page.locator("[data-sent-skill-label]")).toHaveText([`$${COMMS.name}`]);
    const read = await settleRun(server.baseUrl, setup, readRun);
    assert.equal(read.status, "completed");
    assert.deepEqual(read.skill_uses, [{ provenance: "author_selected", name: COMMS.name },
      { provenance: "agent_selected", name: DIALOGUE.name, reason: REASON }]);
    // The Host settles both Host Tools with no Destination Attempt.
    assert.deepEqual(await skillCalls(readRun), [["load_skill", DIALOGUE.name, "settled", null, false],
      ["read_skill_resource", DIALOGUE.name, "settled", null, false]]);
    const fixture = (path: string) => readFile(join(FIXTURES, path), "utf8");
    // The first RunStep has the SKILL.md of the selected package. The third has the loaded SKILL.md and the exact file.
    const first = await inspectRun(server.baseUrl, setup, readRun, await attemptAt(readRun, 0));
    assert.deepEqual(skillItems(first).filter(([kind]) => kind === "skill_instruction"),
      [["skill_instruction", COMMS.package_digest, await fixture("internal-comms/SKILL.md")]]);
    const third = await inspectRun(server.baseUrl, setup, readRun, await attemptAt(readRun, 2));
    assert.deepEqual(skillItems(third).filter(([kind]) => kind !== "skill_catalog").map(([kind, , text]) => [kind, text]), [
      ["skill_instruction", await fixture("internal-comms/SKILL.md")],
      ["skill_instruction", await fixture("dialogue-style-guide/SKILL.md")],
      ["skill_resource", await fixture("dialogue-style-guide/references/examples.md")]]);
    const lines = [`使用了你选择的技能「${COMMS.name}」`, `使用了技能「${DIALOGUE.name}」：${REASON}`];
    await page.reload();
    await expect(page.locator("[data-assistant-skill-uses] li")).toHaveText(lines);

    // A load of a name that the catalog does not have is refused with its reason, and the Run completes.
    await page.locator('textarea[name="assistant-message"]').fill(MISSING);
    const missingRun = admitted(await send(page));
    const missing = await settleRun(server.baseUrl, setup, missingRun);
    assert.equal(missing.status, "completed");
    assert.equal(Object.hasOwn(missing, "skill_uses"), false);
    assert.deepEqual(await skillCalls(missingRun), [["load_skill", "no-such-skill", "refused", "skill_not_in_catalog", false]]);
    await page.reload();
    // Only the history turn of the first Run names Skills.
    await expect(page.locator("[data-assistant-skill-uses]")).toHaveCount(1);
    await expect(page.locator(`[data-assistant-history-run="${readRun}"] [data-assistant-skill-uses] li`)).toHaveText(lines);

    // A chip whose reference is not in the catalog refuses the message with one sentence, and the text stays. The
    // picker shows an earlier digest of dialogue-style-guide, as a list read before an operator change does.
    await page.route((url) => url.pathname.endsWith("/skills"), async (route) => {
      const response = await route.fetch();
      const body = await response.json() as ListSkillsResponse;
      await route.fulfill({ response, json: { ...body, skills: body.skills.map((entry) => entry.reference.name === DIALOGUE.name
        ? { ...entry, reference: { ...entry.reference, package_digest: EARLIER_DIGEST } } : entry) } });
    });
    await page.reload();
    await selectThenType(page, DIALOGUE.name, PLAIN);
    await page.unroute((url) => url.pathname.endsWith("/skills"));
    const stale = await send(page);
    assert.equal(stale.status, 422);
    assert.deepEqual(stale.request.create_agent_run_input.skill_selections,
      [{ name: DIALOGUE.name, package_digest: EARLIER_DIGEST }]);
    await expect(page.getByRole("status")).toHaveText(UNAVAILABLE);
    await expect(page.locator('textarea[name="assistant-message"]')).toHaveValue(PLAIN);
    await expect(page.locator("[data-skill-chip]")).toHaveCount(0);
    // The author sends the same text with no chip, and the Run completes with no Skill.
    const plainRun = admitted(await send(page));
    const plain = await settleRun(server.baseUrl, setup, plainRun);
    assert.equal(plain.status, "completed");
    assert.equal(Object.hasOwn(plain, "skill_uses"), false);
    assert.deepEqual(errors, []);
    await assertProjectReceipts([setup.projectId], { projects: 1, receipts: { createProject: 1, updateProjectAssistance: 1,
      createVolume: 1, createChapter: 1, applyAuthorEdit: 1, createAgentRun: 3 }, authorActions: 3 });
  } finally {
    for (const close of started.reverse()) await close();
  }
}
