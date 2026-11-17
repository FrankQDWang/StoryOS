import {
  createProjectCommandChallenge,
  deleteMemoryEntry,
  digestDeleteMemoryEntry,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DeleteMemoryEntryRequest, ProjectScope }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { uuidV7 } from "./acceptance-journal.ts";
import { sameScope } from "./project-instruction.ts";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

/** The outcome of one delete, as the Memory section shows it. */
export type MemoryDeleteOutcome = "deleted" | "changed" | "failed";

/**
 * One delete intent of an entry at a version. A delete that has no known outcome keeps its flight. The next confirm
 * of the same entry at the same version then sends the same idempotency key. Thus the Host applies the delete one time.
 */
export type MemoryDeleteFlight = {
  identity: string;
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

type Connection = { baseUrl: string; fetchImpl: typeof fetch; scope: ProjectScope; cryptoImpl: Crypto };

/** Deletes the entry `memoryEntryId` at the version `version`. `flight` is the intent that has no known outcome. */
export async function deleteMemoryEntryOnce(connection: Connection & {
  memoryEntryId: string;
  version: string;
  flight: MemoryDeleteFlight | undefined;
}): Promise<{ outcome: MemoryDeleteOutcome; flight: MemoryDeleteFlight | undefined }> {
  const identity = `${connection.memoryEntryId}\n${connection.version}`;
  const flight = connection.flight?.identity === identity ? connection.flight : {
    identity,
    idempotencyKey: uuidV7(connection.cryptoImpl),
    correlationId: uuidV7(connection.cryptoImpl),
  };
  const request: DeleteMemoryEntryRequest = {
    command_schema: "storyos.command.delete-memory-entry.request.v1",
    delete_memory_entry_input: {
      expected_version: connection.version,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: SECURITY_POLICY_REVISION,
      correlation_id: flight.correlationId,
    },
  };
  const options = { baseUrl: connection.baseUrl, fetchImpl: connection.fetchImpl,
    projectId: connection.scope.project_id };
  try {
    flight.nonce ??= (await createProjectCommandChallenge({ ...options, request: {
      method: "DELETE",
      route_template: "/api/v1/projects/{project_id}/memory-entries/{memory_entry_id}",
      command_schema: request.command_schema,
      canonical_command_digest: await digestDeleteMemoryEntry(request, connection.cryptoImpl),
      idempotency_key: flight.idempotencyKey,
    } })).nonce;
    const deleted = await deleteMemoryEntry({ ...options, memoryEntryId: connection.memoryEntryId, request,
      idempotencyKey: flight.idempotencyKey, antiForgery: flight.nonce });
    if (!sameScope(deleted.project_scope, connection.scope)) return { outcome: "failed", flight };
    return { outcome: deleted.effect.kind === "conflicted" ? "changed" : "deleted", flight: undefined };
  } catch {
    return { outcome: "failed", flight };
  }
}
