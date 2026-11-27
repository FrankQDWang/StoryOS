import {
  digestSetCurrentChapter,
  setCurrentChapter,
  StoryOSProtocolError,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { SetCurrentChapterResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { type CommandFlight, endsWithoutRetry, sendOwnedCommand } from "./owned-command.ts";

const inFlight = new Map<string, CommandFlight>();
// The idempotency key of each command that went out and did not end: it has no response yet, or it has an error
// response and its one retry comes next. A flight can exist with no command that went out, for example after a
// Command Challenge rate limit.
const sent = new Set<string>();

/** The caller replaced a Set Current Chapter command after its Command Challenge and before the command went out. */
export class SkippedCurrentChapterError extends Error {}

/** The command and its repeat went out and got no answer of the Server, so the result of the command is not known. */
export class UnknownCurrentChapterResultError extends Error {}

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
  // The caller waits for the Retry-After of a Challenge rate limit and repeats with the same key (#1679).
  const endsCommand = (error: unknown) => error instanceof SkippedCurrentChapterError || endsWithoutRetry(error)
    || (error instanceof StoryOSProtocolError && error.status === 429);
  let retried = false;
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
        }, (error: unknown) => {
          // An error response after the retry, or an error response with no retry, ends the command.
          if (error instanceof StoryOSProtocolError && (retried || endsCommand(error))) {
            sent.delete(command.idempotencyKey);
          }
          // The repeat got no answer and can have applied the command. A later intent must not get its replay.
          if (!(error instanceof StoryOSProtocolError) && retried) {
            sent.delete(command.idempotencyKey);
            inFlight.delete(identity);
            throw new UnknownCurrentChapterResultError("the Set Current Chapter command got no answer");
          }
          throw error;
        });
      }
      // The command did not go out, so a later intent gets a new Command Challenge.
      inFlight.delete(identity);
      return Promise.reject(new SkippedCurrentChapterError("the Set Current Chapter command was replaced"));
    },
    endsCommand,
    beforeRetry: () => {
      retried = true;
      return Promise.resolve();
    },
  });
}
