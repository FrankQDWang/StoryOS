import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect } from "playwright/test";
import type { BrowserContext, Page } from "playwright";
import {
  createProjectCommandChallenge, digestUpdateProjectAssistance, getAgentRun, getChapter, updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunRequest, CreateAgentRunResponse, SelectionReference, SteerAgentRunRequest,
  UpdateProjectAssistanceRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { runStoryOSWorker, sessionFetch } from "./node-integration.ts";

function uuidV7() {
  const value = randomUUID();
  const time = Date.now().toString(16).padStart(12, "0");
  return `${time.slice(0, 8)}-${time.slice(8)}-7${value.slice(15)}`;
}

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const PARAGRAPHS = ["雨夜。渡口无人。", "船夫在等着。", "灯灭了。"];

/** Selects the text between two offsets of the editor paragraphs, as a mouse selection does. */
async function selectText(page: Page, start: [number, number], end: [number, number]) {
  await page.locator("[data-manuscript-editor]").evaluate((editor, [from, to]) => {
    const point = ([paragraph, offset]: [number, number]) => {
      const node = editor.querySelectorAll("p")[paragraph]!.firstChild!;
      return [node, offset] as const;
    };
    (editor as HTMLElement).focus();
    document.getSelection()!.setBaseAndExtent(...point(from), ...point(to));
  }, [start, end] as const);
}

export async function verifyProductionSelectionReference(context: BrowserContext, origin: string) {
  const page = await context.newPage();
  await page.setViewportSize({ width: 1487, height: 1058 });
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.setDefaultTimeout(10_000);
  try {
    assert.equal((await page.goto(origin))?.status(), 200);
    await page.locator('#app[data-boot-state="protected-ready"]').waitFor();
    await page.locator('input[name="title"]').fill(`Selection reference ${uuidV7()}`);
    await page.locator('input[name="title"]').press("Enter");
    await page.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    const projectId = await page.locator("[data-project-id]").getAttribute("data-project-id");
    assert.ok(projectId);
    const options = { baseUrl: origin, projectId, fetchImpl: sessionFetch(origin, "session-a") };
    const assistance: UpdateProjectAssistanceRequest = {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: { availability: "available", expected_assistance_revision: "0",
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: uuidV7() },
    };
    const idempotencyKey = uuidV7();
    const challenge = await createProjectCommandChallenge({ ...options, request: {
      method: "PUT", route_template: "/api/v1/projects/{project_id}/assistance",
      command_schema: assistance.command_schema, canonical_command_digest: await digestUpdateProjectAssistance(assistance),
      idempotency_key: idempotencyKey,
    } });
    await updateProjectAssistance({ ...options, request: assistance, idempotencyKey, antiForgery: challenge.nonce });
    await page.locator("[data-add-chapter]").click();
    await page.locator("[data-create-volume-action]").click();
    await page.locator('input[name="volume-title"]').fill("渡口卷");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator("[data-add-chapter]").click();
    await page.locator('[data-chapter-placement="append"]').click();
    await page.locator('form[data-create-chapter] input[name="chapter-title"]').fill("渡口");
    await page.locator('input[name="chapter-title"]').press("Enter");
    const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
    await editor.waitFor();
    const chapterId = (await page.locator('nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]')
      .getAttribute("data-chapter-id"))!;
    await editor.click();
    await page.keyboard.insertText(PARAGRAPHS[0]!);
    for (const paragraph of PARAGRAPHS.slice(1)) {
      await page.keyboard.press("Enter");
      await page.keyboard.insertText(paragraph);
    }
    const saved = page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]');
    await expect.poll(async () => (await getChapter({ ...options, chapterId })).chapter.current_revision.blocks
      .map((block) => block.text)).toEqual(PARAGRAPHS);
    await saved.waitFor();

    // 渡口无人。 to 船夫: the label counts the paragraphs, and an edit before the range moves it.
    await selectText(page, [0, 3], [1, 2]);
    await page.locator("[data-add-selection-reference]").click();
    const label = '[data-selection-reference-label="第 1 章 渡口（第 1–2 段）"]';
    await expect(page.locator(label)).toHaveCount(1);
    await selectText(page, [0, 0], [0, 0]);
    await page.keyboard.insertText("冬");
    await expect.poll(async () => (await getChapter({ ...options, chapterId })).chapter.current_revision.blocks[0]!.text)
      .toBe(`冬${PARAGRAPHS[0]}`);
    await saved.waitFor();
    const chapter = (await getChapter({ ...options, chapterId })).chapter;
    const blockIds = chapter.current_revision.blocks.map((block) => block.manuscript_block_id);
    const reference = (start: [number, number], end: [number, number]): SelectionReference => ({
      kind: "chapter_text", chapter_id: chapterId, base_authoritative_revision_id: chapter.current_revision.revision_id,
      coordinate_profile: "storyos.editor.utf16-code-unit.v1",
      start: { manuscript_block_id: blockIds[start[0]]!, offset: start[1] },
      end: { manuscript_block_id: blockIds[end[0]]!, offset: end[1] } });

    let created: CreateAgentRunRequest | undefined;
    let admitted: CreateAgentRunResponse | undefined;
    let steered: SteerAgentRunRequest | undefined;
    await page.route((url) => url.pathname.endsWith("/agent-runs"), async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      created = route.request().postDataJSON() as CreateAgentRunRequest;
      const response = await route.fetch();
      admitted = await response.json() as CreateAgentRunResponse;
      await route.fulfill({ response });
    });
    await page.route((url) => url.pathname.endsWith("/steering-inputs"), async (route) => {
      steered = route.request().postDataJSON() as SteerAgentRunRequest;
      await route.continue();
    });
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await page.locator('[name="assistant-message"]').fill("What would make this sharper?");
    await page.locator(".composer button.send-button").click();
    await expect.poll(() => admitted?.effect.kind).toBe("admitted");
    assert.deepEqual(created?.create_agent_run_input.working_target, { kind: "selection_references",
      source_chapter_id: chapterId, references: [reference([0, 4], [1, 2])] });
    await expect(page.locator("[data-selection-reference-label]")).toHaveCount(0);
    await expect(page.locator("[data-sent-reference-label]")).toHaveText(["第 1 章 渡口（第 1–2 段）"]);

    // Guidance during the Run adds its own reference, and a repeated one does not count again.
    await selectText(page, [2, 0], [2, 2]);
    await page.locator("[data-add-selection-reference]").click();
    await page.locator('[name="assistant-message"]').fill("Use the lamp line too.");
    await page.locator(".composer button.send-button").click();
    await expect.poll(() => steered?.steer_agent_run_input.selection_references).toEqual([reference([2, 0], [2, 2])]);
    await expect(page.locator("[data-sent-reference-label]"))
      .toHaveText(["第 1 章 渡口（第 1–2 段）", "第 1 章 渡口（第 3 段）"]);
    // A copy from the editor and a paste in the composer add the same reference, with no pasted text.
    await selectText(page, [1, 0], [1, 2]);
    await page.evaluate(() => {
      const data = new DataTransfer();
      document.querySelector("[data-manuscript-editor]")!.dispatchEvent(new ClipboardEvent("copy",
        { clipboardData: data, bubbles: true, cancelable: true }));
      document.querySelector("textarea[name=assistant-message]")!.dispatchEvent(new ClipboardEvent("paste",
        { clipboardData: data, bubbles: true, cancelable: true }));
    });
    await expect(page.locator('[data-selection-reference-label="第 1 章 渡口（第 2 段）"]')).toHaveCount(1);
    await expect(page.locator('[name="assistant-message"]')).toHaveValue("");
    await page.locator("[data-remove-selection-reference]").click();
    assert.ok(admitted?.effect.kind === "admitted");
    const runId = admitted.effect.run_id;
    for (let attempt = 0; attempt < 8; attempt += 1) {
      if ((await getAgentRun({ ...options, runId })).status === "completed") break;
      await runStoryOSWorker({ repositoryRoot, workerBinary: join(repositoryRoot,
        "target", "release-package", "storyos-worker"), args: ["--once"] });
    }
    const run = await getAgentRun({ ...options, runId });
    assert.equal(run.status, "completed");
    assert.deepEqual(run.context.selection_references?.map((item) => [item.reference, item.availability.kind]),
      [[reference([0, 4], [1, 2]), "current"], [reference([2, 0], [2, 2]), "current"]]);
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
  }
  await verifyNewChapterSelection(context, origin);
}

