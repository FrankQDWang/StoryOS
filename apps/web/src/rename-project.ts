import { digestUpdateProject, updateProject } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { UpdateProjectResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, sendOwnedCommand } from "./owned-command.ts";

export {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE,
  historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";

const inFlightRenames = new Map<string, CommandFlight>();

function renameIdentity(options: {
  projectId: string;
  title: string;
  expectedProjectRevision: string;
}): string {
  return `${options.projectId}\n${options.expectedProjectRevision}\n${options.title}`;
}

export async function renameOwnedProject(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  title: string;
  expectedProjectRevision: string;
}): Promise<UpdateProjectResponse> {
  return sendOwnedCommand({
    ...options,
    flights: inFlightRenames,
    identity: renameIdentity(options),
    method: "PATCH",
    routeTemplate: "/api/v1/projects/{project_id}",
    name: "Update Project",
    request: (envelope) => ({
      command_schema: "storyos.command.update-project.request.v1",
      update_project_input: {
        title: options.title,
        expected_project_revision: options.expectedProjectRevision,
        ...envelope,
      },
    }),
    digest: digestUpdateProject,
    send: updateProject,
  });
}
