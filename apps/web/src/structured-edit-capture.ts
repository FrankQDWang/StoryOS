import type { AuthorEditUnit }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export interface StructuredSelectionEdit {
  kind: "structured_selection";
  authorEditUnit: AuthorEditUnit;
  expectedProposalHeads: string[];
}
