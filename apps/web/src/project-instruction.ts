import {
  StoryOSProtocolError,
  createProjectCommandChallenge,
  digestUpdateProjectInstruction,
  getProjectInstruction,
  updateProjectInstruction,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ProjectScope, UpdateProjectInstructionRequest }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

/** The text and the revision of the current Project Instruction. Revision "0" is a Project with no Revision. */
export type SavedInstruction = { text: string; revision: string };

/** The outcome of one save, as the settings page shows it. */
export type InstructionSaveOutcome =
  | { kind: "applied"; saved: SavedInstruction }
  | { kind: "unchanged" }
  | { kind: "conflicted" }
  | { kind: "over_limit" }
  | { kind: "unknown" };

/**
 * One save that has no known outcome. A next save of the same text on the same revision sends the same idempotency
 * key, so the Host applies the save one time.
 */
export type InstructionSaveFlight = {
  identity: string;
  idempotencyKey: string;
  correlationId: string;
  nonce?: string;
};

type Connection = { baseUrl: string; fetchImpl: typeof fetch; scope: ProjectScope };

const sameScope = (actual: ProjectScope, expected: ProjectScope) =>
  actual.owner_user_id === expected.owner_user_id && actual.project_id === expected.project_id;

/** Reads the current Project Instruction. A response of another Project Scope gives `undefined`. */
export async function readProjectInstruction(connection: Connection): Promise<SavedInstruction | undefined> {
  const read = await getProjectInstruction({ baseUrl: connection.baseUrl, fetchImpl: connection.fetchImpl,
    projectId: connection.scope.project_id });
  if (!sameScope(read.project_scope, connection.scope)) return undefined;
  return read.instruction === null
    ? { text: "", revision: "0" }
    : { text: read.instruction.text, revision: read.instruction.revision };
}

/** Saves `text` on the revision `expected`. `flight` is the save of the same text that has no known outcome. */
export async function saveProjectInstruction(connection: Connection & {
  cryptoImpl: Crypto;
  text: string;
  expected: string;
  flight: InstructionSaveFlight | undefined;
}): Promise<{ outcome: InstructionSaveOutcome; flight: InstructionSaveFlight | undefined }> {
  const identity = `${connection.expected}\n${connection.text}`;
  const flight = connection.flight?.identity === identity ? connection.flight : {
    identity,
    idempotencyKey: uuidV7(connection.cryptoImpl),
    correlationId: uuidV7(connection.cryptoImpl),
  };
  const request: UpdateProjectInstructionRequest = {
    command_schema: "storyos.command.update-project-instruction.request.v1",
    update_project_instruction_input: {
      text: connection.text,
      expected_instruction_revision: connection.expected,
      client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: SECURITY_POLICY_REVISION,
      correlation_id: flight.correlationId,
    },
  };
  const options = { baseUrl: connection.baseUrl, fetchImpl: connection.fetchImpl,
    projectId: connection.scope.project_id };
  try {
    flight.nonce ??= (await createProjectCommandChallenge({ ...options, request: {
      method: "PUT",
      route_template: "/api/v1/projects/{project_id}/instruction",
      command_schema: request.command_schema,
      canonical_command_digest: await digestUpdateProjectInstruction(request, connection.cryptoImpl),
      idempotency_key: flight.idempotencyKey,
    } })).nonce;
    const saved = await updateProjectInstruction({ ...options, request,
      idempotencyKey: flight.idempotencyKey, antiForgery: flight.nonce });
    if (!sameScope(saved.project_scope, connection.scope)) return { outcome: { kind: "unknown" }, flight };
    const { effect } = saved;
    const outcome: InstructionSaveOutcome = effect.kind === "authoritative_applied"
      ? { kind: "applied", saved: { text: connection.text, revision: effect.revision } }
      : effect.kind === "no_effect" ? { kind: "unchanged" } : { kind: "conflicted" };
    return { outcome, flight: undefined };
  } catch (error) {
    return overItemLimit(error)
      ? { outcome: { kind: "over_limit" }, flight: undefined }
      : { outcome: { kind: "unknown" }, flight };
  }
}

/** The refusal before Admission of a text above the Context item limit (ADR 0052). */
function overItemLimit(error: unknown): boolean {
  if (!(error instanceof StoryOSProtocolError) || error.status !== 422 || typeof error.responseBody !== "string") {
    return false;
  }
  try {
    return (JSON.parse(error.responseBody) as { code?: unknown }).code === "instruction_over_item_limit";
  } catch {
    return false;
  }
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
