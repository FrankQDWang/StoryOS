import {
  getProjectAssistance,
  getStatistics,
  listSkills,
  type GetManuscriptTreeResponse,
  type GetProjectAssistanceResponse,
  type GetStatisticsResponse,
  type ListSkillsResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readManuscriptTree } from "./manuscript-tree-read.ts";

type EarlyReadValues = {
  tree: GetManuscriptTreeResponse;
  statistics: GetStatisticsResponse;
  assistance: GetProjectAssistanceResponse;
  skills: ListSkillsResponse;
};

/**
 * The reads of a Project that an open starts with its Chapter read, after the Project read, so that they do not wait
 * for the Editor Session (#1937). Each read needs only the Project. The view takes each read one time: a later read of
 * the view goes to the Server.
 */
export interface ProjectReads {
  take<Key extends keyof EarlyReadValues>(key: Key): Promise<EarlyReadValues[Key]> | undefined;
}

/**
 * Starts the tree read and the statistics read of the Project. `panels` also starts the reads of the assistant panel,
 * which reads them only at its first open.
 */
export function startProjectReads(options: { baseUrl: string; projectId: string; fetchImpl: typeof fetch },
  { panels = false }: { panels?: boolean } = {}): ProjectReads {
  const reads: { [Key in keyof EarlyReadValues]?: Promise<EarlyReadValues[Key]> } = {
    tree: readManuscriptTree(options),
    statistics: getStatistics(options),
    ...(panels ? { assistance: getProjectAssistance(options), skills: listSkills(options) } : {}),
  };
  // A read that the view does not take fails with no unhandled rejection.
  for (const read of Object.values(reads)) read.catch(() => {});
  return {
    take(key) {
      const read = reads[key];
      delete reads[key];
      return read;
    },
  };
}
