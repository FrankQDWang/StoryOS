import {
  createProjectCommandChallenge,
  deleteChapter,
  digestDeleteChapter,
  StoryOSProtocolError,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DeleteChapterResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

type InFlightDeleteChapter = {
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

const inFlightDeletes = new Map<string, InFlightDeleteChapter>();

function deleteChapterIdentity(options: {
  projectId: string;
  chapterId: string;
  expectedChapterRevision: string;
}): string {
  return `${options.projectId}\n${options.chapterId}\n${options.expectedChapterRevision}`;
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

export async function deleteOwnedChapter(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  chapterId: string;
  expectedChapterRevision: string;
}): Promise<DeleteChapterResponse> {
  const identity = deleteChapterIdentity(options);
  let flight = inFlightDeletes.get(identity);
  if (flight === undefined) {
    flight = {
      idempotencyKey: uuidV7(options.cryptoImpl),
      correlationId: uuidV7(options.cryptoImpl),
    };
    inFlightDeletes.set(identity, flight);
  }
  try {
    const removed = await submitDeleteChapter(options, flight);
    inFlightDeletes.delete(identity);
    return removed;
  } catch (error) {
    if (error instanceof StoryOSProtocolError && error.status === 429) {
      delete flight.nonce;
      await new Promise<void>((resolve) => {
        setTimeout(resolve, ((error.retryAfterSeconds ?? 1) + 1) * 1000);
      });
    }
    const removed = await submitDeleteChapter(options, flight);
    inFlightDeletes.delete(identity);
    return removed;
  }
}

async function submitDeleteChapter(
  options: {
    baseUrl: string;
    fetchImpl: typeof fetch;
    cryptoImpl: Crypto;
    projectId: string;
    chapterId: string;
    expectedChapterRevision: string;
  },
  flight: InFlightDeleteChapter,
): Promise<DeleteChapterResponse> {
  const request = {
    command_schema: "storyos.command.delete-chapter.request.v1",
    delete_chapter_input: {
      expected_chapter_revision: options.expectedChapterRevision,
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
        method: "DELETE",
        route_template: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
        command_schema: request.command_schema,
        canonical_command_digest: await digestDeleteChapter(request, options.cryptoImpl),
        idempotency_key: flight.idempotencyKey,
      },
    });
    flight.nonce = challenge.nonce;
  }
  const nonce = flight.nonce;
  if (nonce === undefined) {
    throw new Error("the Delete Chapter challenge nonce is missing");
  }
  return deleteChapter({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    chapterId: options.chapterId,
    fetchImpl: options.fetchImpl,
    idempotencyKey: flight.idempotencyKey,
    antiForgery: nonce,
    request,
  });
}
