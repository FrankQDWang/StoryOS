import type {
  ApplyAuthorEditResponse,
  CreateEditorSessionResponse,
  DigestValue,
  GetChapterResponse,
  GetProjectResponse,
  Release1ProtocolProfile,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import type {
  EditorReadyState,
  EditorState,
  ProjectActivityEvent,
} from "../../src/editor-types.ts";

export const OWNER = "018f0000-0000-7001-8000-000000000001";
export const PROJECT = "018f0000-0000-7001-8000-000000000002";
export const CHAPTER = "018f0000-0000-7001-8000-000000000003";
export const REVISION = "018f0000-0000-7001-8000-000000000004";
export const SESSION = "018f0000-0000-7001-8000-000000000021";
export const EDITOR_SNAPSHOT = "018f0000-0000-7001-8000-000000000022";

export interface BrowserScenario {
  readonly chapter: GetChapterResponse;
  readonly journalName: string;
  readonly profile: Release1ProtocolProfile;
  readonly project: GetProjectResponse;
  readonly session: CreateEditorSessionResponse;
}

export function createBrowserScenario(): BrowserScenario {
  const project: GetProjectResponse = {
    schema_id: "storyos.query.project.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000010",
    project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    project: { project_id: PROJECT, title: "Project A", open: { kind: "current_chapter", current_chapter_id: CHAPTER } },
  };
  const chapter: GetChapterResponse = {
    schema_id: "storyos.query.chapter.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000011",
    project_scope: project.project_scope,
    project_activity_position: "0",
    chapter: {
      chapter_id: CHAPTER,
      title: "Chapter A",
      current_revision: { revision_id: REVISION, body: "Base" },
    },
  };
  const session: CreateEditorSessionResponse = {
    schema_id: "storyos.command.create-editor-session.response.v1",
    correlation_id: "018f0000-0000-7001-8000-000000000020",
    project_scope: project.project_scope,
    editor_session: {
      editor_session_id: SESSION,
      client_session_binding_ref: "binding:test",
      client_session_generation: "1",
      client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1",
      opened_at: "2026-08-13T08:00:00.000Z",
      disposition: "open",
    },
    writer: { kind: "current_writer", writer_generation: "1" },
    base_snapshot: {
      snapshot_id: EDITOR_SNAPSHOT,
      chapter_id: CHAPTER,
      project_activity_position: "0",
      authoritative_head_revision_id: REVISION,
      proposal_head_revision_ids: [],
      target_refs: [`manuscript:${CHAPTER}`],
      observed_ownership_partition: "authoritative",
      materialized_revision: { revision_id: REVISION, body: "Base" },
      materialized_payload_digest: {
        algorithm: "sha256",
        profile: "storyos.canonical-payload.sha256.v1",
        value_hex_lowercase: "7b47361aad19bb483aeab081a7df0a55f7cb0fdb3327efe6dd016e6353a17880",
      },
      created_at: "2026-08-13T08:00:00.000Z",
    },
  };
  return {
    chapter,
    journalName: `storyos-local-edit-journal:${OWNER}:${PROJECT}`,
    profile: RELEASE_1_PROTOCOL_PROFILE,
    project,
    session,
  };
}

export function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

export function requestResult<Result>(request: IDBRequest<Result>): Promise<Result> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

export function trackDatabase(
  database: IDBDatabase,
  openDatabases: Set<IDBDatabase>,
): void {
  openDatabases.add(database);
}

export function closeTrackedDatabases(openDatabases: Set<IDBDatabase>): void {
  for (const database of openDatabases) database.close();
  openDatabases.clear();
}

export function deleteJournal(name: string): Promise<void> {
  const request = indexedDB.deleteDatabase(name);
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error ?? new Error("IndexedDB deletion failed"));
    // A close-pending connection can finish an active transaction before this request succeeds.
    // The blocked event is an intermediate state, not a failed deletion.
  });
}

export function requireEditorReady(state: EditorState): asserts state is EditorReadyState {
  if (state.kind !== "editor-ready") {
    throw new Error(`workspace unavailable: ${state.code}`);
  }
}

export function requestHeaders(init: RequestInit | undefined): Headers {
  return new Headers(init?.headers);
}

export function requireRequestBody(init: RequestInit | undefined): Record<string, unknown> {
  if (typeof init?.body !== "string") {
    throw new Error("the request body is unavailable");
  }
  const value: unknown = JSON.parse(init.body);
  return requireRecord(value, "the request body");
}

