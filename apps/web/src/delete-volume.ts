import {
  createProjectCommandChallenge,
  deleteVolume,
  digestDeleteVolume,
  StoryOSProtocolError,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DeleteVolumeResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

type InFlightDeleteVolume = {
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

const inFlightDeletes = new Map<string, InFlightDeleteVolume>();

function deleteVolumeIdentity(options: {
  projectId: string;
  volumeId: string;
  expectedTreeRevision: string;
}): string {
  return `${options.projectId}\n${options.volumeId}\n${options.expectedTreeRevision}`;
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

export async function deleteOwnedVolume(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  volumeId: string;
  expectedTreeRevision: string;
}): Promise<DeleteVolumeResponse> {
  const identity = deleteVolumeIdentity(options);
  let flight = inFlightDeletes.get(identity);
  if (flight === undefined) {
    flight = {
      idempotencyKey: uuidV7(options.cryptoImpl),
      correlationId: uuidV7(options.cryptoImpl),
    };
    inFlightDeletes.set(identity, flight);
  }
  try {
    const removed = await submitDeleteVolume(options, flight);
    inFlightDeletes.delete(identity);
    return removed;
  } catch (error) {
    if (error instanceof StoryOSProtocolError && error.status === 429) {
      delete flight.nonce;
      await new Promise<void>((resolve) => {
        setTimeout(resolve, ((error.retryAfterSeconds ?? 1) + 1) * 1000);
      });
    }
    const removed = await submitDeleteVolume(options, flight);
    inFlightDeletes.delete(identity);
    return removed;
  }
}

async function submitDeleteVolume(
  options: {
    baseUrl: string;
    fetchImpl: typeof fetch;
    cryptoImpl: Crypto;
    projectId: string;
    volumeId: string;
    expectedTreeRevision: string;
  },
  flight: InFlightDeleteVolume,
): Promise<DeleteVolumeResponse> {
  const request = {
    command_schema: "storyos.command.delete-volume.request.v1",
    delete_volume_input: {
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
        method: "DELETE",
        route_template: "/api/v1/projects/{project_id}/volumes/{volume_id}",
        command_schema: request.command_schema,
        canonical_command_digest: await digestDeleteVolume(request, options.cryptoImpl),
        idempotency_key: flight.idempotencyKey,
      },
    });
    flight.nonce = challenge.nonce;
  }
  const nonce = flight.nonce;
  if (nonce === undefined) {
    throw new Error("the Delete Volume challenge nonce is missing");
  }
  return deleteVolume({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    volumeId: options.volumeId,
    fetchImpl: options.fetchImpl,
    idempotencyKey: flight.idempotencyKey,
    antiForgery: nonce,
    request,
  });
}
