import { digestUpdateVolume, updateVolume } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { UpdateVolumeResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, sendOwnedCommand } from "./owned-command.ts";

const inFlightUpdates = new Map<string, CommandFlight>();

function updateVolumeIdentity(options: {
  projectId: string;
  volumeId: string;
  title: string;
  order: string;
  expectedTreeRevision: string;
}): string {
  return `${options.projectId}\n${options.volumeId}\n${options.expectedTreeRevision}\n${options.order}\n${options.title}`;
}

export async function updateOwnedVolume(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  volumeId: string;
  title: string;
  order: string;
  expectedTreeRevision: string;
}): Promise<UpdateVolumeResponse> {
  return sendOwnedCommand({
    ...options,
    flights: inFlightUpdates,
    identity: updateVolumeIdentity(options),
    method: "PATCH",
    routeTemplate: "/api/v1/projects/{project_id}/volumes/{volume_id}",
    name: "Update Volume",
    request: (envelope) => ({
      command_schema: "storyos.command.update-volume.request.v1",
      update_volume_input: {
        title: options.title,
        order: options.order,
        expected_tree_revision: options.expectedTreeRevision,
        ...envelope,
      },
    }),
    digest: digestUpdateVolume,
    send: (command) => updateVolume({ ...command, volumeId: options.volumeId }),
  });
}
