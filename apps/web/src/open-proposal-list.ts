import { listChapterOpenProposals } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ProjectScope } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export type ProposalLocator = {
  proposalId: string;
  runId: string;
  decisionId: string;
};

// The Server list is the source of the open Proposals of a Chapter. A failed or truncated list leaves Proposal heads unknown.
export type OpenProposalList = { kind: "reading" } | { kind: "failed" }
  | {
    kind: "listed"; complete: boolean; locators: ProposalLocator[];
    /** The Proposal and its Revision of each entry, as `{proposal_id}@{revision_id}`. */
    heads: string[];
  };

export async function readOpenProposalList(scope: ProjectScope, chapterId: string, baseUrl: string,
  fetchImpl: typeof fetch): Promise<OpenProposalList> {
  try {
    const response = await listChapterOpenProposals({ baseUrl, fetchImpl, projectId: scope.project_id, chapterId });
    if (response.project_scope.owner_user_id !== scope.owner_user_id
      || response.project_scope.project_id !== scope.project_id || response.chapter_id !== chapterId) {
      return { kind: "failed" };
    }
    return { kind: "listed", complete: response.completeness === "complete",
      locators: response.proposals.map(({ proposal_id, source }) => source.kind === "agent_run_decision"
        ? { proposalId: proposal_id, runId: source.run_id, decisionId: source.decision_id }
        : { proposalId: proposal_id, runId: "", decisionId: "" }),
      heads: response.proposals.map(({ proposal_id, revision_id }) => `${proposal_id}@${revision_id}`) };
  } catch {
    return { kind: "failed" };
  }
}
