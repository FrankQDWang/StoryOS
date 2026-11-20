import { digestExportHumanReadableManuscript, exportHumanReadableManuscript } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ExportHumanReadableManuscriptResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, sendOwnedCommand } from "./owned-command.ts";

const inFlightExports = new Map<string, CommandFlight>();

export async function requestOwnedHumanReadableExport(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
}): Promise<ExportHumanReadableManuscriptResponse> {
  return sendOwnedCommand({
    ...options,
    flights: inFlightExports,
    identity: options.projectId,
    method: "POST",
    routeTemplate: "/api/v1/projects/{project_id}/manuscript/exports",
    name: "human-readable export",
    request: (envelope) => ({
      command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
      export_human_readable_manuscript_input: envelope,
    }),
    digest: digestExportHumanReadableManuscript,
    send: exportHumanReadableManuscript,
  });
}
