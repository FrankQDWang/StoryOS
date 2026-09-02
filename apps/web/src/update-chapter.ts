import {
  createProjectCommandChallenge,
  digestUpdateChapter,
  updateChapter,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { UpdateChapterResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

type InFlightUpdateChapter = {
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

const inFlightUpdates = new Map<string, InFlightUpdateChapter>();

function updateChapterIdentity(options: {
  projectId: string;
  chapterId: string;
  title: string;
  order: string;
  expectedTreeRevision: string;
}): string {
  return `${options.projectId}\n${options.chapterId}\n${options.expectedTreeRevision}\n${options.order}\n${options.title}`;
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

export async function updateOwnedChapter(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  chapterId: string;
  title: string;
  order: string;
  expectedTreeRevision: string;
}): Promise<UpdateChapterResponse> {
  const identity = updateChapterIdentity(options);
  let flight = inFlightUpdates.get(identity);
  if (flight === undefined) {
    flight = {
      idempotencyKey: uuidV7(options.cryptoImpl),
      correlationId: uuidV7(options.cryptoImpl),
    };
    inFlightUpdates.set(identity, flight);
  }
  try {
    const updated = await submitUpdateChapter(options, flight);
    inFlightUpdates.delete(identity);
    return updated;
  } catch {
    const updated = await submitUpdateChapter(options, flight);
    inFlightUpdates.delete(identity);
    return updated;
  }
}

async function submitUpdateChapter(
  options: {
    baseUrl: string;
    fetchImpl: typeof fetch;
    cryptoImpl: Crypto;
    projectId: string;
    chapterId: string;
    title: string;
    order: string;
    expectedTreeRevision: string;
  },
  flight: InFlightUpdateChapter,
): Promise<UpdateChapterResponse> {
  const request = {
    command_schema: "storyos.command.update-chapter.request.v1",
    update_chapter_input: {
      title: options.title,
      order: options.order,
      expected_tree_revision: options.expectedTreeRevision,
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
        method: "PATCH",
        route_template: "/api/v1/projects/{project_id}/chapters/{chapter_id}",
        command_schema: request.command_schema,
        canonical_command_digest: await digestUpdateChapter(request, options.cryptoImpl),
        idempotency_key: flight.idempotencyKey,
      },
    });
    flight.nonce = challenge.nonce;
  }
  const nonce = flight.nonce;
  if (nonce === undefined) {
    throw new Error("the Update Chapter challenge nonce is missing");
  }
  return updateChapter({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    chapterId: options.chapterId,
    fetchImpl: options.fetchImpl,
    idempotencyKey: flight.idempotencyKey,
    antiForgery: nonce,
    request,
  });
}
