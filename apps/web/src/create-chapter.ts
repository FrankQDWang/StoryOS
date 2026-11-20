import { createChapter, digestCreateChapter } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateChapterResponse, CreateChapterPlacement } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, sendOwnedCommand } from "./owned-command.ts";

const inFlightCreates = new Map<string, CommandFlight>();

function createChapterIdentity(options: {
  projectId: string;
  volumeId: string;
  title: string;
  expectedTreeRevision: string;
  placement?: CreateChapterPlacement | undefined;
}): string {
  return `${options.projectId}\n${options.volumeId}\n${options.expectedTreeRevision}\n${options.title}\n${JSON.stringify(options.placement)}`;
}

export async function createOwnedChapter(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  volumeId: string;
  title: string;
  expectedTreeRevision: string;
  placement?: CreateChapterPlacement | undefined;
}): Promise<CreateChapterResponse> {
  return sendOwnedCommand({
    ...options,
    flights: inFlightCreates,
    identity: createChapterIdentity(options),
    method: "POST",
    routeTemplate: "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
    name: "Create Chapter",
    request: (envelope) => ({
      command_schema: "storyos.command.create-chapter.request.v1",
      create_chapter_input: {
        title: options.title,
        ...(options.placement === undefined ? {} : { placement: options.placement }),
        expected_tree_revision: options.expectedTreeRevision,
        ...envelope,
      },
    }),
    digest: digestCreateChapter,
    send: (command) => createChapter({ ...command, volumeId: options.volumeId }),
  });
}
