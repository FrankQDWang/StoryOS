import assert from "node:assert/strict";
import { expect } from "playwright/test";
import type { BrowserContext } from "playwright";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { uuidV7 } from "../../src/acceptance-journal.ts";
import { createProjectCommandChallenge, digestUpdateProjectAssistance, getChapter,
  updateProjectAssistance } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { sessionFetch } from "./node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const id = () => uuidV7(globalThis.crypto);

export async function verifyProductionComposerControls(context: BrowserContext, origin: string) {
  const page = await context.newPage();
  await page.setViewportSize({ width: 1487, height: 1058 });
  page.setDefaultTimeout(10_000);
  try {
    await page.goto(origin);
    await page.locator('input[name="title"]').fill(`Composer ${id()}`);
    await page.locator('input[name="title"]').press("Enter");
    await page.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    const projectId = await page.locator('[data-project-id]').getAttribute("data-project-id");
    assert.ok(projectId);
    const options = { baseUrl: origin, projectId, fetchImpl: sessionFetch(origin, "session-a") };
    const request: import("../../../../generated/typescript/storyos-public-release-1/client.mjs").UpdateProjectAssistanceRequest = { command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: { availability: "available", expected_assistance_revision: "0",
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: id() } };
    const key = id();
    const challenge = await createProjectCommandChallenge({ ...options, request: { method: "PUT",
      route_template: "/api/v1/projects/{project_id}/assistance", command_schema: request.command_schema,
      canonical_command_digest: await digestUpdateProjectAssistance(request), idempotency_key: key } });
    await updateProjectAssistance({ ...options, request, idempotencyKey: key, antiForgery: challenge.nonce });
    await page.locator('[data-add-chapter]').click();
    await page.locator('[data-create-volume-action]').click();
    await page.locator('input[name="volume-title"]').fill("Composer Volume");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator('[data-add-chapter]').click();
    await page.locator('[data-chapter-placement="append"]').click();
    await page.locator('input[name="chapter-title"]').fill("Composer Chapter");
    await page.locator('input[name="chapter-title"]').press("Enter");
    const chapterId = await page.locator('nav[aria-label="稿件目录"] button[aria-current="true"]')
      .getAttribute("data-chapter-id");
    assert.ok(chapterId);
    await page.locator('[data-manuscript-editor][contenteditable="true"]').click();
    await page.keyboard.insertText("The lantern went dark.");
    await expect.poll(async () => (await getChapter({ ...options, chapterId })).chapter.current_revision.body)
      .toBe("The lantern went dark.");
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await page.goto(`${origin}/projects/${projectId}`);
    await page.locator('[data-assistant-availability="available"]').waitFor();
    const composer = page.locator('textarea[name="assistant-message"]');
    await expect(composer).toHaveCount(1);
    await composer.fill("Revise this passage: keep the voice.");
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-875", "composer-two-lines.png") });
  } finally {
    await page.close();
  }
}
