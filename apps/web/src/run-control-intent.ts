import { createProjectCommandChallenge, digestPauseAgentRun, digestSteerAgentRun,
  pauseAgentRun, steerAgentRun } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { GetAgentRunResponse, PauseAgentRunRequest, ProjectScope, SteerAgentRunRequest }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { uuidV7 } from "./acceptance-journal.ts";
import type { AssistantContext } from "./writing-assistant-panel.tsx";

type Target = { scope: ProjectScope; runId: string; conversationId: string; idempotencyKey: string };
export type RunControlIntent = Target & (
  { kind: "pause"; request: PauseAgentRunRequest }
  | { kind: "guidance"; request: SteerAgentRunRequest }
);

function storageKey(scope: ProjectScope) {
  return `run_control:${scope.owner_user_id}:${scope.project_id}`;
}

export function readRunControl(scope: ProjectScope): RunControlIntent | undefined {
  try {
    const raw = globalThis.sessionStorage?.getItem(storageKey(scope));
    if (!raw) return undefined;
    const intent = JSON.parse(raw) as RunControlIntent;
    if (intent.scope?.owner_user_id !== scope.owner_user_id || intent.scope.project_id !== scope.project_id
      || typeof intent.runId !== "string" || typeof intent.conversationId !== "string"
      || typeof intent.idempotencyKey !== "string") return undefined;
    switch (intent.kind) {
      case "pause":
        return intent.request.command_schema === "storyos.command.pause-agent-run.request.v1"
          && typeof intent.request.pause_agent_run_input.correlation_id === "string" ? intent : undefined;
      case "guidance":
        return intent.request.command_schema === "storyos.command.steer-agent-run.request.v1"
          && intent.request.steer_agent_run_input.conversation_id === intent.conversationId
          && typeof intent.request.steer_agent_run_input.author_message.text === "string" ? intent : undefined;
    }
  } catch {
    return undefined;
  }
}

export function saveRunControl(intent: RunControlIntent) {
  globalThis.sessionStorage?.setItem(storageKey(intent.scope), JSON.stringify(intent));
}

export function clearRunControl(intent: RunControlIntent) {
  if (readRunControl(intent.scope)?.idempotencyKey === intent.idempotencyKey) {
    globalThis.sessionStorage?.removeItem(storageKey(intent.scope));
  }
}

export function freezeRunControl(context: AssistantContext, run: GetAgentRunResponse,
  action: { kind: "pause" } | { kind: "guidance"; text: string }): RunControlIntent {
  const target = { scope: context.scope, runId: run.run_id, conversationId: run.conversation_id,
    idempotencyKey: uuidV7(context.cryptoImpl) };
  const binding = {
    client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: uuidV7(context.cryptoImpl),
  };
  switch (action.kind) {
    case "pause":
      return { ...target, kind: "pause", request: { command_schema: "storyos.command.pause-agent-run.request.v1",
        pause_agent_run_input: binding } };
    case "guidance":
      return { ...target, kind: "guidance", request: { command_schema: "storyos.command.steer-agent-run.request.v1",
        steer_agent_run_input: { ...binding, conversation_id: run.conversation_id,
          author_message: { text: action.text } } } };
  }
}

export async function submitRunControl(context: AssistantContext, intent: RunControlIntent) {
  if (context.scope.owner_user_id !== intent.scope.owner_user_id
    || context.scope.project_id !== intent.scope.project_id) throw new Error("Run control Scope changed");
  const options = { baseUrl: context.baseUrl, fetchImpl: context.fetchImpl, projectId: intent.scope.project_id };
  const digest = intent.kind === "pause" ? await digestPauseAgentRun(intent.request, context.cryptoImpl)
    : await digestSteerAgentRun(intent.request, context.cryptoImpl);
  const route = intent.kind === "pause" ? "pause" : "steering-inputs";
  const challenge = await createProjectCommandChallenge({ ...options, request: { method: "POST",
    route_template: `/api/v1/projects/{project_id}/agent-runs/{run_id}/${route}`,
    command_schema: intent.request.command_schema, canonical_command_digest: digest,
    idempotency_key: intent.idempotencyKey } });
  const command = { ...options, runId: intent.runId, idempotencyKey: intent.idempotencyKey,
    antiForgery: challenge.nonce };
  const response = intent.kind === "pause" ? await pauseAgentRun({ ...command, request: intent.request })
    : await steerAgentRun({ ...command, request: intent.request });
  if (response.project_scope.owner_user_id !== intent.scope.owner_user_id
    || response.project_scope.project_id !== intent.scope.project_id
    || response.receipt.command_kind !== (intent.kind === "pause" ? "pauseAgentRun" : "steerAgentRun")
    || ("run_id" in response.effect && response.effect.run_id !== intent.runId)) {
    throw new Error("Run control response Scope mismatch");
  }
  return response;
}
