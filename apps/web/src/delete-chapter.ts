import { deleteChapter, digestDeleteChapter } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DeleteChapterResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, sendOwnedCommand, waitAfterRateLimit } from "./owned-command.ts";

const inFlightDeletes = new Map<string, CommandFlight>();

function deleteChapterIdentity(options: {
  projectId: string;
  chapterId: string;
  expectedTreeRevision: string;
}): string {
  return `${options.projectId}\n${options.chapterId}\n${options.expectedTreeRevision}`;
}

export async function deleteOwnedChapter(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  chapterId: string;
  expectedTreeRevision: string;
}): Promise<DeleteChapterResponse> {
  return sendOwnedCommand({
    ...options,
    flights: inFlightDeletes,
    identity: deleteChapterIdentity(options),
    method: "DELETE",
    routeTemplate: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
    name: "Delete Chapter",
    request: (envelope) => ({
      command_schema: "storyos.command.delete-chapter.request.v1",
      delete_chapter_input: {
        expected_tree_revision: options.expectedTreeRevision,
        ...envelope,
      },
    }),
    digest: digestDeleteChapter,
    send: (command) => deleteChapter({ ...command, chapterId: options.chapterId }),
    // A Challenge rate limit gets a new Challenge after Retry-After.
    beforeRetry: waitAfterRateLimit,
  });
}
