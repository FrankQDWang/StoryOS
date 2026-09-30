import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect } from "playwright/test";
import type { BrowserContext } from "playwright";
import {
  createProjectCommandChallenge, digestUpdateProjectAssistance, getAgentRun,
  getChapter, getEditorSession, getProposal, updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, ApplyAuthorEditRequest, CreateAgentRunResponse, UpdateProjectAssistanceRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { queryStoryOSPostgres, runStoryOSWorker, sessionFetch } from "./node-integration.ts";

function uuidV7() {
  const value = randomUUID();
  const time = Date.now().toString(16).padStart(12, "0");
  return `${time.slice(0, 8)}-${time.slice(8)}-7${value.slice(15)}`;
}

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const SOURCE = "Guard the narrator voice in this passage.";
const SIBLING = "The second door opened.";

export async function verifyProductionInlineProposal(context: BrowserContext, origin: string) {
  const page = await context.newPage();
  await page.setViewportSize({ width: 1487, height: 1058 });
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.setDefaultTimeout(10_000);
  try {
    assert.equal((await page.goto(origin))?.status(), 200);
    await page.locator('#app[data-boot-state="protected-ready"]').waitFor();
    await page.locator('input[name="title"]').fill(`Inline Proposal ${uuidV7()}`);
    await page.locator('input[name="title"]').press("Enter");
    await page.locator('#app[data-boot-state="empty-project-ready"]').waitFor();
    const projectId = await page.locator("form[data-rename]").getAttribute("data-rename");
    assert.ok(projectId);
    const options = { baseUrl: origin, projectId, fetchImpl: sessionFetch(origin, "session-a") };
    const request: UpdateProjectAssistanceRequest = {
      command_schema: "storyos.command.update-project-assistance.request.v1",
      update_project_assistance_input: { availability: "available", expected_assistance_revision: "0",
        client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: uuidV7() },
    };
    const idempotencyKey = uuidV7();
    const challenge = await createProjectCommandChallenge({ ...options, request: {
      method: "PUT", route_template: "/api/v1/projects/{project_id}/assistance",
      command_schema: request.command_schema, canonical_command_digest: await digestUpdateProjectAssistance(request),
      idempotency_key: idempotencyKey,
    } });
    await updateProjectAssistance({ ...options, request, idempotencyKey, antiForgery: challenge.nonce });
    await page.locator('input[name="volume-title"]').fill("Inline Volume");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator('input[name="chapter-title"]').fill("Inline Chapter");
    await page.locator('input[name="chapter-title"]').press("Enter");
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    await page.goto(`${origin}/projects/${projectId}`);
    const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
    await editor.waitFor();
    let chapterId = (await page.locator('nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]')
      .getAttribute("data-chapter-id"))!;
    assert.ok(chapterId);
    await editor.click();
    await page.keyboard.insertText(SOURCE);
    await page.keyboard.press("Enter");
    await page.keyboard.insertText(SIBLING);
    await expect.poll(async () => (await getChapter({ ...options, chapterId })).chapter.current_revision.blocks.map((block) => block.text))
      .toEqual([SOURCE, SIBLING]);
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    const before = await getChapter({ ...options, chapterId });
    assert.deepEqual(before.chapter.current_revision.blocks.map((block) => block.text), [SOURCE, SIBLING]);
    let admitted: CreateAgentRunResponse | undefined;
    await page.route((url) => url.pathname.endsWith("/agent-runs"), async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      const response = await route.fetch();
      admitted = await response.json() as CreateAgentRunResponse;
      await route.fulfill({ response });
    });
    const openInline = async () => {
      admitted = undefined;
      await page.locator('[data-assistant-availability="available"]').waitFor();
      await page.locator('input[name="assistant-message"]').fill("Revise this phrase: keep the voice.");
      await page.locator(".composer button").click();
      await expect.poll(() => admitted?.effect.kind).toBe("admitted");
      const admission = admitted as CreateAgentRunResponse | undefined;
      assert.ok(admission?.effect.kind === "admitted");
      const runId = admission.effect.run_id;
      for (let attempt = 0; attempt < 8; attempt += 1) {
        if ((await getAgentRun({ ...options, runId })).status === "completed") break;
        await runStoryOSWorker({ repositoryRoot, workerBinary: join(repositoryRoot,
          "target", "release-package", "storyos-worker"), args: ["--once"] });
      }
      const run = await getAgentRun({ ...options, runId });
      assert.equal(run.status, "completed");
      assert.ok(run.decision.kind === "prose_change" && run.decision.opened_proposal.kind === "present");
      const proposalId = run.decision.opened_proposal.proposal_id;
      const proposal = (await getProposal({ ...options, proposalId })).proposal;
      assert.equal(proposal.kind, "inline_edit");
      assert.equal(proposal.validation, "valid");

      await page.locator('[data-assistant-inspect]').last().click();
      return { proposalId, proposal };
    };
    const { proposalId, proposal } = await openInline();
    const candidate = page.locator(`span[data-inline-proposal-id="${proposalId}"]`);
    await expect(candidate).toHaveCount(1);
    assert.equal(await candidate.textContent(), "narrator tone");
    assert.equal(await candidate.evaluate((element) => element.parentElement?.textContent),
      "Guard the narrator tone in this passage.");
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, before.chapter);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-ready.png") });
    await page.locator(`[data-proposal-id="${proposalId}"][data-proposal-eligibility="eligible"]`).waitFor();
    await editor.waitFor();
    const candidateRequests: { key: string | undefined; body: string | null }[] = [];
    let finishCandidateLoss = () => {};
    const candidateLost = new Promise<void>((resolve) => { finishCandidateLoss = resolve; });
    const editRoute = (url: URL) => url.pathname.endsWith("/manuscript/author-edits");
    await page.route(editRoute, async (route) => {
      candidateRequests.push({ key: (await route.request().allHeaders())["idempotency-key"],
        body: route.request().postData() });
      const response = await route.fetch();
      assert.equal(response.status(), 200);
      if (candidateRequests.length === 1) { await route.abort("failed"); finishCandidateLoss(); }
      else await route.fulfill({ response });
    });
    await candidate.click();
    await candidate.evaluate((element) => {
      const range = document.createRange();
      range.setStart(element.firstChild!, 5); range.collapse(true);
      const selection = window.getSelection()!;
      selection.removeAllRanges(); selection.addRange(range);
    });
    await page.keyboard.insertText("xx");

    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.candidate_text)
      .toBe("narraxxtor tone");
    await candidateLost;
    const edited = (await getProposal({ ...options, proposalId })).proposal;
    assert.notEqual(edited.revision_id, proposal.revision_id);
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, before.chapter);
    await page.reload();
    await expect(candidate).toHaveText("narraxxtor tone");
    assert.equal(await candidate.getAttribute("data-proposal-revision-id"), edited.revision_id);
    await page.locator(`[data-proposal-id="${proposalId}"][data-proposal-eligibility="eligible"]`).waitFor();
    assert.ok(candidateRequests.length >= 1);
    assert.ok(candidateRequests.every((request) => JSON.stringify(request) === JSON.stringify(candidateRequests[0])));
    const frozenEdit = JSON.parse(candidateRequests[0]!.body!) as ApplyAuthorEditRequest;
    assert.deepEqual(frozenEdit.proposal_target, {
      proposal_id: proposalId, operation_id: proposal.operations[0]!.operation_id,
      revision_id: proposal.revision_id, manuscript_block_id: proposal.manuscript_block_id,
    });
    await page.unroute(editRoute);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-edited-reloaded.png") });
    let acceptanceRequest: AcceptProposalRequest | undefined;
    let acceptancePosts = 0;
    let finishAcceptanceLoss = () => {};
    const acceptanceLost = new Promise<void>((resolve) => { finishAcceptanceLoss = resolve; });
    let acceptanceKey: string | undefined;
    let acceptanceBytes: string | undefined;
    await page.route((url) => url.pathname.endsWith(`/proposals/${proposalId}/acceptances`), async (route) => {
      acceptancePosts += 1;
      const key = (await route.request().allHeaders())["idempotency-key"];
      if (acceptancePosts === 1) { acceptanceKey = key; acceptanceBytes = route.request().postData() ?? undefined; }
      else { assert.equal(key, acceptanceKey); assert.equal(route.request().postData(), acceptanceBytes); }
      acceptanceRequest = route.request().postDataJSON() as AcceptProposalRequest;
      const response = await route.fetch();
      assert.equal(response.status(), 200);
      if (acceptancePosts <= 2) await route.abort("failed");
      else await route.fulfill({ response });
      if (acceptancePosts === 2) finishAcceptanceLoss();
    });
    await page.locator(`[data-proposal-accept="${proposalId}"]`).click();
    await expect.poll(async () => (await getProposal({ ...options, proposalId })).proposal.operation_resolution)
      .toBe("applied");
    await acceptanceLost;
    assert.equal(acceptancePosts, 2);
    assert.equal(acceptanceRequest?.accept_proposal_input.proposal_revision_id, edited.revision_id);
    assert.deepEqual(acceptanceRequest?.accept_proposal_input.selected_operation_ids,
      [edited.operations[0]!.operation_id]);
    await page.reload();
    const accepted = await getChapter({ ...options, chapterId });
    assert.deepEqual(accepted.chapter.current_revision.blocks, [
      { ...before.chapter.current_revision.blocks[0]!, text: "Guard the narraxxtor tone in this passage." },
      before.chapter.current_revision.blocks[1]!,
    ]);
    await expect(page.locator("[data-manuscript-editor] > p")).toHaveText([
      "Guard the narraxxtor tone in this passage.", SIBLING,
    ]);
    await expect(candidate).toHaveCount(0);
    assert.equal(acceptancePosts, 2, "reload must not resubmit an uncertain explicit decision");
    await page.getByRole("button", { name: "重试接受", exact: true }).click();
    await expect.poll(() => acceptancePosts).toBe(3);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-accepted-recovered.png") });
    const createInlineChapter = async (title: string) => {
      await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
      await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
        WHERE project_id = '${projectId}'::uuid`);
      await page.locator('form[data-create-chapter] input[name="chapter-title"]').fill(title);
      await page.locator('form[data-create-chapter] input[name="chapter-title"]').press("Enter");
      await page.locator('nav[aria-label="稿件目录"] button[data-chapter-id]').filter({ hasText: title }).click();
      chapterId = (await page.locator('nav[aria-label="稿件目录"] button[data-chapter-id]').filter({ hasText: title })
        .getAttribute("data-chapter-id"))!;
      assert.ok(chapterId);
      await page.locator(`[data-make-current-chapter="${chapterId}"]`).click();
      await editor.waitFor(); await editor.click(); await page.keyboard.insertText(SOURCE);
      await expect.poll(async () => (await getChapter({ ...options, chapterId: chapterId! })).chapter.current_revision.body)
        .toBe(SOURCE);
      await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
      await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
        WHERE project_id = '${projectId}'::uuid`);
    };
    await createInlineChapter("Inline Rejection Chapter");
    const rejectBase = await getChapter({ ...options, chapterId });
    const second = await openInline();
    const secondCandidate = page.locator(`span[data-inline-proposal-id="${second.proposalId}"]`);
    await expect(secondCandidate).toHaveText("narrator tone");
    await page.locator(`[data-proposal-id="${second.proposalId}"][data-proposal-eligibility="eligible"]`).waitFor();
    let mixedRequest: ApplyAuthorEditRequest | undefined;
    let mixedResponse: unknown;
    let edgeLossArmed = false;
    const edgeRequests: { key: string | undefined; body: string | null }[] = [];
    await page.route((url) => url.pathname.endsWith("/author-edits"), async (route) => {
      mixedRequest = route.request().postDataJSON() as ApplyAuthorEditRequest;
      const response = await route.fetch();
      mixedResponse = await response.json();
      if (edgeLossArmed || edgeRequests.length > 0) {
        edgeRequests.push({ key: (await route.request().allHeaders())["idempotency-key"],
          body: route.request().postData() });
      }
      if (edgeLossArmed) { edgeLossArmed = false; await route.abort("failed"); }
      else await route.fulfill({ response });
    });
    await editor.focus();
    await secondCandidate.evaluate((element) => {
      const range = document.createRange();
      range.setStart(element.parentElement!.firstChild!, 6);
      range.setEnd(element.firstChild!, 5);
      const selection = window.getSelection()!;
      selection.removeAllRanges(); selection.addRange(range);
      document.dispatchEvent(new Event("selectionchange"));
    });
    assert.equal(await secondCandidate.evaluate(() => window.getSelection()?.toString()), "the narra");
    await page.keyboard.insertText("Preserve my complete mixed input.");
    await expect(page.locator("[data-draft-replacement]")).toHaveText("Preserve my complete mixed input.").catch((error) => {
      throw new Error(`${error}\nMixed input request and response: ${JSON.stringify({ mixedRequest, mixedResponse })}`);
    });
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, rejectBase.chapter);
    assert.deepEqual((await getProposal({ ...options, proposalId: second.proposalId })).proposal, second.proposal);
    const sources = mixedRequest?.author_edit_units[0]?.selection_snapshot.ordered_selection?.sources;
    assert.equal(sources?.length, 2);
    assert.deepEqual(sources?.map((source) => [source.owner.kind, source.source_text]),
      [["manuscript", SOURCE], ["proposal", "narrator tone"]]);
    await editor.focus();
    await secondCandidate.evaluate((element) => {
      window.getSelection()?.setBaseAndExtent(element.firstChild!, 3, element.parentElement!.lastChild!, 8);
      document.dispatchEvent(new Event("selectionchange"));
    });
    await page.keyboard.insertText("Preserve reverse mixed input.");
    const preservedInputs = ["Preserve my complete mixed input.", "Preserve reverse mixed input."];
    await expect(page.locator("[data-draft-replacement]")).toHaveText(preservedInputs);
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, rejectBase.chapter);
    assert.deepEqual((await getProposal({ ...options, proposalId: second.proposalId })).proposal, second.proposal);
    assert.deepEqual(mixedRequest?.author_edit_units[0]?.selection_snapshot.ordered_selection?.sources
      .map((source) => [source.owner.kind, source.source_text]),
      [["proposal", "narrator tone"], ["manuscript", SOURCE]]);
    await page.reload();
    await expect(page.locator("[data-draft-replacement]")).toHaveText(preservedInputs);
    await expect(secondCandidate).toHaveText("narrator tone");
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-mixed-preserved.png") });
    await page.locator(`[data-proposal-reject="${second.proposalId}"]`).click();
    await expect.poll(async () => (await getProposal({ ...options, proposalId: second.proposalId })).proposal.operation_resolution)
      .toBe("rejected");
    await page.reload();
    await expect(page.locator(`span[data-inline-proposal-id="${second.proposalId}"]`)).toHaveCount(0);
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, rejectBase.chapter);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-rejected-reloaded.png") });
    for (const edge of ["start", "end"] as const) {
      await createInlineChapter(`Inline ${edge} edge Chapter`);
      await editor.focus();
      await page.keyboard.press("Enter");
      await page.keyboard.insertText(SIBLING);
      await expect.poll(async () => (await getChapter({ ...options, chapterId })).chapter.current_revision.blocks.map((block) => block.text))
        .toEqual([SOURCE, SIBLING]);
      await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
      await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
        WHERE project_id = '${projectId}'::uuid`);
      const edgeBase = await getChapter({ ...options, chapterId });
      const opened = await openInline();
      const edgeCandidate = page.locator(`span[data-inline-proposal-id="${opened.proposalId}"]`);
      await expect(edgeCandidate).toHaveText("narrator tone");
      await page.locator(`[data-proposal-id="${opened.proposalId}"][data-proposal-eligibility="eligible"]`).waitFor();
      await editor.focus();
      await edgeCandidate.evaluate((element, edge) => {
        const offset = edge === "start" ? 0 : element.textContent!.length;
        window.getSelection()?.setBaseAndExtent(element.firstChild!, offset, element.firstChild!, offset);
        document.dispatchEvent(new Event("selectionchange"));
      }, edge);
      edgeRequests.length = 0;
      edgeLossArmed = edge === "start";
      await page.keyboard.insertText("!");
      const expectedText = edge === "start" ? "Guard the !narrator voice in this passage."
        : "Guard the narrator voice! in this passage.";
      await expect.poll(async () => (await getChapter({ ...options, chapterId })).chapter.current_revision.blocks)
        .toEqual([{ ...edgeBase.chapter.current_revision.blocks[0]!, text: expectedText },
          edgeBase.chapter.current_revision.blocks[1]!]).catch((error) => {
          throw new Error(`${error}\nEdge request and response: ${JSON.stringify({ mixedRequest, mixedResponse })}`);
        });
      const retainEdgeFacts = async () => {
        const journal = await page.evaluate(async () => {
          const result: Record<string, unknown> = {};
          for (const item of await indexedDB.databases()) {
            if (item.name == null) continue;
            const database = await new Promise<IDBDatabase>((resolve, reject) => {
              const opening = indexedDB.open(item.name!);
              opening.onsuccess = () => resolve(opening.result); opening.onerror = () => reject(opening.error);
            });
            const stores = ["intents", "submission_groups", "metadata"].filter((store) => database.objectStoreNames.contains(store));
            const values: Record<string, unknown> = {};
            for (const store of stores) values[store] = await new Promise((resolve, reject) => {
              const request = database.transaction(store).objectStore(store).getAll();
              request.onsuccess = () => resolve(request.result); request.onerror = () => reject(request.error);
            });
            result[item.name] = values; database.close();
          }
          return result;
        });
        const session = mixedRequest === undefined ? undefined : await getEditorSession({ ...options,
          editorSessionId: mixedRequest.editor_session_id });
        await writeFile(join(repositoryRoot, "target", "issue-828", `inline-${edge}-edge-facts.json`),
          JSON.stringify({ request: mixedRequest, response: mixedResponse, edgeRequests, session, journal }, null, 2));
      };
      await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor().catch(async (error) => {
        await retainEdgeFacts(); throw error;
      });
      await retainEdgeFacts();
      if (edge === "start") {
        assert.ok(edgeRequests.length >= 1);
        assert.ok(edgeRequests.every((request) => JSON.stringify(request) === JSON.stringify(edgeRequests[0])));
      }
      assert.deepEqual(mixedRequest?.expected_proposal_head_revision_ids, [opened.proposal.revision_id]);
      assert.equal(mixedRequest?.proposal_target, undefined);
      const afterEdge = (await getProposal({ ...options, proposalId: opened.proposalId })).proposal;
      assert.equal(afterEdge.revision_id, opened.proposal.revision_id);
      assert.equal(afterEdge.candidate_text, "narrator tone");
      await page.reload();
      await expect(page.locator("[data-manuscript-editor] > p")).toHaveText([expectedText, SIBLING]);
      await expect(page.locator(`[data-proposal-id="${opened.proposalId}"] .block-proposal-text`)).toHaveText("narrator tone");
      await expect(page.locator(`[data-proposal-accept="${opened.proposalId}"]`)).toHaveCount(0);
      await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", `inline-${edge}-edge-reloaded.png`) });
    }
    await createInlineChapter("Inline writer refusal Chapter");
    const writerBase = await getChapter({ ...options, chapterId });
    const writerProposal = await openInline();
    const writerCandidate = page.locator(`span[data-inline-proposal-id="${writerProposal.proposalId}"]`);
    await expect(writerCandidate).toHaveText("narrator tone");
    await page.locator(`[data-proposal-id="${writerProposal.proposalId}"][data-proposal-eligibility="eligible"]`).waitFor();
    const observer = await context.newPage();
    try {
      await observer.setViewportSize({ width: 1487, height: 1058 });
      assert.equal((await observer.goto(`${origin}/projects/${projectId}`))?.status(), 200);
      await observer.locator("[data-take-over-writer]").click();
      await observer.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
      let refusedPosts = 0;
      await page.route((url) => url.pathname.endsWith(`/proposals/${writerProposal.proposalId}/acceptances`), async (route) => {
        refusedPosts += 1;
        await route.continue();
      });
      await page.locator(`[data-proposal-accept="${writerProposal.proposalId}"]`).click();
      await page.locator(`[data-proposal-id="${writerProposal.proposalId}"][data-proposal-session="ineligible"]`).waitFor();
      assert.equal(refusedPosts, 1);
      assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, writerBase.chapter);
      const refusedProposal = (await getProposal({ ...options, proposalId: writerProposal.proposalId })).proposal;
      assert.equal(refusedProposal.revision_id, writerProposal.proposal.revision_id);
      assert.equal(refusedProposal.candidate_text, "narrator tone");
      assert.equal(refusedProposal.operation_resolution, "pending");
      await page.reload();
      await expect(writerCandidate).toHaveText("narrator tone");
      await expect(page.locator(`[data-proposal-accept="${writerProposal.proposalId}"]`)).toHaveCount(0);
      assert.equal(refusedPosts, 1, "reload must not submit a stale writer decision");
      await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-writer-refused-reloaded.png") });
    } finally { await observer.close(); }
    assert.deepEqual(errors, []);
  } catch (error) {
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-failure.png") });
    throw error;
  } finally { await page.close(); }
}
