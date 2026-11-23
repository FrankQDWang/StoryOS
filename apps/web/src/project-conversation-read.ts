import { getAgentRun, getProjectConversation } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  GetAgentRunResponse, ProjectConversationRun, ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

/** The facts of a Conversation that the panel opened from the Host (ADR 0094 section 10). */
export type OpenedConversation = {
  conversationId: string;
  /** The oldest Run that the read gave. The line of a truncated read goes above it. */
  firstRunId: string;
  truncated: boolean;
};

/** One Run of an opened Conversation: the row of the Conversation read and the Run read. */
export type OpenedRun = { row: ProjectConversationRun; run: GetAgentRunResponse };

const sameScope = (left: ProjectScope, right: ProjectScope) =>
  left.owner_user_id === right.owner_user_id && left.project_id === right.project_id;

/**
 * Reads one Project Conversation, then each of its Runs, the oldest first. A response of another Project Scope, of
 * another Conversation, or with no Run gives an error, and the panel shows nothing of it.
 */
export async function readOpenedConversation(options: {
  scope: ProjectScope; baseUrl: string; fetchImpl: typeof fetch; conversationId: string;
}): Promise<{ runs: OpenedRun[]; opened: OpenedConversation }> {
  const { scope, baseUrl, fetchImpl, conversationId } = options;
  const conversation = await getProjectConversation({
    baseUrl, fetchImpl, projectId: scope.project_id, conversationId,
  });
  if (!sameScope(conversation.project_scope, scope) || conversation.conversation_id !== conversationId
    || conversation.runs.length === 0) throw new Error("Project Conversation Scope mismatch");
  const runs: OpenedRun[] = [];
  for (const row of conversation.runs) {
    const run = await getAgentRun({ baseUrl, fetchImpl, projectId: scope.project_id, runId: row.run_id });
    if (!sameScope(run.project_scope, scope) || run.run_id !== row.run_id
      || run.conversation_id !== conversationId) throw new Error("Run Scope mismatch");
    runs.push({ row, run });
  }
  return {
    runs,
    opened: {
      conversationId,
      firstRunId: runs[0]!.row.run_id,
      truncated: conversation.completeness === "truncated",
    },
  };
}
