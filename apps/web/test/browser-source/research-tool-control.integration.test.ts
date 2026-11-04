import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import challengeFixture from "../../../../generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json";
import enablementFixture from "../../../../generated/golden-wire/storyos-public-release-1/set-project-tool-enablement.json";
import { digestSetProjectToolEnablement } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ProjectResearchTool, SetProjectToolEnablementEffect, SetProjectToolEnablementRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  RESEARCH_TOOL_LABEL, RESEARCH_TOOL_SERVICE_SENTENCE, RESEARCH_TOOL_UNAVAILABLE_SENTENCE,
} from "../../src/research-tool-control.tsx";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { CHAPTER, OWNER, PROJECT, jsonResponse, requestHeaders, requireRequestBody } from "./scenario.ts";

const scope = { owner_user_id: OWNER, project_id: PROJECT };

/** A fake server with one research tool state. It records each request and settles each enablement with `effect`. */
function fakeServer(state: ProjectResearchTool, effect?: SetProjectToolEnablementEffect) {
  const requests: { path: string; method: string; headers: Headers; body?: Record<string, unknown> }[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    const method = init?.method ?? "GET";
    requests.push({ path, method, headers: requestHeaders(init),
      ...(method === "GET" ? {} : { body: requireRequestBody(init) }) });
    if (path.endsWith("/assistance")) {
      return jsonResponse({ ...assistanceFixture, project_scope: scope, research_tool: state });
    }
    if (path.endsWith("/anti-forgery-challenges")) return jsonResponse(challengeFixture);
    if (path.endsWith("/tool-enablement") && effect !== undefined) {
      return jsonResponse({ ...enablementFixture, project_scope: scope, effect });
    }
    throw new Error(`Unexpected request ${method} ${path}`);
  };
  return { fetchImpl, requests };
}

async function renderPanel(fetchImpl: typeof fetch, run: (host: HTMLElement) => Promise<void>) {
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
    await settle();
    await run(host);
  } finally {
    await act(async () => root.unmount());
    host.remove();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
}

async function settle() {
  for (let turn = 0; turn < 8; turn += 1) await act(async () => { await Promise.resolve(); });
}

function view(host: HTMLElement) {
  const control = host.querySelector<HTMLElement>("[data-research-tool]");
  const toggle = control?.querySelector<HTMLInputElement>("input[role=switch]");
  return {
    assistance: host.querySelector("[data-writing-assistant]")?.getAttribute("data-assistant-availability"),
    control: control === null || toggle == null ? null : {
      label: control.querySelector("label")?.textContent,
      sentence: control.querySelector("p")?.textContent,
      checked: toggle.checked,
      disabled: toggle.disabled,
    },
  };
}

it.each([
  ["not_offered", null],
  ["disabled", { label: RESEARCH_TOOL_LABEL, sentence: RESEARCH_TOOL_SERVICE_SENTENCE, checked: false, disabled: false }],
  ["enabled", { label: RESEARCH_TOOL_LABEL, sentence: RESEARCH_TOOL_SERVICE_SENTENCE, checked: true, disabled: false }],
  ["quarantined", { label: RESEARCH_TOOL_LABEL, sentence: RESEARCH_TOOL_UNAVAILABLE_SENTENCE, checked: false, disabled: true }],
] as const)("shows the web research control of a %s Project and keeps the assistance usable", async (state, control) => {
  await renderPanel(fakeServer(state).fetchImpl, async (host) => {
    expect(view(host)).toEqual({ assistance: "available", control });
    expect(host.textContent).not.toMatch(/web_research|mcp\.exa\.ai|STORYOS_EXA_API_KEY|0000000e0001/);
  });
});

it.each([
  ["applied", { kind: "authoritative_applied", enabled: true, revision: "1", project_activity_position: "2" },
    { checked: true, disabled: false, sentence: RESEARCH_TOOL_SERVICE_SENTENCE }],
  ["refused", { kind: "refused", reason: "registration_unavailable" },
    { checked: false, disabled: true, sentence: RESEARCH_TOOL_UNAVAILABLE_SENTENCE }],
] as const)("sends the enablement through its challenge and shows the %s state", async (_, effect, settled) => {
  const server = fakeServer("disabled", effect);
  await renderPanel(server.fetchImpl, async (host) => {
    await act(async () => { host.querySelector<HTMLInputElement>("[data-research-tool] input")!.click(); });
    await settle();

    const [challenge, command] = server.requests.filter((request) => request.method !== "GET");
    const request = command!.body as SetProjectToolEnablementRequest;
    expect({
      challenge: challenge!.body,
      command: { path: command!.path, method: command!.method,
        idempotencyKey: command!.headers.get("idempotency-key"),
        antiForgery: command!.headers.get("x-storyos-anti-forgery"),
        input: { ...request.set_project_tool_enablement_input, correlation_id: "" } },
      control: view(host).control,
    }).toEqual({
      challenge: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/tool-enablement",
        command_schema: "storyos.command.set-project-tool-enablement.request.v1",
        canonical_command_digest: await digestSetProjectToolEnablement(request, crypto),
        idempotency_key: command!.headers.get("idempotency-key"),
      },
      command: { path: `/api/v1/projects/${PROJECT}/tool-enablement`, method: "PUT",
        idempotencyKey: expect.stringMatching(/^[0-9a-f-]{36}$/), antiForgery: challengeFixture.nonce,
        input: { registration_id: "web_research", enabled: true, client_contract_revision: expect.any(String),
          security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: "" } },
      control: { label: RESEARCH_TOOL_LABEL, ...settled },
    });
  });
});
