import {
  createProjectCommandChallenge,
  digestSetCurrentChapter,
  setCurrentChapter,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { SetCurrentChapterResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

type InFlightSetCurrentChapter = {
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

const inFlight = new Map<string, InFlightSetCurrentChapter>();

function currentChapterIdentity(options: {
  projectId: string;
  chapterId: string;
  expectedCurrentChapterId: string;
  expectedTargetRevisionId: string;
}): string {
  return `${options.projectId}\n${options.chapterId}\n${options.expectedCurrentChapterId}\n${options.expectedTargetRevisionId}`;
}

function uuidV7(cryptoImpl: Crypto, now = Date.now()): string {
  const bytes = cryptoImpl.getRandomValues(new Uint8Array(16));
  for (let offset = 5; offset >= 0; offset -= 1) {
    bytes[offset] = now & 0xff;
    now = Math.floor(now / 256);
  }
  bytes[6] = (bytes[6]! & 0x0f) | 0x70;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
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
}): Promise<SetCurrentChapterResponse> {
  const identity = currentChapterIdentity(options);
  let flight = inFlight.get(identity);
  if (flight === undefined) {
    flight = {
      idempotencyKey: uuidV7(options.cryptoImpl),
      correlationId: uuidV7(options.cryptoImpl),
    };
    inFlight.set(identity, flight);
  }
  try {
    const switched = await submitSetCurrentChapter(options, flight);
    inFlight.delete(identity);
    return switched;
  } catch {
    const switched = await submitSetCurrentChapter(options, flight);
    inFlight.delete(identity);
    return switched;
  }
}

async function submitSetCurrentChapter(
  options: {
    baseUrl: string;
    fetchImpl: typeof fetch;
    cryptoImpl: Crypto;
    projectId: string;
    chapterId: string;
    expectedCurrentChapterId: string;
    expectedTargetRevisionId: string;
    editorSessionId: string;
  },
  flight: InFlightSetCurrentChapter,
): Promise<SetCurrentChapterResponse> {
  const request = {
    command_schema: "storyos.command.set-current-chapter.request.v1",
    set_current_chapter_input: {
      chapter_id: options.chapterId,
      expected_current_chapter_id: options.expectedCurrentChapterId,
      expected_target_revision_id: options.expectedTargetRevisionId,
      editor_session_id: options.editorSessionId,
      client_contract_revision:
        RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: SECURITY_POLICY_REVISION,
      correlation_id: flight.correlationId,
    },
  };
  if (flight.nonce === undefined) {
    const challenge = await createProjectCommandChallenge({
      baseUrl: options.baseUrl,
      projectId: options.projectId,
      fetchImpl: options.fetchImpl,
      request: {
        method: "PUT",
        route_template: "/api/v1/projects/{project_id}/current-chapter",
        command_schema: request.command_schema,
        canonical_command_digest: await digestSetCurrentChapter(request, options.cryptoImpl),
        idempotency_key: flight.idempotencyKey,
      },
    });
    flight.nonce = challenge.nonce;
  }
  const nonce = flight.nonce;
  if (nonce === undefined) {
    throw new Error("the Set Current Chapter challenge nonce is missing");
  }
  return setCurrentChapter({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    fetchImpl: options.fetchImpl,
    idempotencyKey: flight.idempotencyKey,
    antiForgery: nonce,
    request,
  });
}
