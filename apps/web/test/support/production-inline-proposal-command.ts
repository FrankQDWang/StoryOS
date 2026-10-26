import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect } from "playwright/test";
import type { BrowserContext, Locator } from "playwright";
import {
  acceptProposal, createProjectCommandChallenge, digestAcceptProposal, digestUpdateProjectAssistance, getAgentRun,
  getChapter, getEditorSession, getProposal, updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AcceptProposalRequest, ApplyAuthorEditRequest, AssistanceWorkingTarget, CreateAgentRunResponse, UpdateProjectAssistanceRequest }
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
const CANDIDATE_LABEL = "第 1 章 Inline Chapter（第 1 段的修改建议）";

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
    const projectId = await page.locator("[data-project-id]").getAttribute("data-project-id");
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
    await page.locator("[data-add-chapter]").click();
    await page.locator("[data-create-volume-action]").click();
    await page.locator('input[name="volume-title"]').fill("Inline Volume");
    await page.locator('input[name="volume-title"]').press("Enter");
    await page.locator("[data-add-chapter]").click();
    await page.locator('[data-chapter-placement="append"]').click();
    await page.locator('form[data-create-chapter] input[name="chapter-title"]').fill("Inline Chapter");
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
    let requestTarget: AssistanceWorkingTarget | undefined;
    await page.route((url) => url.pathname.endsWith("/agent-runs"), async (route) => {
      if (route.request().method() !== "POST") return route.continue();
      requestTarget = route.request().postDataJSON().create_agent_run_input.working_target;
      const response = await route.fetch();
      admitted = await response.json() as CreateAgentRunResponse;
      await route.fulfill({ response });
    });
    const openInline = async () => {
      admitted = undefined;
      await page.locator('[data-assistant-availability="available"]').waitFor();
      await page.locator('[name="assistant-message"]').fill("Revise this phrase: keep the voice.");
      await page.locator(".composer button.send-button").click();
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

      await expect(page.locator('[data-assistant-inspect]')).toHaveCount(0);
      return { proposalId, proposal };
    };
    const { proposalId, proposal } = await openInline();
    const candidate = page.locator(`span[data-inline-proposal-id="${proposalId}"]`);
    /** Selects the text between two offsets of an inline candidate, as a mouse selection does. */
    const selectIn = (target: Locator, start: number, end: number) => target.evaluate((element, [from, to]) => {
      element.closest<HTMLElement>("[data-manuscript-editor]")!.focus();
      document.getSelection()!.setBaseAndExtent(element.firstChild!, from, element.firstChild!, to);
    }, [start, end] as const);
    const selectCandidate = (start: number, end: number) => selectIn(candidate, start, end);
    /** The references of the next message, from the draft of the Project in the session storage. */
    const draftReferences = () => page.evaluate((project) => {
      const key = Object.keys(sessionStorage).find((item) => item.startsWith("selection_references:")
        && item.endsWith(`:${project}`));
      return key === undefined ? undefined
        : (JSON.parse(sessionStorage.getItem(key)!) as { entries: { reference: unknown }[] }).entries.map((item) => item.reference);
    }, projectId);
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
    let edited = (await getProposal({ ...options, proposalId })).proposal;
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
    // The author selects all of the candidate text and adds it to the message (ADR 0062).
    await selectCandidate(0, edited.operations[0]!.candidate_text.length);
    await page.locator("[data-add-selection-reference]").click();
    await expect(page.locator(`[data-selection-reference-label="${CANDIDATE_LABEL}"]`)).toHaveCount(1);
    admitted = undefined;
    await page.locator('[name="assistant-message"]').fill("Revise the selected candidate and preserve its meaning.");
    await page.locator(".composer button.send-button").click();
    await expect.poll(() => admitted?.effect.kind).toBe("admitted");
    assert.deepEqual(requestTarget, { kind: "selection_references", source_chapter_id: chapterId, references: [{
      kind: "proposal_candidate_text", chapter_id: chapterId, proposal_id: proposalId,
      operation_id: edited.operations[0]!.operation_id, revision_id: edited.revision_id,
      coordinate_profile: "storyos.editor.utf16-code-unit.v1", start_offset: 0,
      end_offset: edited.operations[0]!.candidate_text.length }] });
    const revisionAdmission = admitted as CreateAgentRunResponse | undefined;
    assert.ok(revisionAdmission?.effect.kind === "admitted");
    const revisionRunId = revisionAdmission.effect.run_id;
    for (let attempt = 0; attempt < 8; attempt += 1) {
      if ((await getAgentRun({ ...options, runId: revisionRunId })).status === "completed") break;
      await runStoryOSWorker({ repositoryRoot, workerBinary: join(repositoryRoot,
        "target", "release-package", "storyos-worker"), args: ["--once"] });
    }
    const revisionRun = await getAgentRun({ ...options, runId: revisionRunId });
    assert.equal(revisionRun.status, "completed");
    assert.ok(revisionRun.decision.kind === "prose_change"
      && revisionRun.decision.locations?.[0]?.outcome.kind === "revised");
    const aiEdited = (await getProposal({ ...options, proposalId })).proposal;
    assert.equal(aiEdited.candidate_text, "narraxxtor tone Keep the voice consistent.");
    assert.notEqual(aiEdited.revision_id, edited.revision_id);
    assert.equal(aiEdited.operations[0]!.operation_id, edited.operations[0]!.operation_id);
    edited = aiEdited;
    await expect(page.locator('[data-assistant-inspect]')).toHaveCount(0);
    await expect(candidate).toHaveText("narraxxtor tone Keep the voice consistent.");
    await expect(candidate).toHaveAttribute("data-proposal-revision-id", edited.revision_id);
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, before.chapter);
    await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count=0 WHERE project_id='${projectId}'::uuid`);
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
    // The acceptance result is lost, so the client cannot know the accepted text: the reference goes
    // when the candidate goes.
    await selectCandidate(0, "narraxxtor".length);
    await page.locator("[data-add-selection-reference]").click();
    await expect(page.locator(`[data-selection-reference-label="${CANDIDATE_LABEL}"]`)).toHaveCount(1);
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
      { ...before.chapter.current_revision.blocks[0]!, text: "Guard the narraxxtor tone Keep the voice consistent. in this passage." },
      before.chapter.current_revision.blocks[1]!,
    ]);
    await expect(page.locator("[data-manuscript-editor] > p")).toHaveText([
      "Guard the narraxxtor tone Keep the voice consistent. in this passage.", SIBLING,
    ]);
    await expect(candidate).toHaveCount(0);
    assert.equal(acceptancePosts, 2, "reload must not resubmit an uncertain explicit decision");
    await page.getByRole("button", { name: "重试接受", exact: true }).click();
    await expect.poll(() => acceptancePosts).toBe(3);
    await expect(page.getByRole("button", { name: "重试接受", exact: true })).toHaveCount(0);
    await expect(page.locator("[data-selection-reference-label]")).toHaveCount(0);
    await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count=0 WHERE project_id='${projectId}'::uuid`);
    await page.locator('[data-manuscript-editor] > p').first().click();
    await page.keyboard.press('End'); await page.keyboard.insertText(' Continue after Inline.');
    await expect.poll(async () => (await getChapter({ ...options, chapterId })).chapter.current_revision.blocks[0]?.text)
      .toContain('Continue after Inline.');
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-accepted-recovered.png") });
    const createInlineChapter = async (title: string) => {
      await page.locator('[data-manuscript-editor][contenteditable="true"]').waitFor();
      await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
        WHERE project_id = '${projectId}'::uuid`);
      await page.locator("[data-add-chapter]").click();
      await page.locator('[data-chapter-placement="append"]').click();
      await page.locator('form[data-create-chapter] input[name="chapter-title"]').fill(title);
      await page.locator('form[data-create-chapter] input[name="chapter-title"]').press("Enter");
      await page.locator('nav[aria-label="稿件目录"] button[data-chapter-id]').filter({ hasText: title }).click();
      chapterId = (await page.locator('nav[aria-label="稿件目录"] button[data-chapter-id]').filter({ hasText: title })
        .getAttribute("data-chapter-id"))!;
      assert.ok(chapterId);
      // The earlier Chapter stays editable until the switch opens the new Chapter.
      await expect(page.getByRole("heading", { name: title, exact: true })).toBeVisible({ timeout: 15_000 });
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
    let editRequest: ApplyAuthorEditRequest | undefined;
    let editResponse: unknown;
    let edgeLossArmed = false;
    const edgeRequests: { key: string | undefined; body: string | null }[] = [];
    await page.route((url) => url.pathname.endsWith("/author-edits"), async (route) => {
      editRequest = route.request().postDataJSON() as ApplyAuthorEditRequest;
      const response = await route.fetch();
      editResponse = await response.json();
      if (edgeLossArmed || edgeRequests.length > 0) {
        edgeRequests.push({ key: (await route.request().allHeaders())["idempotency-key"],
          body: route.request().postData() });
      }
      if (edgeLossArmed) { edgeLossArmed = false; await route.abort("failed"); }
      else await route.fulfill({ response });
    });
    const proposalText = async () => (await getProposal({ ...options, proposalId: second.proposalId })).proposal.candidate_text;
    // A selection from the candidate into the Chapter text changes only the candidate (ADR 0072).
    await editor.focus();
    await secondCandidate.evaluate((element) => {
      window.getSelection()?.setBaseAndExtent(element.firstChild!, 3, element.parentElement!.lastChild!, 8);
      document.dispatchEvent(new Event("selectionchange"));
    });
    await page.keyboard.insertText(" first owner");
    await expect.poll(proposalText).toBe("nar first owner").catch((error) => {
      throw new Error(`${error}\nCandidate request and response: ${JSON.stringify({ editRequest, editResponse })}`);
    });
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, rejectBase.chapter);
    assert.equal(editRequest?.proposal_target?.proposal_id, second.proposalId);
    await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor();
    // A selection from the Chapter text into the candidate sends only a Chapter edit. The Server refuses each
    // Chapter edit outside the edges of a pending inline Proposal, so the edit conflicts (ADR 0072, #1273).
    await editor.focus();
    await secondCandidate.evaluate((element) => {
      const range = document.createRange();
      range.setStart(element.parentElement!.firstChild!, 6);
      range.setEnd(element.firstChild!, 5);
      const selection = window.getSelection()!;
      selection.removeAllRanges(); selection.addRange(range);
      document.dispatchEvent(new Event("selectionchange"));
    });
    assert.equal(await secondCandidate.evaluate(() => window.getSelection()?.toString()), "the nar f");
    await page.keyboard.insertText("a ");
    await expect.poll(() => (editResponse as { effect?: { kind?: string; reason?: string } } | undefined)?.effect)
      .toMatchObject({ kind: "conflicted", reason: "proposal_head_present" }).catch((error) => {
        throw new Error(`${error}\nChapter request and response: ${JSON.stringify({ editRequest, editResponse })}`);
      });
    assert.equal(editRequest?.proposal_target, undefined);
    assert.deepEqual(editRequest?.author_edit_units.flatMap((unit) => unit.normalized_primitives)
      .map((primitive) => primitive.kind), ["replace_block_selection"]);
    assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, rejectBase.chapter);
    assert.equal(await proposalText(), "nar first owner");
    assert.equal((await queryStoryOSPostgres(`SELECT count(*) FROM storyos.draft_artifacts
      WHERE project_id = '${projectId}'::uuid`)).trim(), "0");
    await page.reload();
    await expect(page.locator(`[data-proposal-id="${second.proposalId}"]`)).toContainText("nar first owner");
    await expect(page.locator("[data-refused-edit-draft]")).toHaveCount(0);
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-first-owner-edits.png") });
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
          throw new Error(`${error}\nEdge request and response: ${JSON.stringify({ editRequest, editResponse })}`);
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
        const session = editRequest === undefined ? undefined : await getEditorSession({ ...options,
          editorSessionId: editRequest.editor_session_id });
        await writeFile(join(repositoryRoot, "target", "issue-828", `inline-${edge}-edge-facts.json`),
          JSON.stringify({ request: editRequest, response: editResponse, edgeRequests, session, journal }, null, 2));
      };
      await page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]').waitFor().catch(async (error) => {
        await retainEdgeFacts(); throw error;
      });
      await retainEdgeFacts();
      if (edge === "start") {
        assert.ok(edgeRequests.length >= 1);
        assert.ok(edgeRequests.every((request) => JSON.stringify(request) === JSON.stringify(edgeRequests[0])));
      }
      assert.deepEqual(editRequest?.expected_proposal_head_revision_ids, [opened.proposal.revision_id]);
      assert.equal(editRequest?.proposal_target, undefined);
      const afterEdge = (await getProposal({ ...options, proposalId: opened.proposalId })).proposal;
      assert.equal(afterEdge.revision_id, opened.proposal.revision_id);
      assert.equal(afterEdge.candidate_text, "narrator tone");
      await page.reload();
      await expect(page.locator("[data-manuscript-editor] > p")).toHaveText([expectedText, SIBLING]);
      await expect(page.locator(`[data-proposal-id="${opened.proposalId}"] .block-proposal-text`)).toHaveText("narrator tone");
      // The edge edit moved the Chapter Head, so the Proposal is on an earlier base (ADR 0064).
      await expect(page.locator(`[data-proposal-accept="${opened.proposalId}"]`)).toHaveCount(1);
      await expect(page.locator(`[data-proposal-reject="${opened.proposalId}"]`)).toHaveCount(1);
      await expect(page.locator(`[data-proposal-earlier-base="${opened.proposalId}"]`))
        .toHaveText("正文已变化。接受会用候选文字替换目标的当前文字。");
      await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", `inline-${edge}-edge-reloaded.png`) });
      if (edge === "end") {
        // An Acceptance on the moved head proves the Anchor range on the current head and applies the
        // candidate there (ADR 0064).
        const head = (await getChapter({ ...options, chapterId })).chapter.current_revision.revision_id;
        if (afterEdge.validation_receipt.kind !== "present") throw new Error("expected a validation receipt");
        const writerSession = (await queryStoryOSPostgres(`SELECT current_editor_session_id::text
          FROM storyos.project_writer_generations WHERE project_id = '${projectId}'::uuid
          ORDER BY writer_generation DESC LIMIT 1`)).trim();
        await queryStoryOSPostgres(`UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
          WHERE project_id = '${projectId}'::uuid`);
        const acceptance: AcceptProposalRequest = { command_schema: "storyos.command.accept-proposal.request.v1",
          accept_proposal_input: { proposal_revision_id: afterEdge.revision_id,
            validation_receipt_id: afterEdge.validation_receipt.validation_receipt_id,
            selected_operation_ids: [afterEdge.operation_id], expected_authoritative_revision_id: head,
            editor_session_id: writerSession,
            client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
            security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: uuidV7() } };
        const acceptanceKey = uuidV7();
        const acceptanceChallenge = await createProjectCommandChallenge({ ...options, request: {
          method: "POST", route_template: "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
          command_schema: acceptance.command_schema, canonical_command_digest: await digestAcceptProposal(acceptance),
          idempotency_key: acceptanceKey } });
        const accepted = await acceptProposal({ ...options, proposalId: opened.proposalId, request: acceptance,
          idempotencyKey: acceptanceKey, antiForgery: acceptanceChallenge.nonce });
        if (accepted.effect.kind !== "applied") throw new Error(`expected applied, got ${JSON.stringify(accepted.effect)}`);
        assert.equal(accepted.effect.target_resolution, "proven_range");
        assert.equal(accepted.effect.authoritative_revision.body, `Guard the narrator tone! in this passage.\n${SIBLING}`);
        await page.reload();
        await expect(page.locator("[data-manuscript-editor] > p")).toHaveText(["Guard the narrator tone! in this passage.", SIBLING]);
        await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-accepted-moved-head.png") });
      }
    }
    // A reference to part of an inline candidate becomes a reference to the same Chapter text when the
    // author accepts the candidate. The candidate starts at offset 10 of its block (ADR 0062).
    await createInlineChapter("Inline accepted reference Chapter");
    const referenced = await openInline();
    const referencedCandidate = page.locator(`span[data-inline-proposal-id="${referenced.proposalId}"]`);
    await expect(referencedCandidate).toHaveText("narrator tone");
    await page.locator(`[data-proposal-id="${referenced.proposalId}"][data-proposal-eligibility="eligible"]`).waitFor();
    await selectIn(referencedCandidate, 0, "narrator".length);
    await page.locator("[data-add-selection-reference]").click();
    await expect(page.locator('[data-selection-reference-label$="（第 1 段的修改建议）"]')).toHaveCount(1);
    await page.locator(`[data-proposal-accept="${referenced.proposalId}"]`).click();
    await expect(page.locator('[data-selection-reference-label$="（第 1 段）"]')).toHaveCount(1);
    const referencedChapter = (await getChapter({ ...options, chapterId })).chapter;
    const referencedBlock = referencedChapter.current_revision.blocks[0]!.manuscript_block_id;
    assert.equal(referencedChapter.current_revision.blocks[0]!.text.slice(10, 18), "narrator");
    assert.deepEqual(await draftReferences(), [{ kind: "chapter_text", chapter_id: chapterId,
      base_authoritative_revision_id: referencedChapter.current_revision.revision_id,
      coordinate_profile: "storyos.editor.utf16-code-unit.v1",
      start: { manuscript_block_id: referencedBlock, offset: 10 }, end: { manuscript_block_id: referencedBlock, offset: 18 } }]);
    await page.locator("[data-remove-selection-reference]").click();
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
      assert.equal(refusedPosts, 0, "the read-only Editor Session preflight must block the stale writer before submission");
      assert.deepEqual((await getChapter({ ...options, chapterId })).chapter, writerBase.chapter);
      const refusedProposal = (await getProposal({ ...options, proposalId: writerProposal.proposalId })).proposal;
      assert.deepEqual(refusedProposal, writerProposal.proposal);
      await page.reload();
      await expect(writerCandidate).toHaveText("narrator tone");
      await expect(page.locator(`[data-proposal-accept="${writerProposal.proposalId}"]`)).toHaveCount(0);
      assert.equal(refusedPosts, 0, "reload must not submit a stale writer decision");
      await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-writer-refused-reloaded.png") });
    } finally { await observer.close(); }
    assert.deepEqual(errors, []);
  } catch (error) {
    await page.screenshot({ path: join(repositoryRoot, "target", "issue-828", "inline-failure.png") });
    throw error;
  } finally { await page.close(); }
}
