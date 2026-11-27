import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext, Page } from "playwright";
import { expect } from "playwright/test";

import { USER_A, prepare } from "./acceptance.ts";
import { queryStoryOSPostgres, startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const EDITOR = '[data-manuscript-editor][contenteditable="true"]';
const SAVED = '[data-save-state="saved"][data-unsettled-intent-count="0"]';
const CHAPTER_TEXT = "雨下了一整夜。";
const NEXT_TEXT = "天亮了。";
const FIRST = "每次回答都用中文，语气克制。";
const SECOND = "每次回答都用中文，语气克制，不超过三句话。";
// One character above the Context item limit of ADR 0052.
const OVER_LIMIT = "字".repeat(32_001);

/** The revision and the text of each Project Instruction Revision of the Project, in revision order. */
async function revisions(projectId: string): Promise<unknown> {
  return JSON.parse(await queryStoryOSPostgres(`
    SELECT coalesce(json_agg(json_build_array(revision::text, text) ORDER BY revision), '[]')::text
      FROM storyos.project_instruction_revisions
     WHERE owner_user_id = '${USER_A}'::uuid AND project_id = '${projectId}'::uuid`));
}

/** Saves the text of the field, and waits for the sentence of the outcome. */
async function save(page: Page, text: string, sentence: string) {
  await page.locator("[data-project-instruction]").fill(text);
  await page.locator("[data-project-instruction-save]").click();
  await expect(page.locator("[data-project-instruction-outcome]")).toHaveText(sentence);
}

/**
 * ADR 0094 section 9: the author opens the settings page of a Project with the gear control, saves the Project
 * Instruction, sees the refusal of a text above the limit, and goes back to writing past the warning for a text that
 * is not saved.
 */
export async function verifyProductionProjectSettings(context: BrowserContext): Promise<void> {
  const started: (() => Promise<unknown>)[] = [];
  try {
    const server = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A },
      serverBinary: join(repositoryRoot, "target/release-package/storyos-server") });
    started.push(() => stopStoryOSServer(server.server));
    const page = await context.newPage();
    started.push(() => page.close());
    const errors: string[] = [];
    const dialogs: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("dialog", (dialog) => { dialogs.push(dialog.type()); void dialog.dismiss(); });
    page.setDefaultTimeout(10_000);
    await page.setViewportSize({ width: 1487, height: 1058 });
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-0000007a2a00", "Settings journey", "7a2a1");
    const workspace = `${server.baseUrl}/projects/${setup.projectId}`;

    // 1. The author types text in the Chapter, and opens the settings page with the gear.
    await page.goto(workspace);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator(EDITOR).focus();
    await page.keyboard.insertText(CHAPTER_TEXT);
    await page.locator(SAVED).waitFor();
    await page.locator('.tree-footer a[aria-label="项目设置"]').click();
    await page.waitForURL(`${workspace}/settings`);
    await expect(page.locator("[data-project-instruction]")).toHaveValue("");

    // 2. The author saves a Project Instruction, and a reload shows the saved text.
    await save(page, FIRST, "已保存。新的指令从下一次请求开始生效。");
    await page.reload();
    await expect(page.locator("[data-project-instruction]")).toHaveValue(FIRST);

    // 3. A second save makes the second Revision, and the first Revision keeps its text.
    await save(page, SECOND, "已保存。新的指令从下一次请求开始生效。");
    const saved = [["1", FIRST], ["2", SECOND]];
    assert.deepEqual(await revisions(setup.projectId), saved);

    // 4. The Host refuses a text above the limit, and the field keeps it.
    await save(page, OVER_LIMIT, "项目指令太长，没有保存。请缩短后再保存。");
    await expect(page.locator("[data-project-instruction]")).toHaveValue(OVER_LIMIT);
    assert.deepEqual(await revisions(setup.projectId), saved);

    // 5. The text of step 4 is not saved, so `返回写作` asks first. `留下` keeps the page and the text.
    const confirm = page.locator("[data-settings-leave-confirm]");
    await page.locator("[data-return-to-writing]").click();
    await expect(confirm.locator("p")).toHaveText("项目指令有未保存的修改。确定离开吗？");
    await confirm.locator("[data-settings-stay]").click();
    await expect(confirm).toHaveCount(0);
    await expect(page.locator("[data-project-instruction]")).toHaveValue(OVER_LIMIT);
    assert.equal(new URL(page.url()).pathname, `/projects/${setup.projectId}/settings`);
    await page.locator("[data-return-to-writing]").click();
    await confirm.locator("[data-settings-leave]").click();

    // 6. The workspace has the text of step 1, and the editor accepts input with no click.
    await page.waitForURL(workspace);
    await expect(page.locator("[data-manuscript-editor] > p")).toHaveText([CHAPTER_TEXT]);
    await page.locator(EDITOR).focus();
    await page.keyboard.press("End");
    await page.keyboard.insertText(NEXT_TEXT);
    await page.locator(SAVED).waitFor();
    await expect(page.locator("[data-manuscript-editor] > p")).toHaveText([`${CHAPTER_TEXT}${NEXT_TEXT}`]);
    assert.deepEqual(await revisions(setup.projectId), saved);
    assert.deepEqual([errors, dialogs], [[], []]);

    await assertProjectReceipts([setup.projectId], { projects: 1, receipts: { createProject: 1,
      updateProjectAssistance: 1, createVolume: 1, createChapter: 1, applyAuthorEdit: 2, updateProjectInstruction: 2 } });
  } finally {
    for (const close of started.reverse()) await close();
  }
}
