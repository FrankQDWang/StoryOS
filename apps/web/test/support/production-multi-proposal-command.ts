import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect } from "playwright/test";
import type { BrowserContext } from "playwright";
import { createProjectCommandChallenge, digestUpdateProjectAssistance, getAgentRun,
  getChapter, getProposal, updateProjectAssistance } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunResponse, UpdateProjectAssistanceRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { queryStoryOSPostgres, runStoryOSWorker, sessionFetch } from "./node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
function id() {
  const value = randomUUID();
  const time = Date.now().toString(16).padStart(12, "0");
  return `${time.slice(0, 8)}-${time.slice(8)}-7${value.slice(15)}`;
}

export async function verifyProductionMultiProposal(context: BrowserContext, origin: string) {
  const page = await context.newPage();
  await page.setViewportSize({ width: 1487, height: 1058 });
  page.setDefaultTimeout(10_000);
  try {
    await page.goto(origin);
    await page.locator('input[name="title"]').fill(`Multi-location ${id()}`);
    await page.locator('input[name="title"]').press("Enter");
    await page.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    const projectId = await page.locator('.project-heading[data-project-id]').getAttribute('data-project-id');
    assert.ok(projectId);
    const options = { baseUrl: origin, projectId, fetchImpl: sessionFetch(origin, "session-a") };
    const request: UpdateProjectAssistanceRequest = { command_schema: "storyos.command.update-project-assistance.request.v1",
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
    await page.locator('input[name="volume-title"]').fill("Volume");
    await page.locator('input[name="volume-title"]').press('Enter');
    const chapters: string[] = [];
    for (const [title, lines] of [
      ["Chapter A", ["Keep the narrator voice in this passage.", "Keep the second block voice in this passage."]],
      ["Chapter B", ["A different boat reached the shore."]],
    ] as const) {
      await page.locator('[data-add-chapter]').click();
      await page.locator('[data-chapter-placement="append"]').click();
      await page.locator('input[name="chapter-title"]').fill(title);
      await page.locator('input[name="chapter-title"]').press('Enter');
      const row = page.locator('li[data-chapter-id]').filter({hasText:title});
      await row.waitFor();
      const chapterId = await row.getAttribute('data-chapter-id');
      assert.ok(chapterId); chapters.push(chapterId);
      await row.locator('[data-chapter-menu]').click();
      if (await row.locator('[data-make-current-chapter]').count()) await row.locator('[data-make-current-chapter]').click();
      else await page.locator('[data-add-chapter]').press('Escape');
      const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
      await editor.waitFor();
      await expect(page.locator('.editor-panel h2')).toHaveText(title);
      await editor.click();
      for (const [index, text] of lines.entries()) {
        if (index) await page.keyboard.press('Enter');
        await page.keyboard.insertText(text);
      }
      await expect.poll(async () => (await getChapter({...options,chapterId})).chapter.current_revision.blocks.map(b=>b.text)).toEqual([...lines]);
      await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    }
    let admitted: CreateAgentRunResponse | undefined;
    await page.route(url=>url.pathname.endsWith('/agent-runs'),async route=>{
      if(route.request().method()!=="POST")return route.continue();
      const response=await route.fetch(); admitted=await response.json() as CreateAgentRunResponse;
      await route.fulfill({response});
    });
    await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count=0 WHERE project_id='${projectId}'::uuid`);
    await page.locator('[data-assistant-availability="available"]').waitFor();
    await page.locator('input[name="assistant-message"]').fill('Rewrite the first 2 paragraphs of chapter 1 and the first paragraph of chapter 2');
    await page.locator('.composer button').click();
    await expect.poll(()=>admitted?.effect.kind).toBe('admitted');
    const result=admitted as CreateAgentRunResponse|undefined;
    assert.ok(result?.effect.kind==='admitted');
    const runId=result.effect.run_id;
    for(let i=0;i<8;i++) {
      if((await getAgentRun({...options,runId})).status==='completed')break;
      await runStoryOSWorker({repositoryRoot,workerBinary:join(repositoryRoot,'target/release-package/storyos-worker'),args:['--once']});
    }
    const run=await getAgentRun({...options,runId});
    assert.equal(run.status,'completed'); assert.ok(run.decision.kind==='prose_change');
    assert.equal(run.decision.locations?.length,3);
    await page.locator('[data-assistant-inspect]').click();
    await expect(page.locator('[data-proposal-location]')).toHaveCount(3);
    for(const location of run.decision.locations ?? []) {
      assert.ok(location.outcome.kind!=='refused');
      const link=page.locator(`[data-proposal-location="${location.outcome.operation_id}"]`);
      await expect(link).toContainText(location.explanation);
      await link.click();
      await expect(page.locator('[data-manuscript-editor]')).toHaveAttribute('contenteditable','true');
      const candidate=page.locator(`[data-proposal-id="${location.outcome.proposal_id}"][data-proposal-operation-id="${location.outcome.operation_id}"]`);
      await expect(candidate).toHaveAttribute('data-proposal-focused','true');
      await expect(candidate.locator('.block-proposal-text')).toHaveText(location.candidate_text);
    }
    await page.screenshot({path:join(repositoryRoot,'target/382-multi-pending.png')});
    assert.deepEqual(chapters.sort(),[...new Set(run.decision.locations?.map(l=>l.chapter_id))].sort());
  } finally { await page.close(); }
}
