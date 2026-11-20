import { digestTakeOverProjectWriter, takeOverProjectWriter } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { TakeOverProjectWriterResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { challengeRefused } from "./refused-command.ts";
import { type CommandFlight, sendOwnedCommand } from "./owned-command.ts";

const EDITOR_CONTRACT_REVISION = "storyos.editor-contract.release-1.v3";

const inFlight = new Map<string, CommandFlight>();

function takeoverIdentity(options: {
  projectId: string;
  editorSessionId: string;
  observedWriterGeneration: string;
}): string {
  return `${options.projectId}\n${options.editorSessionId}\n${options.observedWriterGeneration}`;
}

export async function takeOverOwnedProjectWriter(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  editorSessionId: string;
  observedWriterGeneration: string;
}): Promise<TakeOverProjectWriterResponse> {
  return sendOwnedCommand({
    ...options,
    flights: inFlight,
    identity: takeoverIdentity(options),
    method: "POST",
    routeTemplate: "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers",
    name: "Takeover",
    request: (envelope) => ({
      command_schema: "storyos.command.take-over-project-writer.request.v1",
      ...envelope,
      editor_session_id: options.editorSessionId,
      observed_writer_generation: options.observedWriterGeneration,
      editor_contract_revision: EDITOR_CONTRACT_REVISION,
    }),
    digest: digestTakeOverProjectWriter,
    send: (command) => takeOverProjectWriter({ ...command, editorSessionId: options.editorSessionId }),
    // A Takeover has no historical acknowledgement rule: only a refused Challenge ends it without the one retry.
    endsCommand: challengeRefused,
  });
}
