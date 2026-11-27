import {
  getProjectAssistance,
  getStatistics,
  listSkills,
  type GetManuscriptTreeResponse,
  type GetProjectAssistanceResponse,
  type GetStatisticsResponse,
  type ListSkillsResponse,
  type ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readOpenProposalList, type OpenProposalList } from "./open-proposal-list.ts";
import { readManuscriptTree } from "./manuscript-tree-read.ts";

type EarlyReadValues = {
  tree: GetManuscriptTreeResponse;
  statistics: GetStatisticsResponse;
  assistance: GetProjectAssistanceResponse;
  skills: ListSkillsResponse;
};

/**
 * The reads of a Project that an open starts with its Chapter read, so that they do not wait for the Editor Session
 * (#1937). Each read needs only the Project. The view takes each read one time: a later read of the view goes to the
 * Server.
 */
export interface ProjectReads {
  take<Key extends keyof EarlyReadValues>(key: Key): Promise<EarlyReadValues[Key]> | undefined;
  /** The read of the open suggestions of `chapterId`, when the open started one for that Chapter. */
  takeSuggestions(chapterId: string): Promise<OpenProposalList> | undefined;
}

/**
 * Starts the tree read and the statistics read of the Project. `panels` also starts the reads of the assistant panel,
 * which reads them only at its first open. `suggestions` also starts the read of the open suggestions of a Chapter.
 * Only an open that has a base Snapshot earlier than the start of this call can use that read: the Project Activity
 * from that Snapshot gives each later Proposal (ADR 0105).
 */
export function startProjectReads(options: { baseUrl: string; projectId: string; fetchImpl: typeof fetch },
  { panels = false, suggestions }: { panels?: boolean; suggestions?: { scope: ProjectScope; chapterId: string } } = {})
  : ProjectReads {
  const reads: { [Key in keyof EarlyReadValues]?: Promise<EarlyReadValues[Key]> } = {
    tree: readManuscriptTree(options),
    statistics: getStatistics(options),
    ...(panels ? { assistance: getProjectAssistance(options), skills: listSkills(options) } : {}),
  };
  // A read that the view does not take fails with no unhandled rejection.
  for (const read of Object.values(reads)) read.catch(() => {});
  let list = suggestions === undefined ? undefined : { chapterId: suggestions.chapterId,
    read: readOpenProposalList(suggestions.scope, suggestions.chapterId, options.baseUrl, options.fetchImpl) };
  return {
    take(key) {
      const read = reads[key];
      delete reads[key];
      return read;
    },
    takeSuggestions(chapterId) {
      const taken = list?.chapterId === chapterId ? list.read : undefined;
      if (taken !== undefined) list = undefined;
      return taken;
    },
  };
}