/**
 * A Chapter that the author creates after the first one gives a selection the add control and its label
 * with no reload, also after one lost outline response (#1175). The Project has no assistance, so it writes
 * only Receipts of the outline and the editor.
 */
async function verifyNewChapterSelection(context: BrowserContext, origin: string) {
  const page = await context.newPage();
  await page.setViewportSize({ width: 1487, height: 1058 });
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.setDefaultTimeout(10_000);
  try {
    assert.equal((await page.goto(origin))?.status(), 200);
    await page.locator('#app[data-boot-state="protected-ready"]').waitFor();
    await page.locator('input[name="title"]').fill(`New Chapter reference ${uuidV7()}`);
    await page.locator('input[name="title"]').press("Enter");
    await page.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    await page.locator("[data-add-chapter]").click();
    await page.locator("[data-create-volume-action]").click();
    await page.locator('input[name="volume-title"]').fill("渡口卷");
    await page.locator('input[name="volume-title"]').press("Enter");
    const createChapter = async (title: string) => {
      await page.locator("[data-add-chapter]").click();
      await page.locator('[data-chapter-placement="append"]').click();
      await page.locator('form[data-create-chapter] input[name="chapter-title"]').fill(title);
      await page.locator('input[name="chapter-title"]').press("Enter");
    };
    const saved = page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]');
    const typeInEditor = async (text: string) => {
      const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
      await editor.click();
      await page.keyboard.insertText(text);
      await saved.waitFor();
    };
    await createChapter("渡口");
    await expect(page.locator("h2")).toHaveText("渡口");
    await typeInEditor(PARAGRAPHS[0]!);
    await createChapter("灯塔");
    const newChapter = page.locator('nav[aria-label="稿件目录"] button[data-chapter-id]', { hasText: "灯塔" });
    await newChapter.waitFor();
    // The workspace of the new Chapter loses the response of its first outline read. Before #1175 the
    // control then stayed absent until a reload.
    let lostTreeRead = false;
    await page.route((url) => url.pathname.endsWith("/manuscript/tree"), async (route) => {
      if (lostTreeRead) return route.continue();
      lostTreeRead = true;
      return route.abort("failed");
    });
    await newChapter.click();
    await expect(page.locator("h2")).toHaveText("灯塔");
    await typeInEditor("海上起雾。");
    await page.keyboard.press("Home");
    for (let index = 0; index < 2; index += 1) await page.keyboard.press("Shift+ArrowRight");
    await page.locator("[data-add-selection-reference]").click();
    await expect(page.locator("[data-selection-reference-label]")).toHaveCount(1);
    await expect(page.locator('[data-selection-reference-label="第 2 章 灯塔（第 1 段）"]')).toHaveCount(1);
    assert.equal(lostTreeRead, true);
    assert.deepEqual(errors, []);
  } finally {
    await page.close();
  }
}
