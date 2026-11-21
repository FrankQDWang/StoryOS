import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext, Locator, Page } from "playwright";
import { expect } from "playwright/test";

import { getChapter, getProposal } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, drainLeftoverWork, prepare, settleRun } from "./acceptance.ts";
import { startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { selectThenType, send } from "./production-skill-journey-command.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
// The Skill directory of the release package, which the distribution ships (S10-EVD-002).
const SKILLS = join(repositoryRoot, "target/release-package/skill-packages");
// Each text differs from the fixed candidate text of the fake model for its target Block in a few words.
const FIRST = "Guard the old narrator tone in this passage.";
const SECOND = "Keep the first block voice in this passage.";
// The fake model gives one candidate for each target Block of this request.
const REVISE = "Revise these passages: keep the voice.";
const CHECK = "这一章前后有没有矛盾？";
// The text answer of the fake model.
const ADVICE = "This passage is inspectable Host-fake advice. It is not Authoritative State.";
// The marks that the rule of the runs (S10-REQ-004) gives for each candidate: `【…】` is a mark, and `▏` is a tick.
const SHOWN = ["Guard the ▏narrator 【voic】e in this passage.", "Keep the 【second】 block voice in this passage."];
const TYPED = "Keep the 【second】 block voice in this passage【 now】.";
const CANDIDATE = "Guard the narrator voice in this passage.";
const EDITED = "Keep the second block voice in this passage now.";
const DECORATIONS = ".proposal-change-mark, .proposal-change-tick, .proposal-change-pair";

/** The name and the description of each SkillPackage of the directory, from the front matter of its `SKILL.md`. */
async function packagedSkills(): Promise<string[][]> {
  const names = (await readdir(SKILLS, { withFileTypes: true })).filter((entry) => entry.isDirectory())
    .map((entry) => entry.name).sort();
  return Promise.all(names.map(async (name) => {
    const text = await readFile(join(SKILLS, name, "SKILL.md"), "utf8");
    const field = (key: string) => new RegExp(`^${key}: (.+)$`, "mu").exec(text.split("---")[1] ?? "")?.[1];
    return [field("name") ?? "", field("description") ?? ""];
  }));
}

/** The text of one candidate in the notation of the fixture table of S10-REQ-004. */
function shown(candidate: Locator): Promise<string> {
  return candidate.locator(".block-proposal-text").evaluate((text) => Array.from(text.childNodes, (node) =>
    node instanceof Element && node.matches(".proposal-change-tick") ? "▏"
      : node instanceof Element && node.matches(".proposal-change-mark") ? `【${node.textContent}】`
        : node.textContent ?? "").join(""));
}

/** The text of each element of the editor that has the background of a pair, in document order. */
function paired(page: Page): Promise<string[]> {
  return page.locator("[data-manuscript-editor] .proposal-change-pair").evaluateAll((elements) =>
    elements.map((element) => element.textContent ?? ""));
}

/**
 * The author journey S10-JRN-001 of Stage 10 (S10-EVD-003): the Skill picker of the packaged SkillPackages, a
 * `local-revision` request with two target Blocks, the marks of each candidate and the pair under the pointer, an
 * edit of a candidate, one acceptance and one rejection, and a `conflict-check` request with a text answer.
 */
export async function verifyProductionAuthorVoiceJourney(context: BrowserContext): Promise<void> {
  const started: (() => Promise<unknown>)[] = [];
  try {
    const server = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A },
      serverBinary: join(repositoryRoot, "target/release-package/storyos-server"),
      extraEnv: { STORYOS_SKILLS_DIR: SKILLS } });
    started.push(() => stopStoryOSServer(server.server));
    const page = await context.newPage();
    started.push(() => page.close());
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.setDefaultTimeout(10_000);
    await page.setViewportSize({ width: 1487, height: 1058 });
    // Each `settleOnce` claims the oldest queued Run, so the Runs that earlier journeys left settle first.
    await drainLeftoverWork();
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-0000005a0a00", "Author voice journey", "5a0a1");
    const chapter = () => getChapter({ baseUrl: server.baseUrl, projectId: setup.projectId,
      fetchImpl: setup.fetchImpl, chapterId: setup.chapterId });
    await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator('[data-manuscript-editor][contenteditable="true"]').click();
    // Each input is saved before the next one, so each gives one Author Edit.
    const saved = async (texts: string[]) => {
      await expect.poll(async () => (await chapter()).chapter.current_revision.blocks.map((block) => block.text))
        .toEqual(texts);
      await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    };
    await page.keyboard.insertText(FIRST);
    await saved([FIRST]);
    await page.keyboard.press("Enter");
    await saved([FIRST, ""]);
    await page.keyboard.insertText(SECOND);
    await saved([FIRST, SECOND]);
    const blocks = (await chapter()).chapter.current_revision.blocks.map((block) => block.manuscript_block_id);
    const target = (index: number) => page.locator(`[data-manuscript-editor] p[data-id="${blocks[index]}"]`);

    // 1. The picker lists each SkillPackage of the release package with the description of its SKILL.md.
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const composer = page.locator('textarea[name="assistant-message"]');
    await composer.click();
    await page.keyboard.type("$");
    await expect.poll(() => page.locator("#skill-picker [data-skill-option]").evaluateAll((options) =>
      options.map((option) => [option.querySelector(".skill-name")?.textContent,
        option.querySelector(".skill-description")?.textContent]))).toEqual(await packagedSkills());
    await page.keyboard.press("Backspace");

    // 2. A `local-revision` request on the two Blocks gives one Proposal with a candidate for each Block.
    await selectThenType(page, "local-revision", REVISE);
    const revise = await send(page);
    assert.equal(revise.status, 202);
    assert.ok(revise.body.effect.kind === "admitted");
    const reviseRun = await settleRun(server.baseUrl, setup, revise.body.effect.run_id);
    assert.equal(reviseRun.status, "completed");
    assert.ok(reviseRun.decision.kind === "prose_change", JSON.stringify(reviseRun.decision));
    const outcomes = (reviseRun.decision.locations ?? []).map((location) => {
      assert.ok(location.outcome.kind === "opened", JSON.stringify(location.outcome));
      return location.outcome;
    });
    assert.deepEqual(reviseRun.decision.locations?.map((location) => location.manuscript_block_id), blocks);
    const proposalId = outcomes[0]!.proposal_id;
    assert.deepEqual(outcomes.map((outcome) => outcome.proposal_id), [proposalId, proposalId]);
    const candidates = outcomes.map((outcome) =>
      page.locator(`[data-proposal-id="${proposalId}"][data-proposal-operation-id="${outcome.operation_id}"]`));
    await expect(candidates[1]!).toBeVisible();
    assert.deepEqual([await shown(candidates[0]!), await shown(candidates[1]!)], SHOWN);

    // 3. The pointer on the mark of the second candidate gives a background to its old words only.
    await candidates[1]!.locator(".proposal-change-mark").hover();
    await expect.poll(() => paired(page)).toEqual(["first", "second"]);
    await expect(target(1).locator(".proposal-change-pair")).toHaveText(["first"]);
    await page.mouse.move(0, 0);
    await expect.poll(() => paired(page)).toEqual([]);
    await expect(target(0).locator(DECORATIONS)).toHaveCount(0);
    await expect(target(1).locator(DECORATIONS)).toHaveCount(0);

    // 4. The author types words into the second candidate. They have a mark, and the other marks stay.
    // The location link of the panel puts the caret into the candidate.
    await page.locator(`[data-proposal-location="${outcomes[1]!.operation_id}"]`).click();
    await expect(candidates[1]!).toHaveAttribute("data-proposal-focused", "true");
    await expect.poll(() => page.evaluate(() => window.getSelection()?.anchorNode?.parentElement
      ?.closest("[data-proposal-operation-id]")?.getAttribute("data-proposal-operation-id")))
      .toBe(outcomes[1]!.operation_id);
    await page.keyboard.press("End");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.insertText(" now");
    await page.mouse.move(0, 0);
    await expect.poll(() => shown(candidates[1]!)).toBe(TYPED);
    assert.equal(await shown(candidates[0]!), SHOWN[0]);
    await expect.poll(async () => (await getProposal({ baseUrl: server.baseUrl, projectId: setup.projectId,
      fetchImpl: setup.fetchImpl, proposalId })).proposal.operations.map((operation) => operation.candidate_text))
      .toEqual([CANDIDATE, EDITED]);

    // 5. The author accepts the edited candidate and rejects the other. No mark and no tick stay.
    await candidates[1]!.locator("[data-proposal-accept]").click();
    await expect.poll(async () => (await chapter()).chapter.current_revision.blocks.map((block) => block.text))
      .toEqual([FIRST, EDITED]);
    await candidates[0]!.locator("[data-proposal-reject]").click();
    await expect.poll(async () => (await getProposal({ baseUrl: server.baseUrl, projectId: setup.projectId,
      fetchImpl: setup.fetchImpl, proposalId })).proposal.operations.map((operation) => operation.resolution))
      .toEqual(["rejected", "applied"]);
    await expect(page.locator("[data-manuscript-editor] [data-proposal-id]")).toHaveCount(0);
    await expect(page.locator(`[data-manuscript-editor] :is(${DECORATIONS})`)).toHaveCount(0);
    assert.deepEqual((await chapter()).chapter.current_revision.blocks.map((block) => block.text), [FIRST, EDITED]);

    // 6. In a new Project Conversation, a `conflict-check` request gets a text answer and no Proposal.
    await page.locator("[data-assistant-new-conversation]").click();
    await selectThenType(page, "conflict-check", CHECK);
    const check = await send(page);
    assert.equal(check.status, 202);
    assert.ok(check.body.effect.kind === "admitted");
    assert.deepEqual(check.request.create_agent_run_input.conversation, { kind: "new" });
    assert.notEqual(check.body.conversation_id, revise.body.conversation_id);
    const checkRun = await settleRun(server.baseUrl, setup, check.body.effect.run_id);
    assert.equal(checkRun.status, "completed");
    assert.ok(checkRun.decision.kind !== "prose_change", JSON.stringify(checkRun.decision));
    assert.deepEqual(checkRun.skill_uses, [{ provenance: "author_selected", name: "conflict-check" }]);
    await page.locator(`[data-assistant-run-id="${check.body.effect.run_id}"][data-assistant-dispatch="completed"]`)
      .waitFor();
    await expect(page.locator("[data-assistant-current-run] .assistant-result")).toHaveText(ADVICE);
    // The panel keeps the earlier exchange above the boundary of the new Conversation.
    await expect(page.locator("[data-assistant-current-run] .assistant-conversation-boundary")).toHaveText("新对话");
    await expect(page.locator("[data-assistant-current-run] [data-sent-skill-label]")).toHaveText(["$conflict-check"]);
    await expect(page.locator("[data-assistant-current-run] [data-assistant-skill-uses] li"))
      .toHaveText(["使用了你选择的技能「conflict-check」"]);
    await expect(page.locator("[data-manuscript-editor] [data-proposal-id]")).toHaveCount(0);
    assert.deepEqual(errors, []);

    await assertProjectReceipts([setup.projectId], { projects: 1, receipts: { createProject: 1,
      updateProjectAssistance: 1, createVolume: 1, createChapter: 1, setCurrentChapter: 1,
      // Three edits of the manuscript and one edit of the candidate.
      applyAuthorEdit: 4, createAgentRun: 2, acceptProposal: 1, rejectProposalOperations: 1 } });
  } finally {
    for (const close of started.reverse()) await close();
  }
}
