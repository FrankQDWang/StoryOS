import { deleteVolume, digestDeleteVolume } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DeleteVolumeResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, sendOwnedCommand, waitAfterRateLimit } from "./owned-command.ts";

const inFlightDeletes = new Map<string, CommandFlight>();

function deleteVolumeIdentity(options: {
  projectId: string;
  volumeId: string;
  expectedTreeRevision: string;
}): string {
  return `${options.projectId}\n${options.volumeId}\n${options.expectedTreeRevision}`;
}

export async function deleteOwnedVolume(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  volumeId: string;
  expectedTreeRevision: string;
}): Promise<DeleteVolumeResponse> {
  return sendOwnedCommand({
    ...options,
    flights: inFlightDeletes,
    identity: deleteVolumeIdentity(options),
    method: "DELETE",
    routeTemplate: "/api/v1/projects/{project_id}/volumes/{volume_id}",
    name: "Delete Volume",
    request: (envelope) => ({
      command_schema: "storyos.command.delete-volume.request.v1",
      delete_volume_input: {
        expected_tree_revision: options.expectedTreeRevision,
        ...envelope,
      },
    }),
    digest: digestDeleteVolume,
    send: (command) => deleteVolume({ ...command, volumeId: options.volumeId }),
    // A Challenge rate limit gets a new Challenge after Retry-After.
    beforeRetry: waitAfterRateLimit,
  });
}
