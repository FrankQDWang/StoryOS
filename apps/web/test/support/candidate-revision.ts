import { createAgentRun, digestCreateAgentRun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AgentRunContextInspect, CreateAgentRunRequest, GetProposalResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, challenged } from "./acceptance.ts";

export async function admitCandidateRevision(
  baseUrl: string, fetchImpl: typeof fetch, projectId: string,
  proposal: GetProposalResponse["proposal"], operationId: string, key: string,
  text = "make the candidate calmer.",
  range?: [number, number],
) {
  const candidate = proposal.operations.find((operation) => operation.operation_id === operationId)?.candidate_text ?? " ";
  const request: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: { conversation: { kind: "new" }, author_message: { text },
      working_target: { kind: "selection_references", source_chapter_id: proposal.chapter_id, references: [{
        kind: "proposal_candidate_text", chapter_id: proposal.chapter_id, proposal_id: proposal.proposal_id,
        operation_id: operationId, revision_id: proposal.revision_id, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
        start_offset: range?.[0] ?? 0, end_offset: range?.[1] ?? candidate.length }] },
      instruction: { kind: "absent" }, cause: { kind: "author_request" }, ...BINDING, correlation_id: key },
  };
  let nonce = "";
  const repeat = async () => nonce ? createAgentRun({ baseUrl, projectId, fetchImpl, request,
    antiForgery: nonce, idempotencyKey: key }) : challenged(baseUrl, fetchImpl, projectId, "POST",
    "/api/v1/projects/{project_id}/agent-runs", request.command_schema,
    await digestCreateAgentRun(request), key, (antiForgery) => createAgentRun({
      baseUrl, projectId, fetchImpl, request, antiForgery: (nonce = antiForgery), idempotencyKey: key }));
  return { created: await repeat(), repeat, request };
}

/** The one candidate reference of a Run context. */
export function candidateReference(context: AgentRunContextInspect) {
  const reference = context.selection_references?.[0]?.reference;
  if (reference?.kind !== "proposal_candidate_text") throw new Error("expected a candidate reference");
  return reference;
}
