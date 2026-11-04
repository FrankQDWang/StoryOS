import { useEffect, useRef, useState } from "react";

import {
  createProjectCommandChallenge, digestSetProjectToolEnablement, setProjectToolEnablement,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ProjectResearchTool, SetProjectToolEnablementRequest,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { uuidV7 } from "./acceptance-journal.ts";
import type { AssistantContext } from "./writing-assistant-panel.tsx";

/** The web research Registration of the deployment: the `id` of `EXA_TOOL_REGISTRATION` in `research_destination.rs`. */
const WEB_RESEARCH_REGISTRATION_ID = "web_research";
const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

export const RESEARCH_TOOL_LABEL = "允许本项目进行网络研究";
export const RESEARCH_TOOL_SERVICE_SENTENCE = "开启后，写作助手可以自行用 Exa 搜索网页并阅读网页内容。";
export const RESEARCH_TOOL_UNAVAILABLE_SENTENCE = "网络研究暂时不可用。";
export const RESEARCH_TOOL_UNCONFIRMED_SENTENCE = "网络研究设置未能确认。请刷新页面后查看。";

async function sendEnablement(context: AssistantContext, enabled: boolean): Promise<ProjectResearchTool> {
  const idempotencyKey = uuidV7(context.cryptoImpl);
  const request: SetProjectToolEnablementRequest = {
    command_schema: "storyos.command.set-project-tool-enablement.request.v1",
    set_project_tool_enablement_input: {
      registration_id: WEB_RESEARCH_REGISTRATION_ID,
      enabled,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: SECURITY_POLICY_REVISION,
      correlation_id: uuidV7(context.cryptoImpl),
    },
  };
  const challenge = await createProjectCommandChallenge({
    baseUrl: context.baseUrl, fetchImpl: context.fetchImpl, projectId: context.scope.project_id,
    request: {
      method: "PUT",
      route_template: "/api/v1/projects/{project_id}/tool-enablement",
      command_schema: request.command_schema,
      canonical_command_digest: await digestSetProjectToolEnablement(request, context.cryptoImpl),
      idempotency_key: idempotencyKey,
    },
  });
  const response = await setProjectToolEnablement({
    baseUrl: context.baseUrl, fetchImpl: context.fetchImpl, projectId: context.scope.project_id,
    idempotencyKey, antiForgery: challenge.nonce, request,
  });
  if (response.project_scope.owner_user_id !== context.scope.owner_user_id
    || response.project_scope.project_id !== context.scope.project_id) {
    throw new Error("Project Tool Enablement Scope mismatch");
  }
  const effect = response.effect;
  switch (effect.kind) {
    case "authoritative_applied":
      return effect.enabled ? "enabled" : "disabled";
    case "no_effect":
      return enabled ? "enabled" : "disabled";
    case "refused":
      switch (effect.reason) {
        case "registration_not_offered":
          return "not_offered";
        case "registration_unavailable":
          return "quarantined";
      }
  }
}

/** The one control of Project Tool Enablement. It names the service and no technical identifier (ADR 0040). */
export function ResearchToolControl({ context, state, onSettled }: {
  context: AssistantContext;
  state: ProjectResearchTool;
  onSettled: (state: ProjectResearchTool) => void;
}) {
  const [sending, setSending] = useState(false);
  const [unconfirmed, setUnconfirmed] = useState(false);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);
  if (state === "not_offered") return null;
  const quarantined = state === "quarantined";
  return <section className="assistant-research-tool" data-research-tool={state}>
    <label>
      <input type="checkbox" role="switch" checked={state === "enabled"} disabled={quarantined || sending}
        onChange={(event) => {
          const enabled = event.currentTarget.checked;
          setSending(true);
          setUnconfirmed(false);
          void sendEnablement(context, enabled).then((settled) => {
            if (alive.current) onSettled(settled);
          }, () => {
            if (alive.current) setUnconfirmed(true);
          }).finally(() => {
            if (alive.current) setSending(false);
          });
        }} />
      {RESEARCH_TOOL_LABEL}
    </label>
    <p>{quarantined ? RESEARCH_TOOL_UNAVAILABLE_SENTENCE : RESEARCH_TOOL_SERVICE_SENTENCE}</p>
    {unconfirmed ? <p role="status">{RESEARCH_TOOL_UNCONFIRMED_SENTENCE}</p> : null}
  </section>;
}
