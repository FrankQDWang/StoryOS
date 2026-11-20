import {
  createProjectCommandChallenge,
  StoryOSProtocolError,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DigestValue } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { SECURITY_POLICY_REVISION, uuidV7 } from "./client-identity.ts";
import { historicalAcknowledgementUnavailable } from "./historical-acknowledgement.ts";
import { challengeRefused, forgetFlight, forgettingRefusedFlight } from "./refused-command.ts";

/** The keys of one command intent. While the flight is in the map, the retry and each later call use them. */
export type CommandFlight = {
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

/** The fields that each command input of the Web client has. */
export type CommandEnvelope = {
  client_contract_revision: string;
  security_policy_revision: string;
  correlation_id: string;
};

/** The values that the generated client function of a command gets. */
export type CommandSend<Request> = {
  baseUrl: string;
  projectId: string;
  fetchImpl: typeof fetch;
  idempotencyKey: string;
  antiForgery: string;
  request: Request;
};

export type OwnedCommand<Request extends { command_schema: string }, Response> = {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  /** The flights of the module. Each intent has one flight until its command settles. */
  flights: Map<string, CommandFlight>;
  /** The key of the intent in `flights`. */
  identity: string;
  method: "POST" | "PATCH" | "PUT" | "DELETE";
  routeTemplate: string;
  /** The command name in the error when a Challenge gives no nonce. */
  name: string;
  request: (envelope: CommandEnvelope) => Request;
  digest: (request: Request, cryptoImpl: Crypto) => Promise<DigestValue>;
  send: (command: CommandSend<Request>) => Promise<Response>;
  /** Whether `error` ends the command without the one retry. The default is `endsWithoutRetry`. */
  endsCommand?: (error: unknown) => boolean;
  /** Runs before the one retry. */
  beforeRetry?: (error: unknown, flight: CommandFlight) => Promise<void>;
};

/**
 * No retry occurs after `historical_acknowledgement_unavailable` or after a refused Command Challenge. Each other
 * error gets one retry with the same idempotency key.
 */
export const endsWithoutRetry = (error: unknown): boolean =>
  historicalAcknowledgementUnavailable(error) || challengeRefused(error);

/** After a Challenge rate limit (status 429), waits for Retry-After and one more second, and forgets the nonce. */
export async function waitAfterRateLimit(error: unknown, flight: CommandFlight): Promise<void> {
  if (!(error instanceof StoryOSProtocolError) || error.status !== 429) return;
  delete flight.nonce;
  await new Promise<void>((resolve) => {
    setTimeout(resolve, ((error.retryAfterSeconds ?? 1) + 1) * 1000);
  });
}

/**
 * Sends the command of one intent: a Challenge when the flight has no nonce, then the command. After a failed send,
 * one retry uses the same flight, unless `endsCommand` says that the error ends the command.
 */
export async function sendOwnedCommand<Request extends { command_schema: string }, Response>(
  command: OwnedCommand<Request, Response>,
): Promise<Response> {
  const { flights, identity } = command;
  const flight = flights.get(identity) ?? {
    idempotencyKey: uuidV7(command.cryptoImpl),
    correlationId: uuidV7(command.cryptoImpl),
  };
  flights.set(identity, flight);
  const send = () => forgettingRefusedFlight(flights, identity, flight, () => submit(command, flight));
  try {
    const result = await send();
    forgetFlight(flights, identity, flight);
    return result;
  } catch (error) {
    if ((command.endsCommand ?? endsWithoutRetry)(error)) {
      throw error;
    }
    await command.beforeRetry?.(error, flight);
    const result = await send();
    forgetFlight(flights, identity, flight);
    return result;
  }
}

async function submit<Request extends { command_schema: string }, Response>(
  command: OwnedCommand<Request, Response>,
  flight: CommandFlight,
): Promise<Response> {
  const request = command.request({
    client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
    security_policy_revision: SECURITY_POLICY_REVISION,
    correlation_id: flight.correlationId,
  });
  if (flight.nonce === undefined) {
    const challenge = await createProjectCommandChallenge({
      baseUrl: command.baseUrl,
      projectId: command.projectId,
      fetchImpl: command.fetchImpl,
      request: {
        method: command.method,
        route_template: command.routeTemplate,
        command_schema: request.command_schema,
        canonical_command_digest: await command.digest(request, command.cryptoImpl),
        idempotency_key: flight.idempotencyKey,
      },
    });
    flight.nonce = challenge.nonce;
  }
  const nonce = flight.nonce;
  if (nonce === undefined) {
    throw new Error(`the ${command.name} challenge nonce is missing`);
  }
  return command.send({
    baseUrl: command.baseUrl,
    projectId: command.projectId,
    fetchImpl: command.fetchImpl,
    idempotencyKey: flight.idempotencyKey,
    antiForgery: nonce,
    request,
  });
}
