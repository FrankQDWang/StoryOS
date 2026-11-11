import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { BrowserContext, Page } from "playwright";
import { expect } from "playwright/test";

import {
  getAgentRun, getChapter, getResearchArtifact, listResearchSources,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, prepare, settleOnce } from "./acceptance.ts";
import { queryStoryOSPostgres, startStoryOSServer, stopStoryOSServer } from "./node-integration.ts";
import { assertProjectReceipts } from "./project-receipts.ts";
import {
  HELD_QUERY, MISSING_URL, PAGE_URL, SEARCH_QUERY, callsReply, fakeAgentPlan, fakeExaMcp, functionOutputs, messageReply,
  taggedIdentities,
} from "./research-journey-fakes.ts";

// The Worker takes test-only keys from its process environment and reads no env file (ADR 0050).
const WORKER_ENV = {
  STORYOS_MODEL_DESTINATION: "host_fake",
  STORYOS_AGENT_PLAN_API_KEY: "test-only-agent-plan-key",
  STORYOS_EXA_API_KEY: "test-only-exa-key",
};
const PARAGRAPHS = ["潮水退到码头下面。", "她在雾里等那条船。"];
const PROSE = "上午十点四十二分，涨潮漫过了王子码头。";

/** Points the model and MCP destination evidence of the Project at the fakes, as the Postgres tests do. */
async function pointAtFakes(projectId: string, model: string, mcp: string) {
  await queryStoryOSPostgres(`
    UPDATE storyos.processing_destination_identity_evidence_revisions
       SET endpoint = CASE evidence_kind WHEN 'mcp_remote_boundary' THEN '${mcp}' ELSE '${model}' END
     WHERE project_id = '${projectId}'::uuid AND endpoint IS NOT NULL;
    UPDATE storyos.project_command_challenge_rate_windows SET issued_count = 0
     WHERE project_id = '${projectId}'::uuid`);
}

/** Sends one author message through the panel composer and returns the admitted Run. */
async function send(page: Page, message: string, button = page.locator(".composer .send-button")) {
  const admitted = page.waitForResponse((response) =>
    response.request().method() === "POST" && new URL(response.url()).pathname.endsWith("/agent-runs"));
  if (message.length > 0) await page.locator('[name="assistant-message"]').fill(message);
  await button.click();
  const response = await admitted;
  assert.equal(response.status(), 202);
  return (await response.json() as CreateAgentRunResponse).effect.run_id;
}

