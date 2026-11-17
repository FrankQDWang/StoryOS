import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext, Page } from "playwright";
import { expect } from "playwright/test";

import { createChapter, digestCreateChapter, getProject }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, USER_A, challenged, drainLeftoverWork, id, prepare, settleRun } from "./acceptance.ts";
import { queryStoryOSPostgres, startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { assertProjectReceipts } from "./project-receipts.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const TITLE = "林舟";
const TEXT = "船夫，不说话。\n夜里看灯。";
/** The time limit of the wait for an enabled send button. The panel reads the earlier Run first, and under host load
 * this read can take longer than the page timeout. */
const SEND_ENABLED_TIMEOUT_MS = 30_000;

/**
 * Types `text` into the composer and waits until the send button is enabled. The composer mounts again when the
 * Project Scope of the panel arrives, and that mount clears its message. So the wait types the text again while the
 * message is not `text`.
 */
async function typeMessage(page: Page, text: string): Promise<void> {
  const message = page.locator('textarea[name="assistant-message"]');
  await expect(async () => {
    if (await message.inputValue() !== text) await message.fill(text);
    await expect(page.locator(".composer .send-button")).toBeEnabled({ timeout: 1_000 });
  }).toPass({ timeout: SEND_ENABLED_TIMEOUT_MS });
}
const MESSAGE = `记住林舟。\nSCRIPT:write_memory create ${JSON.stringify({ type: "character", title: TITLE,
  description: "沉默的船夫。", text: TEXT })}`;

/** Adds Chapter B to the Volume of the prepared Chapter, and returns its identity. */
async function addChapter(baseUrl: string, setup: Awaited<ReturnType<typeof prepare>>): Promise<string> {
  const volumeId = await queryStoryOSPostgres(`SELECT parent_volume_id::text FROM storyos.manuscript_objects
    WHERE manuscript_object_id = '${setup.chapterId}'::uuid`);
  const request = { command_schema: "storyos.command.create-chapter.request.v1" as const,
    create_chapter_input: { title: "Chapter B", expected_tree_revision: "3", ...BINDING, correlation_id: id("b78a17") } };
  const created = await challenged(baseUrl, setup.fetchImpl, setup.projectId, "POST",
    "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters", request.command_schema,
    await digestCreateChapter(request), id("b78a18"), (antiForgery) => createChapter({ baseUrl, projectId: setup.projectId,
      volumeId, fetchImpl: setup.fetchImpl, idempotencyKey: id("b78a18"), antiForgery, request }));
  if (created.effect.kind !== "authoritative_applied") throw new Error("Create Chapter must apply");
  return created.effect.chapter_id;
}

/** Writes entry 2 with the origin `chapter` and the source `chapterId` with SQL, as a background write does. */
async function writeChapterEntry(projectId: string, chapterId: string): Promise<void> {
  await queryStoryOSPostgres(`
    INSERT INTO storyos.memory_entry_versions (owner_user_id, project_id, memory_entry_id, entry_number, version,
        memory_revision, state, entry_type, origin, title, description, text, cause, source_references)
      SELECT owner_user_id, project_id, md5(project_id::text || ':2')::uuid, 2, 1, memory_revision + 1, 'current',
             'foreshadowing', 'chapter', '渡口的灯', '灯的来历。', '灯每夜亮一次。', '{}',
             jsonb_build_array(jsonb_build_object('kind', 'chapter', 'chapter_id', '${chapterId}'))
        FROM storyos.project_memory_heads WHERE project_id = '${projectId}'::uuid;
    UPDATE storyos.project_memory_heads SET memory_revision = memory_revision + 1, next_entry_number = 3
     WHERE project_id = '${projectId}'::uuid;`);
}

/**
 * ADR 0094 section 9: the author asks the assistant to remember an entry, and opens the settings page with the gear.
 * The entry is in the group of its type. It shows its text, its origin line, and the link to its source Conversation.
 * The origin line of an entry from a Chapter opens the workspace at that Chapter (S7-D2c). The author deletes each
 * entry on the settings page, and the next Run sends no Memory index (S7-D2b).
 */
export async function verifyProductionMemorySettings(context: BrowserContext): Promise<void> {
  const started: (() => Promise<unknown>)[] = [];
  try {
    const server = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A },
      serverBinary: join(repositoryRoot, "target/release-package/storyos-server") });
    started.push(() => stopStoryOSServer(server.server));
    const page = await context.newPage();
    started.push(() => page.close());
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.setDefaultTimeout(10_000);
    await page.setViewportSize({ width: 1487, height: 1058 });
    // Each `settleOnce` claims the oldest queued Run, so the Runs that earlier journeys left settle first.
    await drainLeftoverWork();
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-000000b78a00", "Memory journey", "b78a1");
    const workspace = `${server.baseUrl}/projects/${setup.projectId}`;

    // 1. The author sends a message with the Memory write script of the fake model.
    await page.goto(workspace);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await typeMessage(page, MESSAGE);
    const admitted = page.waitForResponse((reply) =>
      reply.request().method() === "POST" && new URL(reply.url()).pathname.endsWith("/agent-runs"));
    await page.locator(".composer .send-button").click();
    const body = await (await admitted).json() as CreateAgentRunResponse;
    assert.ok(body.effect.kind === "admitted");
    const runId = body.effect.run_id;
    assert.equal((await settleRun(server.baseUrl, setup, runId)).status, "completed");
    await page.locator(`[data-assistant-run-id="${runId}"][data-assistant-dispatch="completed"]`).waitFor();

    // 2. The author opens the settings page with the gear, and finds the entry in the group `人物`.
    await page.locator('.tree-footer a[aria-label="项目设置"]').click();
    await page.waitForURL(`${workspace}/settings`);
    const group = page.locator('[data-memory-group="character"]');
    await expect(group.locator("h3")).toHaveText("人物");
    const entry = group.locator("[data-memory-entry]");
    await expect(entry).toHaveCount(1);
    await expect(entry.locator("h4")).toHaveText(TITLE);
    assert.equal(await entry.locator(".memory-entry-text").textContent(), TEXT);
    const origin = entry.locator('a[data-memory-entry-origin="conversation"]');
    await expect(origin).toHaveText("你让助手记住的");
    assert.equal(await origin.getAttribute("href"),
      `/projects/${setup.projectId}?conversation=${body.conversation_id}`);
    await expect(page.locator("[data-memory-group]")).toHaveCount(1);

    // 3. An entry from Chapter B shows its link. The link opens the workspace, and Chapter B becomes current.
    const chapterB = await addChapter(server.baseUrl, setup);
    await writeChapterEntry(setup.projectId, chapterB);
    await page.reload();
    const chapterOrigin = page.locator('[data-memory-group="foreshadowing"] a[data-memory-entry-origin="chapter"]');
    await expect(chapterOrigin).toHaveText("来自章节《Chapter B》");
    assert.equal(await chapterOrigin.getAttribute("href"), `/projects/${setup.projectId}?chapter=${chapterB}`);
    await chapterOrigin.click();
    await expect(page.locator(".editor-panel h2")).toHaveText("Chapter B");
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    assert.equal(new URL(page.url()).search, "");
    const opened = await getProject({ baseUrl: server.baseUrl, projectId: setup.projectId, fetchImpl: setup.fetchImpl });
    assert.deepEqual(opened.project.open, { kind: "current_chapter", current_chapter_id: chapterB });

    // 4. The author deletes each entry with "删除" and "确认删除". Each entry leaves the section with no sentence.
    await page.goto(`${workspace}/settings`);
    for (const type of ["character", "foreshadowing"]) {
      const item = page.locator(`[data-memory-group="${type}"] [data-memory-entry]`);
      await item.locator("[data-memory-delete]").click();
      await item.locator("[data-memory-delete-confirm]").click();
      await expect(page.locator(`[data-memory-group="${type}"]`)).toHaveCount(0);
    }
    await expect(page.locator("[data-memory-empty]")).toHaveText("助手还没有记住任何内容。");
    await expect(page.locator("[data-memory-section] [role=alert]")).toHaveCount(0);

    // 5. The request of the next Run has Context and no Memory index.
    await page.goto(workspace);
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await page.locator('[data-assistant-availability="available"]').waitFor();
    // The wait for the response starts only after the panel has read the earlier Run and enabled the button.
    await typeMessage(page, "继续写。");
    const next = page.waitForResponse((reply) =>
      reply.request().method() === "POST" && new URL(reply.url()).pathname.endsWith("/agent-runs"));
    await page.locator(".composer .send-button").click();
    const nextBody = await (await next).json() as CreateAgentRunResponse;
    assert.ok(nextBody.effect.kind === "admitted");
    await settleRun(server.baseUrl, setup, nextBody.effect.run_id);
    // The fake destination keeps no request bytes, so the check reads the Context items that the request sends.
    const items = await queryStoryOSPostgres(`SELECT count(*) FILTER (WHERE item ->> 'source_class' = 'memory_index')
        || ' ' || (count(*) > 0)::text
      FROM storyos.context_assembly_manifests AS manifest, jsonb_array_elements(manifest.payload -> 'selected') AS item
     WHERE manifest.run_id = '${nextBody.effect.run_id}'::uuid AND manifest.manifest_role = 'decision'
       AND manifest.decision_position = 0`);
    assert.equal(items, "0 true");
    assert.deepEqual(errors, []);

    await assertProjectReceipts([setup.projectId], { projects: 1, receipts: { createProject: 1,
      updateProjectAssistance: 1, createVolume: 1, createChapter: 2, createAgentRun: 2, setCurrentChapter: 1,
      deleteMemoryEntry: 2 } });
  } finally {
    for (const close of started.reverse()) await close();
  }
}