export function requireRecord(value: unknown, name: string): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${name} is not an object`);
  }
  return Object.fromEntries(Object.entries(value));
}

export function requireString(value: unknown, name: string): string {
  if (typeof value !== "string") throw new Error(`${name} is not a string`);
  return value;
}

export function requireDigestValue(value: unknown, name: string): DigestValue {
  if (value === null || typeof value !== "object") {
    throw new Error(`${name} is not a digest`);
  }
  const algorithm = Reflect.get(value, "algorithm");
  const profile = Reflect.get(value, "profile");
  const valueHexLowercase = Reflect.get(value, "value_hex_lowercase");
  if (algorithm !== "sha256" || typeof profile !== "string"
    || typeof valueHexLowercase !== "string") {
    throw new Error(`${name} is not a SHA-256 digest`);
  }
  return { algorithm, profile, value_hex_lowercase: valueHexLowercase };
}

export interface AppliedAuthorEditResponseOptions {
  readonly authorCommandAdmissionId?: string;
  readonly authoritativeCommitId?: string;
  readonly authoritativeRevisionId?: string;
  readonly body?: string;
  readonly commandDigest: DigestValue;
  readonly commandId?: string;
  readonly idempotencyKey: string;
  readonly projectActivityPosition?: string;
  readonly priorRevisionId?: string;
  readonly receiptId?: string;
  readonly request: Record<string, unknown>;
}

export function createAppliedAuthorEditResponse(
  options: AppliedAuthorEditResponseOptions,
): ApplyAuthorEditResponse {
  const commandId = options.commandId ?? "018f0000-0000-7001-8000-000000000031";
  const admissionId = options.authorCommandAdmissionId
    ?? "018f0000-0000-7001-8000-000000000032";
  const receiptId = options.receiptId ?? "018f0000-0000-7001-8000-000000000033";
  const revisionId = options.authoritativeRevisionId
    ?? "018f0000-0000-7001-8000-000000000034";
  const commitId = options.authoritativeCommitId
    ?? "018f0000-0000-7001-8000-000000000035";
  const position = options.projectActivityPosition ?? "1";
  const priorRevisionId = options.priorRevisionId ?? REVISION;
  return {
    schema_id: "storyos.command.apply-author-edit.response.v2",
    correlation_id: requireString(options.request.correlation_id, "correlation_id"),
    project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    command_id: commandId,
    author_command_admission_id: admissionId,
    receipt: {
      receipt_id: receiptId,
      project_scope: { owner_user_id: OWNER, project_id: PROJECT },
      command_kind: "applyAuthorEdit",
      command_digest: options.commandDigest,
      idempotency_key: options.idempotencyKey,
      producer_cause: "author_command_admission",
      author_command_admission_id: admissionId,
      expected_heads: [priorRevisionId],
      prior_heads: [priorRevisionId],
      resulting_heads: [revisionId],
      authoritative_revision_ids: [revisionId],
      proposal_revision_ids: [],
      authoritative_commit_ids: [commitId],
      author_action_sequence: position,
      draft_artifact_refs: [],
      artifact_lifecycle_event_refs: [],
      condition_refs: [],
      result: "authoritative_applied",
      created_at: "2026-08-15T08:00:00.000Z",
    },
    effect: {
      kind: "authoritative_applied",
      authoritative_revision: { revision_id: revisionId, body: options.body ?? "Base!?" },
      authoritative_commit_id: commitId,
      author_action_sequence: position,
      project_activity_position: position,
    },
    completed_intent_record_id: requireString(
      options.request.completed_intent_record_id,
      "completed_intent_record_id",
    ),
    local_intent_sequence: requireString(
      options.request.local_intent_sequence,
      "local_intent_sequence",
    ),
  };
}

function canonicalJson(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort().map(
      (key) => [key, canonicalJson(Reflect.get(value, key))],
    ));
  }
  return value;
}

export interface AppliedActivityEventOptions {
  readonly commandId: string;
  readonly commitId: string;
  readonly correlationId: string;
  readonly eventId: string;
  readonly occurredAt: string;
  readonly receiptId: string;
  readonly revisionId: string;
  readonly sequence: string;
}

export async function createAppliedActivityEvent(
  options: AppliedActivityEventOptions,
): Promise<ProjectActivityEvent> {
  const payload: ProjectActivityEvent["payload"] = {
    chapter_id: CHAPTER,
    authoritative_revision_id: options.revisionId,
    authoritative_commit_id: options.commitId,
    author_action_sequence: options.sequence,
  };
  const digest = new Uint8Array(await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(JSON.stringify(canonicalJson(payload))),
  ));
  return {
    envelope_version: 1,
    activity_profile: "storyos.project-activity.v1",
    event_id: options.eventId,
    event_schema: "storyos.event.authoritative-author-edit-applied.v1",
    event_kind: "authoritative_author_edit_applied",
    project_scope: { owner_user_id: OWNER, project_id: PROJECT },
    requester_user_id: OWNER,
    actor: { kind: "author", id: OWNER },
    project_sequence: options.sequence,
    stream_sequence: options.sequence,
    agent_run_id: null,
    run_step_id: null,
    run_sequence: null,
    aggregate_ref: { kind: "chapter", id: CHAPTER },
    correlation_id: options.correlationId,
    causation: { kind: "command", id: options.commandId },
    command_id: options.commandId,
    receipt_ref: { kind: "domain_receipt", id: options.receiptId },
    occurred_at: options.occurredAt,
    recorded_at: options.occurredAt,
    payload,
    payload_digest: {
      algorithm: "sha256",
      profile: "storyos.event-payload.jcs.v1",
      value_hex_lowercase: [...digest]
        .map((byte) => byte.toString(16).padStart(2, "0"))
        .join(""),
    },
    application_wire_record_ref: options.eventId,
    limit_profile_revision: "storyos.foundation.absolute.v1",
  };
}
