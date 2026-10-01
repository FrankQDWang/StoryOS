import { createAgentRun, digestCreateAgentRun } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateAgentRunRequest, GetProposalResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, challenged } from "./acceptance.ts";

export async function admitCandidateRevision(
  baseUrl: string, fetchImpl: typeof fetch, projectId: string,
  proposal: GetProposalResponse["proposal"], operationId: string, key: string,
  text = "make the candidate calmer.",
) {
  const request: CreateAgentRunRequest = {
    command_schema: "storyos.command.create-agent-run.request.v2",
    create_agent_run_input: { conversation: { kind: "new" }, author_message: { text },
      working_target: { kind: "proposal_candidate", source_chapter_id: proposal.chapter_id,
        target: { proposal_id: proposal.proposal_id, operation_id: operationId, revision_id: proposal.revision_id } },
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
