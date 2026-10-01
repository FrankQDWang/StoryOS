import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect } from "playwright/test";
import type { BrowserContext } from "playwright";
import { createProjectCommandChallenge, digestUpdateProjectAssistance, getAgentRun,
  getChapter, getProposal, updateProjectAssistance } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunRequest, CreateAgentRunResponse, AcceptProposalRequest, UpdateProjectAssistanceRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
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
  let phase = "setup";
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
    let requestTarget: CreateAgentRunRequest["create_agent_run_input"]["working_target"] | undefined;
    await page.route(url=>url.pathname.endsWith('/agent-runs'),async route=>{
      if(route.request().method()!=="POST")return route.continue();
      requestTarget = (route.request().postDataJSON() as CreateAgentRunRequest).create_agent_run_input.working_target;
      const response=await route.fetch(); admitted=await response.json() as CreateAgentRunResponse;
      await route.fulfill({response});
    });
    const completeRun = async (message: string) => {
      admitted = undefined;
      await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count=0 WHERE project_id='${projectId}'::uuid`);
      await page.locator('[data-assistant-availability="available"]').waitFor();
      await page.locator('input[name="assistant-message"]').fill(message);
      await page.locator('.composer button').click();
      await expect.poll(() => admitted?.effect.kind).toBe('admitted');
      const result = admitted as CreateAgentRunResponse | undefined;
      assert.ok(result?.effect.kind === 'admitted');
      const runId = result.effect.run_id;
      for (let attempt = 0; attempt < 8; attempt++) {
        if ((await getAgentRun({ ...options, runId })).status === 'completed') break;
        await runStoryOSWorker({ repositoryRoot, workerBinary: join(repositoryRoot, 'target/release-package/storyos-worker'), args: ['--once'] });
      }
      const completed = await getAgentRun({ ...options, runId });
      assert.equal(completed.status, 'completed');
      assert.ok(completed.decision.kind === 'prose_change');
      await page.locator('[data-assistant-inspect]').click();
      return completed;
    };
    const requestMessage = 'Rewrite the first 2 paragraphs of chapter 1 and the first paragraph of chapter 2';
    const run = await completeRun(requestMessage);
    assert.ok(run.decision.kind === 'prose_change');
    assert.equal(run.decision.locations?.length,3);
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
    const locations = run.decision.locations!;
    const first = locations[0]!;
    const secondary = locations[1]!;
    assert.ok(first.outcome.kind !== 'refused' && secondary.outcome.kind !== 'refused');
    const firstOutcome = first.outcome;
    const secondaryOutcome = secondary.outcome;
    const proposalId = secondaryOutcome.proposal_id;
    const proposalRoute = (url: URL) => url.pathname.endsWith(`/proposals/${proposalId}`);
    for (const fault of ['stale', 'deleted', 'wrong_scope']) {
      phase = fault;
      let observedFault = false;
      let faultReads = 0;
      await page.route(proposalRoute, async route => {
        const response = await route.fetch();
        const body = await response.json();
        if (fault === 'stale' && ++faultReads === 1) return route.fulfill({ response });
        if (fault === 'stale') body.proposal.revision_id = id();
        if (fault === 'wrong_scope') body.project_scope.project_id = id();
        await route.fulfill(fault === 'deleted' ? { status: 404, json: { code: 'not_found' } } : { response, json: body });
        observedFault = true;
      });
      await page.locator(`[data-proposal-location="${firstOutcome.operation_id}"]`).click();
      await expect(page.locator('.editor-panel [role="alert"]')).toBeVisible();
      await expect.poll(() => observedFault).toBe(true);
      await expect(page.locator('.editor-panel h2')).toHaveText(fault === 'stale' ? 'Chapter A' : 'Chapter B');
      await expect(page.locator('[data-manuscript-editor]')).toHaveAttribute('contenteditable', 'true');
      await page.unroute(proposalRoute);
      await page.locator('[data-proposal-return]').click();
      await expect(page.locator('.editor-panel h2')).toHaveText('Chapter B');
      await expect(page.locator('.editor-panel [role="alert"]')).toHaveCount(0);
    }
    let releaseHeld!: () => void;
    let observedHeld!: () => void;
    const held = new Promise<void>(resolve => { releaseHeld = resolve; });
    const observed = new Promise<void>(resolve => { observedHeld = resolve; });
    let navigationReads = 0;
    await page.route(proposalRoute, async route => {
      const response = await route.fetch();
      if (++navigationReads === 2) { observedHeld(); await held; }
      await route.fulfill({ response });
    });
    await page.locator(`[data-proposal-location="${firstOutcome.operation_id}"]`).click();
    await observed;
    assert.ok(locations[2]!.outcome.kind !== 'refused');
    await page.locator(`[data-proposal-location="${locations[2]!.outcome.operation_id}"]`).click();
    releaseHeld(); await page.unroute(proposalRoute, undefined);
    await expect(page.locator('.editor-panel h2')).toHaveText('Chapter B');
    await expect(page.locator(`[data-proposal-operation-id="${locations[2]!.outcome.operation_id}"]`)).toHaveAttribute('data-proposal-focused', 'true');
    await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count=0 WHERE project_id='${projectId}'::uuid`);
    phase = "candidate-edit";
    const before = (await getProposal({ ...options, proposalId })).proposal;
    const retainedHistory = async () => JSON.parse(await queryStoryOSPostgres(`SELECT jsonb_build_object(
      'revision', (SELECT to_jsonb(record) FROM storyos.proposal_revisions AS record
        WHERE project_id='${projectId}'::uuid AND revision_id='${before.revision_id}'::uuid),
      'validation', (SELECT to_jsonb(record) FROM storyos.validation_receipts AS record
        WHERE project_id='${projectId}'::uuid AND proposal_revision_id='${before.revision_id}'::uuid))::text`));
    const originalHistory = await retainedHistory();
    await page.locator(`[data-proposal-location="${secondaryOutcome.operation_id}"]`).click();
    const candidate = () => page.locator(`[data-proposal-id="${proposalId}"][data-proposal-operation-id="${secondaryOutcome.operation_id}"]`);
    await expect(candidate()).toHaveAttribute('data-proposal-focused', 'true');
    await expect.poll(() => page.evaluate(() => window.getSelection()?.anchorNode?.parentElement
      ?.closest('[data-proposal-operation-id]')?.getAttribute('data-proposal-operation-id'))).toBe(secondaryOutcome.operation_id);
    await page.keyboard.press('End');
    await page.keyboard.insertText(' Manual candidate edit.');
    await expect(candidate().locator('.block-proposal-text')).toHaveText(`${secondary.candidate_text} Manual candidate edit.`);
    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.operations
      .find(operation => operation.operation_id === secondaryOutcome.operation_id)?.candidate_text)
      .toBe(`${secondary.candidate_text} Manual candidate edit.`);
    const manual = (await getProposal({ ...options, proposalId })).proposal;
    assert.notEqual(manual.revision_id, before.revision_id);
    assert.equal(manual.operations.find(operation => operation.operation_id === firstOutcome.operation_id)?.candidate_text,
      before.operations.find(operation => operation.operation_id === firstOutcome.operation_id)?.candidate_text);
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    const revised = await completeRun('Make the selected candidate calmer and preserve its meaning');
    assert.deepEqual(requestTarget, { kind: 'proposal_candidate', source_chapter_id: secondary.chapter_id,
      target: { proposal_id: proposalId, operation_id: secondaryOutcome.operation_id, revision_id: manual.revision_id } });
    assert.ok(revised.decision.kind === 'prose_change');
    assert.ok(revised.decision.locations?.[0]?.outcome.kind === 'revised');
    const ai = (await getProposal({ ...options, proposalId })).proposal;
    assert.notEqual(ai.revision_id, manual.revision_id);
    await expect(candidate()).toHaveAttribute('data-proposal-revision-id', ai.revision_id);
    const primary = page.locator(`[data-proposal-id="${proposalId}"][data-proposal-operation-id="${firstOutcome.operation_id}"]`);
    phase = "accept-primary";
    await primary.locator('[data-proposal-accept]:not([data-proposal-all])').click();
    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.operations
      .find(operation => operation.operation_id === firstOutcome.operation_id)?.resolution).toBe('applied');
    await expect(primary).toHaveCount(0);
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await expect(candidate().locator('[data-proposal-accept]')).toBeVisible();
    const authorBlock = page.locator(`[data-manuscript-editor] > p[data-id="${first.manuscript_block_id}"]`);
    await expect(authorBlock).toHaveText(ai.operations.find(operation => operation.operation_id === firstOutcome.operation_id)!.candidate_text);
    await authorBlock.click();
    await expect.poll(() => page.evaluate(() => document.activeElement?.hasAttribute('data-manuscript-editor'))).toBe(true);
    await page.keyboard.press('End');
    await page.keyboard.insertText(' Author continues writing.');
    await expect.poll(async () => (await getChapter({ ...options, chapterId: first.chapter_id })).chapter.current_revision.blocks
      .find(block => block.manuscript_block_id === first.manuscript_block_id)?.text)
      .toContain('Author continues writing.');
    const conflicted = (await getProposal({ ...options, proposalId })).proposal;
    assert.equal(conflicted.operations.find(operation => operation.operation_id === secondaryOutcome.operation_id)?.resolution, 'pending');
    await candidate().locator('[data-proposal-accept]').click();
    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.source_condition.kind).toBe('proposal_conflict');
    await candidate().locator('[data-proposal-replan]').click();
    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.revision_id).not.toBe(conflicted.revision_id);
    await expect(candidate()).not.toHaveAttribute('contenteditable', 'false');
    const replanned = (await getProposal({ ...options, proposalId })).proposal;
    assert.notEqual(replanned.revision_id, conflicted.revision_id);
    assert.equal(replanned.operations.find(operation => operation.operation_id === firstOutcome.operation_id)?.resolution, 'applied');
    await candidate().locator('.block-proposal-text').click();
    await page.keyboard.press('End');
    await page.keyboard.insertText(' Edit after Replan.');
    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.operations
      .find(operation => operation.operation_id === secondaryOutcome.operation_id)?.candidate_text).toContain('Edit after Replan.');
    assert.deepEqual(await retainedHistory(), originalHistory);
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await candidate().locator('[data-proposal-reject]').click();
    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.operations
      .find(operation => operation.operation_id === secondaryOutcome.operation_id)?.resolution).toBe('rejected');
    await page.locator(`[data-assistant-history-inspect="${run.run_id}"]`).click();
    const third = locations[2]!;
    assert.ok(third.outcome.kind !== 'refused');
    await page.locator(`[data-proposal-location="${third.outcome.operation_id}"]`).click();
    const thirdOutcome = third.outcome;
    const thirdCandidate = page.locator(`[data-proposal-id="${thirdOutcome.proposal_id}"]`);
    await expect(thirdCandidate).toHaveAttribute('data-proposal-focused', 'true');
    await thirdCandidate.locator('[data-proposal-reject]').click();
    await expect.poll(async () => (await getProposal({ ...options, proposalId: thirdOutcome.proposal_id })).proposal.operations
      .find(operation => operation.operation_id === thirdOutcome.operation_id)?.resolution).toBe('rejected');
    phase = "return";
    await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count=0 WHERE project_id='${projectId}'::uuid`);
    await page.locator(`.assistant-exchange:has([data-assistant-inspect]) [data-proposal-return]`).click();
    await expect(page.locator('.editor-panel h2')).toHaveText('Chapter A');
    await page.locator(`.assistant-exchange:has([data-assistant-history-inspect="${run.run_id}"]) [data-proposal-return]`).click();
    await expect(page.locator('.editor-panel h2')).toHaveText('Chapter B');
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    phase = "whole-request";
    const wholeRun = await completeRun(requestMessage);
    assert.ok(wholeRun.decision.kind === 'prose_change');
    const whole = wholeRun.decision.locations?.[0];
    assert.ok(whole && whole.outcome.kind !== 'refused');
    const wholeOutcome = whole.outcome;
    await page.locator(`[data-proposal-location="${wholeOutcome.operation_id}"]`).click();
    const wholeCandidate = page.locator(`[data-proposal-id="${wholeOutcome.proposal_id}"]`).first();
    await expect(wholeCandidate.locator('[data-proposal-accept][data-proposal-all]')).toBeVisible();
    await page.screenshot({path:join(repositoryRoot,'target/382-multi-pending.png')});
    const bodies: string[] = [];
    const keys: string[] = [];
    let finishLoss!: () => void;
    const lost = new Promise<void>(resolve => { finishLoss = resolve; });
    await page.route(url => url.pathname.endsWith(`/proposals/${wholeOutcome.proposal_id}/acceptances`), async route => {
      bodies.push(route.request().postData()!);
      keys.push((await route.request().allHeaders())['idempotency-key']!);
      const response = await route.fetch();
      if (bodies.length <= 2) await route.abort('failed');
      else await route.fulfill({ response });
      if (bodies.length === 2) finishLoss();
    });
    await wholeCandidate.locator('[data-proposal-accept][data-proposal-all]').click();
    await lost;
    assert.equal(bodies.length, 2);
    const frozen = JSON.parse(bodies[0]!) as AcceptProposalRequest;
    assert.equal(frozen.accept_proposal_input.selected_operation_ids.length, 2);
    await page.reload();
    await expect(page.locator('[data-proposal-return]').last()).toBeVisible();
    assert.equal(bodies.length, 2);
    await page.getByRole('button', { name: '重试接受', exact: true }).click();
    await expect.poll(() => bodies.length).toBe(3);
    assert.deepEqual(bodies, [bodies[0], bodies[0], bodies[0]]);
    assert.deepEqual(keys, [keys[0], keys[0], keys[0]]);
    await expect.poll(async () => (await getProposal({ ...options, proposalId: wholeOutcome.proposal_id })).proposal.operations
      .map(operation => operation.resolution)).toEqual(['applied', 'applied']);
    await page.screenshot({ path: join(repositoryRoot, 'target/382-multi-settled.png') });
    assert.deepEqual(chapters.sort(),[...new Set(run.decision.locations?.map(l=>l.chapter_id))].sort());
  } catch (error) {
    await writeFile(join(repositoryRoot, 'target/382-ui-stop.json'), JSON.stringify({ phase, observed: await page.evaluate(() => {
      const surface = document.querySelector('[data-manuscript-editor]') as HTMLElement & { editor?: import('@tiptap/core').Editor };
      const editor = surface.editor;
      const candidates: unknown[] = [];
      editor?.state.doc.descendants((node, position) => {
        if (node.type.name === 'blockProposal') candidates.push({ position, attrs: node.attrs });
      });
      return { requests: Object.entries(sessionStorage).filter(([key]) => key.startsWith('prose_request:')),
      selection: editor?.state.selection.toJSON(), candidates,
      attached: editor?.view.dom.isConnected, destroyed: editor?.isDestroyed,
      active: document.activeElement?.outerHTML,
      anchor: window.getSelection()?.anchorNode?.parentElement?.outerHTML,
      offset: window.getSelection()?.anchorOffset,
      focus: window.getSelection()?.focusNode?.parentElement?.outerHTML,
      editor: document.querySelector('[data-manuscript-editor]')?.outerHTML,
    }; }) }, null, 2));
    await page.screenshot({ path: join(repositoryRoot, 'target/382-ui-stop.png') });
    throw error;
  } finally { await page.close(); }
}
