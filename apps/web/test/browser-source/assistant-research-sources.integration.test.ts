import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import type {
  ListResearchSourcesResponse, SourceGapReason,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import agentRunFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-agent-run.json";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import sourcesFixture from "../../../../generated/golden-wire/storyos-public-release-1/list-research-sources.json";
import { researchSourceLines } from "../../src/assistant-run-outcome.ts";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse } from "./scenario.ts";

const RUN = "018f0000-0000-7001-8000-000000000c91";

it("lists the sources of the current turn by title or URL with the capture time, a gap as not available with its reason, and a call that may have run", async () => {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify({
    scope, message: "Check the tide tables of 1890.", chapterId: CHAPTER,
    correlationId: "018f0000-0000-7001-8000-000000000c92", idempotencyKey: "018f0000-0000-7001-8000-000000000c93",
    snapshotId: "018f0000-0000-7001-8000-000000000c94", runId: RUN, conversationId: agentRunFixture.conversation_id,
  }));
  const [entry] = sourcesFixture.snapshots;
  const page = { ...entry!, kind: "page", title: null, locator: "https://example.org/port-log",
    source_snapshot_id: "018f0000-0000-7001-8000-000000000c95", captured_at: "2026-11-10T03:05:00.000Z" };
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    if (path.endsWith(`/agent-runs/${RUN}`)) {
      return jsonResponse({ ...agentRunFixture, project_scope: scope, run_id: RUN, status: "completed",
        decision: { kind: "advisory", decision_id: "018f0000-0000-7001-8000-000000000c96", text: "High water was at 10.42." } });
    }
    if (path.endsWith(`/agent-runs/${RUN}/research-sources`)) {
      return jsonResponse({ ...sourcesFixture, project_scope: scope, run_id: RUN, snapshots: [entry, page] });
    }
    throw new Error(`Unexpected request ${path}`);
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await act(async () => {
      root.render(createElement(WritingAssistantPanel, {
        collapsed: false,
        context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
        scheduleRefresh: () => () => {},
      }));
    });
    for (let turn = 0; turn < 8; turn += 1) await act(async () => { await Promise.resolve(); });

    const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
    const list = host.querySelector("[data-assistant-research-sources]");
    expect({
      lines: [...list?.querySelectorAll("li") ?? []].map(item => item.textContent),
      showsIdentifier: [RUN, entry!.source_snapshot_id, page.source_snapshot_id, entry!.tool_call_id,
        sourcesFixture.gaps[0]!.source_gap_id, sourcesFixture.unknown_calls[0]!.tool_call_id,
        entry!.content_digest].some(id => host.textContent?.includes(id) ?? false),
    }).toEqual({
      lines: [
        `Liverpool tide table 1890（获取于 ${time.format(new Date("2026-11-10T03:00:00.000Z"))}）`,
        `https://example.org/port-log（获取于 ${time.format(new Date("2026-11-10T03:05:00.000Z"))}）`,
        "https://example.org/port：无法获取（读取失败）",
        "可能已联网搜索，但没有收到结果：harbour log of 1890",
      ],
      showsIdentifier: false,
    });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});

it("shows the reason of each gap in the source list with the words of the citation list", () => {
  const [gap] = sourcesFixture.gaps;
  const reasons: SourceGapReason[] = ["failed", "empty", "missing", "unattributed"];
  const sources: ListResearchSourcesResponse = {
    ...sourcesFixture, snapshots: [], unknown_calls: [],
    gaps: reasons.map(reason => ({ ...gap!, locator: `https://example.org/${reason}`, reason })),
  };
  expect(researchSourceLines(sources, undefined)).toEqual([
    "https://example.org/failed：无法获取（读取失败）",
    "https://example.org/empty：无法获取（网页没有文字）",
    "https://example.org/missing：无法获取（检索结果中没有这个网页）",
    "https://example.org/unattributed：无法获取（无法确认内容来自这个网页）",
  ]);
});
