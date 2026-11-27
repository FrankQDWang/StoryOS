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

/** The rows of a Project Conversation, the oldest first, and its facts. */
export type ConversationRows = { rows: ProjectConversationRun[]; opened: OpenedConversation };

type ReadOptions = { scope: ProjectScope; baseUrl: string; fetchImpl: typeof fetch; conversationId: string };

const sameScope = (left: ProjectScope, right: ProjectScope) =>
  left.owner_user_id === right.owner_user_id && left.project_id === right.project_id;

/**
 * Reads one Project Conversation. A response of another Project Scope, of another Conversation, or with no Run gives
 * an error, and the panel shows nothing of it.
 */
export async function readConversationRows(options: ReadOptions): Promise<ConversationRows> {
  const { scope, baseUrl, fetchImpl, conversationId } = options;
  const conversation = await getProjectConversation({
    baseUrl, fetchImpl, projectId: scope.project_id, conversationId,
  });
  if (!sameScope(conversation.project_scope, scope) || conversation.conversation_id !== conversationId
    || conversation.runs.length === 0) throw new Error("Project Conversation Scope mismatch");
  return {
    rows: conversation.runs,
    opened: {
      conversationId,
      firstRunId: conversation.runs[0]!.run_id,
      truncated: conversation.completeness === "truncated",
    },
  };
}

/**
 * Starts the read of each Run of `rows` at the same time. A response of another Project Scope, Run, or Conversation
 * gives an error.
 */
export function readConversationRuns(options: ReadOptions & { rows: readonly ProjectConversationRun[] }):
  Promise<GetAgentRunResponse>[] {
  const { scope, baseUrl, fetchImpl, conversationId, rows } = options;
  return rows.map(async (row) => {
    const run = await getAgentRun({ baseUrl, fetchImpl, projectId: scope.project_id, runId: row.run_id });
    if (!sameScope(run.project_scope, scope) || run.run_id !== row.run_id
      || run.conversation_id !== conversationId) throw new Error("Run Scope mismatch");
    return run;
  });
}
