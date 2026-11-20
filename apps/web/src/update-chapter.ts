import { digestUpdateChapter, updateChapter } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { UpdateChapterResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, sendOwnedCommand } from "./owned-command.ts";

const inFlightUpdates = new Map<string, CommandFlight>();

function updateChapterIdentity(options: {
  projectId: string;
  chapterId: string;
  title: string;
  order: string;
  expectedTreeRevision: string;
}): string {
  return `${options.projectId}\n${options.chapterId}\n${options.expectedTreeRevision}\n${options.order}\n${options.title}`;
}

export async function updateOwnedChapter(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  chapterId: string;
  title: string;
  order: string;
  expectedTreeRevision: string;
}): Promise<UpdateChapterResponse> {
  return sendOwnedCommand({
    ...options,
    flights: inFlightUpdates,
    identity: updateChapterIdentity(options),
    method: "PATCH",
    routeTemplate: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
    name: "Update Chapter",
    request: (envelope) => ({
      command_schema: "storyos.command.update-chapter.request.v1",
      update_chapter_input: {
        title: options.title,
        order: options.order,
        expected_tree_revision: options.expectedTreeRevision,
        ...envelope,
      },
    }),
    digest: digestUpdateChapter,
    send: (command) => updateChapter({ ...command, chapterId: options.chapterId }),
  });
}
