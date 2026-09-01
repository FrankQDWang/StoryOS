import {
  createProjectCommandChallenge,
  digestExportHumanReadableManuscript,
  exportHumanReadableManuscript,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ExportHumanReadableManuscriptResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

type InFlightExport = {
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

const inFlightExports = new Map<string, InFlightExport>();

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

export async function requestOwnedHumanReadableExport(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
}): Promise<ExportHumanReadableManuscriptResponse> {
  let flight = inFlightExports.get(options.projectId);
  if (flight === undefined) {
    flight = {
      idempotencyKey: uuidV7(options.cryptoImpl),
      correlationId: uuidV7(options.cryptoImpl),
    };
    inFlightExports.set(options.projectId, flight);
  }
  try {
    const accepted = await submitExport(options, flight);
    inFlightExports.delete(options.projectId);
    return accepted;
  } catch {
    const accepted = await submitExport(options, flight);
    inFlightExports.delete(options.projectId);
    return accepted;
  }
}

async function submitExport(
  options: {
    baseUrl: string;
    fetchImpl: typeof fetch;
    cryptoImpl: Crypto;
    projectId: string;
  },
  flight: InFlightExport,
): Promise<ExportHumanReadableManuscriptResponse> {
  const request = {
    command_schema: "storyos.command.export-human-readable-manuscript.request.v1",
    export_human_readable_manuscript_input: {
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
        method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/exports",
        command_schema: request.command_schema,
        canonical_command_digest: await digestExportHumanReadableManuscript(
          request,
          options.cryptoImpl,
        ),
        idempotency_key: flight.idempotencyKey,
      },
    });
    flight.nonce = challenge.nonce;
  }
  const nonce = flight.nonce;
  if (nonce === undefined) {
    throw new Error("the human-readable export challenge nonce is missing");
  }
  return exportHumanReadableManuscript({
    baseUrl: options.baseUrl,
    projectId: options.projectId,
    fetchImpl: options.fetchImpl,
    idempotencyKey: flight.idempotencyKey,
    antiForgery: nonce,
    request,
  });
}
