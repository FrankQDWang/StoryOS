import { createVolume, digestCreateVolume } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { CreateVolumeResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, sendOwnedCommand } from "./owned-command.ts";

const inFlightCreates = new Map<string, CommandFlight>();

function createVolumeIdentity(options: {
  projectId: string;
  title: string;
  expectedTreeRevision: string;
}): string {
  return `${options.projectId}\n${options.expectedTreeRevision}\n${options.title}`;
}

export async function createOwnedVolume(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  title: string;
  expectedTreeRevision: string;
}): Promise<CreateVolumeResponse> {
  return sendOwnedCommand({
    ...options,
    flights: inFlightCreates,
    identity: createVolumeIdentity(options),
    method: "POST",
    routeTemplate: "/api/v1/projects/{project_id}/volumes",
    name: "Create Volume",
    request: (envelope) => ({
      command_schema: "storyos.command.create-volume.request.v1",
      create_volume_input: {
        title: options.title,
        expected_tree_revision: options.expectedTreeRevision,
        ...envelope,
      },
    }),
    digest: digestCreateVolume,
    send: createVolume,
  });
}