export async function verifyProductionResearchJourney(context: BrowserContext): Promise<void> {
  const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
  let blockIds: string[] = [];
  // Each started resource adds its close step, so a failed start closes the resources before it.
  const started: (() => Promise<unknown>)[] = [];
  try {
    const mcp = await fakeExaMcp(repositoryRoot);
    started.push(mcp.close);
    const model = await fakeAgentPlan([
      callsReply("resp_s509_1", [["call_search", "web_search", { query: SEARCH_QUERY, result_count: null }]]),
      callsReply("resp_s509_2", [["call_fetch", "web_fetch", { urls: [PAGE_URL, MISSING_URL] }]]),
      messageReply("resp_s509_3", (body, earlier) => {
        const [search, page] = [...earlier.slice(-1), body].map((request) => taggedIdentities(request, '<source id="'));
        const [gap] = taggedIdentities(body, '<gap id="');
        return JSON.stringify({ kind: "research_synthesis", summary: "1890 年利物浦的潮汐。", claims: [
          { text: "王子码头上午十点四十二分涨潮。", citations: [page![0], search![0]] },
          { text: "港口日志不可用。", citations: [gap] }] });
      }),
      messageReply("resp_s509_4", () => JSON.stringify({ kind: "prose_change", summary: "加入涨潮时间。",
        changes: [{ block_id: blockIds[0], candidate_text: PROSE, explanation: "研究给出了涨潮时间。" }] })),
      // An empty URL list fails the typed intake, so the batch does not validate.
      callsReply("resp_s509_5", [["call_invalid", "web_fetch", { urls: [] }]]),
      callsReply("resp_s509_6", [["call_held", "web_search", { query: HELD_QUERY, result_count: null }]]),
      messageReply("resp_s509_7", () => "这一段可以写得更安静。"),
    ]);
    started.push(model.close);
    const server = await startStoryOSServer({ repositoryRoot, sessions: { "session-a": USER_A },
      serverBinary: join(repositoryRoot, "target/release-package/storyos-server"),
      extraEnv: { STORYOS_MODEL_DESTINATION: "volcengine_agent_plan" } });
    started.push(() => stopStoryOSServer(server.server));
    const page = await context.newPage();
    started.push(() => page.close());
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.setDefaultTimeout(10_000);
    await page.setViewportSize({ width: 1487, height: 1058 });
    const setup = await prepare(server.baseUrl, "018f0000-0000-7001-8000-0000005e0900", "Research journey", "5e091");
    const options = { baseUrl: server.baseUrl, projectId: setup.projectId, fetchImpl: setup.fetchImpl };
    await page.goto(`${server.baseUrl}/projects/${setup.projectId}`);
    await page.locator(`button[data-chapter-id="${setup.chapterId}"]`).click();
    const editor = page.locator('[data-manuscript-editor][contenteditable="true"]');
    const saved = page.locator('[data-save-state="saved"][data-unsettled-intent-count="0"]');
    await editor.click();
    await page.keyboard.insertText(PARAGRAPHS[0]!);
    await page.keyboard.press("Enter");
    await page.keyboard.insertText(PARAGRAPHS[1]!);
    await expect.poll(async () => (await getChapter({ ...options, chapterId: setup.chapterId })).chapter
      .current_revision.blocks.map((block) => block.text)).toEqual(PARAGRAPHS);
    await saved.waitFor();
    blockIds = (await getChapter({ ...options, chapterId: setup.chapterId })).chapter.current_revision.blocks
      .map((block) => block.manuscript_block_id);

    // The author enables web research in the panel. The grant covers both operations, so no prompt follows.
    await page.locator('[data-research-tool="disabled"] input[role="switch"]').click();
    await page.locator('[data-research-tool="enabled"]').waitFor();
    const researchRun = await send(page, "1890 年利物浦的涨潮时间是几点？请查找来源。");
    await pointAtFakes(setup.projectId, model.base, mcp.base);
    await settleOnce(WORKER_ENV);
    const research = await getAgentRun({ ...options, runId: researchRun });
    assert.equal(research.status, "completed");
    assert.ok(research.decision.kind === "research_synthesis");
    // Each continuation gives the model its result under the original call_id, and each call ran once.
    assert.deepEqual(model.bodies.slice(1, 3).map((body) => functionOutputs(body).map((output) => output.call_id)),
      [["call_search"], ["call_fetch"]]);
    // The first request exposes exactly the two research functions, and the server gets the arguments of the model.
    assert.deepEqual((model.bodies[0]!.tools as { name: string }[]).map((tool) => tool.name), ["web_search", "web_fetch"]);
    assert.deepEqual(mcp.calls().map((call) => call.params as { name: string; arguments: Record<string, unknown> })
      .map(({ name, arguments: { query, urls } }) => [name, query ?? urls]),
    [["web_search_exa", SEARCH_QUERY], ["web_fetch_exa", [PAGE_URL, MISSING_URL]]]);

    // The panel shows the settled sources, the gap, and the synthesis with numbered citations.
    await page.reload();
    const sources = page.locator("[data-assistant-research-sources] li");
    await expect(sources).toHaveText([/^Liverpool tide table 1890（获取于 /u, /^Mersey port log（获取于 /u,
      /^Liverpool tide table 1890（获取于 /u, `${MISSING_URL}：无法获取（读取失败）`]);
    const artifactPanel = page.locator("[data-assistant-research-artifact]");
    await expect(artifactPanel.locator('ul[aria-label="研究结论"] li'))
      .toHaveText(["王子码头上午十点四十二分涨潮。 [1][2]", "港口日志不可用。 [3]"]);
    await expect(page.locator("[data-assistant-research-citations] li")).toHaveText([
      /^\[1\] Liverpool tide table 1890（获取于 /u, /^\[2\] Liverpool tide table 1890（获取于 /u,
      `[3] ${MISSING_URL}：无法获取（读取失败）`]);
    // The exact lines of the current turn, which the history turn shows again after the next Run.
    const shownResearch = {
      sources: await sources.allTextContents(),
      claims: await artifactPanel.locator('ul[aria-label="研究结论"] li').allTextContents(),
      citations: await page.locator("[data-assistant-research-citations] li").allTextContents(),
    };
    const artifactRead = { ...options, researchArtifactId: research.decision.research_artifact_id };
    // Each read has its own correlation identity, so the comparison leaves it out.
    const keptRecords = async () => [await getResearchArtifact(artifactRead), await listResearchSources({ ...options, runId: researchRun })]
      .map(({ correlation_id: _, ...read }) => read);
    const kept = await keptRecords();

    // "用到故事里" asks for a prose Proposal in the same Conversation, and the author rejects it.
    const storyRun = await send(page, "", artifactPanel.locator("[data-assistant-use-research]"));
    await pointAtFakes(setup.projectId, model.base, mcp.base);
    await settleOnce(WORKER_ENV);
    const story = await getAgentRun({ ...options, runId: storyRun });
    assert.equal(story.conversation_id, research.conversation_id);
    assert.ok(story.decision.kind === "prose_change");
    const location = story.decision.locations?.[0]?.outcome;
    assert.ok(location?.kind === "opened");
    assert.equal(await queryStoryOSPostgres(`SELECT source_lineage->>'kind' || ':' || (source_lineage->>'research_artifact_id')
      FROM storyos.proposals WHERE proposal_id = '${location.proposal_id}'::uuid`),
    `research_artifact:${research.decision.research_artifact_id}`);
    await page.reload();
    await expect(page.locator("[data-assistant-result]")).toContainText("这次修改涉及 1 个章节、1 处文字");
    // The synthesis turn is in the history now. It shows the same sources, claims, and citations, and it has no
    // "用到故事里" button, because that button sends in the current Conversation (#1427).
    const researchTurn = page.locator(`[data-assistant-history-run="${researchRun}"]`);
    await expect(researchTurn.locator("[data-assistant-research-sources] li")).toHaveText(shownResearch.sources);
    await expect(researchTurn.locator('ul[aria-label="研究结论"] li')).toHaveText(shownResearch.claims);
    await expect(researchTurn.locator("[data-assistant-research-citations] li")).toHaveText(shownResearch.citations);
    await expect(researchTurn.locator("[data-assistant-use-research]")).toHaveCount(0);
    await page.locator(`[data-proposal-id="${location.proposal_id}"] [data-proposal-reject]`).click();
    await expect(page.locator(`[data-proposal-id="${location.proposal_id}"]`)).toHaveCount(0);
    await saved.waitFor();
    assert.deepEqual(await keptRecords(), kept);

    // The model sends a research request that StoryOS cannot accept. The Run refuses, and no Tool runs.
    const invalidRun = await send(page, "再查一下 1890 年的港口日志。");
    await pointAtFakes(setup.projectId, model.base, mcp.base);
    await settleOnce(WORKER_ENV);
    assert.deepEqual((await getAgentRun({ ...options, runId: invalidRun })).decision,
      { kind: "execution_refused", capability: "tool_request_invalid" });
    await page.reload();
    await expect(page.locator("[data-assistant-result]"))
      .toHaveText("模型发出的联网检索请求不符合要求，正文没有改动。你可以重试，或继续手动写作。");

    // The author cancels a Run in the panel while the fake Exa server holds its search answer. The cancel fences the
    // dispatched ToolCall, and the Worker drops the exchange at its next lease renewal (ADR 0086, #1425).
    const held = mcp.hold();
    const cancelRun = await send(page, "再查一下 1890 年利物浦港口的早晨记录。");
    await pointAtFakes(setup.projectId, model.base, mcp.base);
    // A lease of 3 seconds makes the Worker renew it each second, so it reads the fence soon after the cancel.
    const worker = settleOnce({ ...WORKER_ENV, STORYOS_EXPORT_LEASE_TTL_SECS: "3" });
    started.push(() => worker.catch(() => undefined));
    // The dispatch claim commits before the exchange, so the held call has a dispatched ToolCall.
    await expect.poll(() => mcp.calls().length).toBe(3);
    await page.locator("[data-cancel-agent-run]").click();
    await expect(page.getByRole("status")).toHaveText("任务已取消。");
    const uncertain = `可能已联网搜索，但没有收到结果：${HELD_QUERY}`;
    await expect(page.locator("[data-assistant-result]")).toHaveText(uncertain);
    await expect(page.locator("[data-cancel-agent-run]")).toHaveCount(0);
    // The Worker exits only after it drops the exchange. Then the fake answers late, to a closed connection.
    await worker;
    assert.deepEqual(held.release(), { connectionOpen: false });
    const cancelled = await getAgentRun({ ...options, runId: cancelRun });
    assert.equal(cancelled.status, "cancelled");
    assert.deepEqual(cancelled.decision.kind === "tool_request" ? cancelled.decision.calls : cancelled.decision,
      [{ kind: "search", query: HELD_QUERY, outcome: "unknown" }]);
    const unknownSources = await listResearchSources({ ...options, runId: cancelRun });
    assert.deepEqual({ snapshots: unknownSources.snapshots, gaps: unknownSources.gaps,
      unknown: unknownSources.unknown_calls.map(({ call, late_result }) => ({ call, late_result })) },
    { snapshots: [], gaps: [], unknown: [{ call: { kind: "search", query: HELD_QUERY, outcome: "unknown" }, late_result: false }] });
    // The fence of the cancel settled the ToolCall, and no late answer reached the store.
    assert.equal(await queryStoryOSPostgres(`SELECT state || ':' || (attempt->'fence'->>'cause') || ':'
        || (attempt->'fence'->>'local_stop') || ':' || (attempt ? 'late_observation')::text
      FROM storyos.tool_calls WHERE run_id = '${cancelRun}'::uuid`), "unknown:run_cancelled:drop_exchange:false");
    await page.reload();
    await expect(page.locator("[data-assistant-result]")).toHaveText(uncertain);

    // The author disables web research. The next request exposes no Tool, and manual writing saves.
    await page.locator('[data-research-tool="enabled"] input[role="switch"]').click();
    await page.locator('[data-research-tool="disabled"]').waitFor();
    const plainRun = await send(page, "这一段怎么写得更安静？");
    await pointAtFakes(setup.projectId, model.base, mcp.base);
    await settleOnce(WORKER_ENV);
    assert.equal((await getAgentRun({ ...options, runId: plainRun })).decision.kind, "advisory");
    assert.equal(model.bodies.length, 7);
    assert.equal("tools" in model.bodies[6]!, false);
    assert.equal(mcp.calls().length, 3);
    await editor.click();
    await page.keyboard.press("ControlOrMeta+End");
    await page.keyboard.insertText("雾散了。");
    await expect.poll(async () => (await getChapter({ ...options, chapterId: setup.chapterId })).chapter
      .current_revision.blocks.map((block) => block.text)).toEqual([PARAGRAPHS[0], `${PARAGRAPHS[1]}雾散了。`]);
    await saved.waitFor();
    assert.deepEqual(errors, []);
    await assertProjectReceipts([setup.projectId], { projects: 1, receipts: { createProject: 1, updateProjectAssistance: 1,
      createVolume: 1, createChapter: 1, applyAuthorEdit: 4, setProjectToolEnablement: 2, createAgentRun: 5,
      rejectProposalOperations: 1, cancelAgentRun: 1 }, authorActions: 7 });
  } finally {
    for (const close of started.reverse()) await close();
  }
}
