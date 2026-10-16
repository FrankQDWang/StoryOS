import { getManuscriptTree } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { GetManuscriptTreeResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

const TREE_READ_ATTEMPTS = 3;

/**
 * Reads the manuscript tree for the outline and the reference labels. A failed read is tried again after a
 * short wait, at most two more times. Thus one lost response does not remove them until a reload.
 */
export async function readManuscriptTree(options: { baseUrl: string; projectId: string; fetchImpl: typeof fetch }):
  Promise<GetManuscriptTreeResponse> {
  for (let attempt = 1; ; attempt += 1) {
    try {
      return await getManuscriptTree(options);
    } catch (error: unknown) {
      if (attempt === TREE_READ_ATTEMPTS) throw error;
      await new Promise((resolve) => setTimeout(resolve, attempt * 500));
    }
  }
}
