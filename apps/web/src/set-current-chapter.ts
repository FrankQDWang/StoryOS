import {
  digestSetCurrentChapter,
  setCurrentChapter,
  StoryOSProtocolError,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { SetCurrentChapterResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, endsWithoutRetry, sendOwnedCommand } from "./owned-command.ts";

const inFlight = new Map<string, CommandFlight>();
// The idempotency key of each command that went out and has no response yet. A flight can exist with no command that
// went out, for example after a Command Challenge rate limit.
const sent = new Set<string>();

/** The caller replaced a Set Current Chapter command after its Command Challenge and before the command went out. */
export class SkippedCurrentChapterError extends Error {}

function currentChapterIdentity(options: {
  projectId: string;
  chapterId: string;
  expectedCurrentChapterId: string;
  expectedTargetRevisionId: string;
}): string {
  return `${options.projectId}\n${options.chapterId}\n${options.expectedCurrentChapterId}\n${options.expectedTargetRevisionId}`;
}

export async function setOwnedCurrentChapter(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  chapterId: string;
  expectedCurrentChapterId: string;
  expectedTargetRevisionId: string;
  editorSessionId: string;
  /**
   * False when the caller replaced the command. It is read after the Command Challenge, until the command first goes
   * out. A command that went out goes out again with its key.
   */
  proceed?: () => boolean;
}): Promise<SetCurrentChapterResponse> {
  const identity = currentChapterIdentity(options);
  return sendOwnedCommand({
    ...options,
    flights: inFlight,
    identity,
    method: "PUT",
    routeTemplate: "/api/v1/projects/{project_id}/current-chapter",
    name: "Set Current Chapter",
    request: (envelope) => ({
      command_schema: "storyos.command.set-current-chapter.request.v1",
      set_current_chapter_input: {
        chapter_id: options.chapterId,
        expected_current_chapter_id: options.expectedCurrentChapterId,
        expected_target_revision_id: options.expectedTargetRevisionId,
        editor_session_id: options.editorSessionId,
        ...envelope,
      },
    }),
    digest: digestSetCurrentChapter,
    send: (command) => {
      if (sent.has(command.idempotencyKey) || options.proceed?.() !== false) {
        sent.add(command.idempotencyKey);
        return setCurrentChapter(command).then((response) => {
          sent.delete(command.idempotencyKey);
          return response;
        });
      }
      // The command did not go out, so a later intent gets a new Command Challenge.
      inFlight.delete(identity);
      return Promise.reject(new SkippedCurrentChapterError("the Set Current Chapter command was replaced"));
    },
    // The caller waits for the Retry-After of a Challenge rate limit and repeats with the same key (#1679).
    endsCommand: (error) => error instanceof SkippedCurrentChapterError || endsWithoutRetry(error)
      || (error instanceof StoryOSProtocolError && error.status === 429),
  });
}
