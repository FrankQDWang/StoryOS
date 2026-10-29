// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { createHash, createHmac } from "node:crypto";
import { test } from "vitest";
import { applyAuthorEdit, createEditorSession, createProjectCommandChallenge, digestApplyAuthorEdit,
  digestCreateEditorSession, getApplyAuthorEditOutcome,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ApplyAuthorEditRequest, CreateEditorSessionRequest,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { BINDING, USER_A, UUID_V7, authorEditRequest, challenged, id, prepare } from "../support/acceptance.ts";
import { problemCode, startServer } from "../support/http-harness.ts";
import { TEST_CHALLENGE_SECRET, queryStoryOSPostgres, requireStoryOSProtocolError, stopStoryOSServer,
} from "../support/node-integration.ts";

const SHARED_REVISION = "storyos.project-command-challenge-rate.fixed-window.v1";
const AUTHOR_EDIT_REVISION = "storyos.project-command-challenge-rate.author-edit.fixed-window.v1";

/** The keyed digest of the Server: each part follows its length as a big-endian u64. */
function secretDigest(profile: string, parts: readonly Buffer[]): string {
  const digest = createHmac("sha256", TEST_CHALLENGE_SECRET);
  for (const part of [Buffer.from(profile), ...parts]) {
    const length = Buffer.alloc(8);
    length.writeBigUInt64BE(BigInt(part.length));
    digest.update(length).update(part);
  }
  return digest.digest("hex");
}

/**
 * Changes a stored Author Edit Challenge into one that a Server issued under the shared revision
 * before the command kind moved to the `author_edit` class. Returns that Challenge.
 */
async function issuedUnderSharedRevision(projectId: string, key: string, expired: boolean) {
  const where = `project_id = '${projectId}'::uuid AND command_kind = 'applyAuthorEdit' AND idempotency_key = '${key}'::uuid`;
  const row = JSON.parse(await queryStoryOSPostgres(`SELECT row_to_json(challenge)::text FROM (
    SELECT owner_user_id, project_id, client_session_binding_digest, client_session_generation::text AS generation,
      client_contract_revision, security_policy_revision, limit_profile_revision, method, route_template,
      command_schema, command_kind, canonical_command_digest, idempotency_key
    FROM storyos.project_command_challenges WHERE ${where}) AS challenge`)) as Record<string, string>;
  const generation = Buffer.alloc(8);
  generation.writeBigUInt64BE(BigInt(row.generation!));
  const nonce = secretDigest("storyos.project-command-challenge.nonce.hmac-sha256.v1", [
    row.owner_user_id, row.project_id, row.client_session_binding_digest, generation, row.client_contract_revision,
    row.security_policy_revision, row.limit_profile_revision, SHARED_REVISION, row.method, row.route_template,
    row.command_schema, row.command_kind, row.canonical_command_digest, row.idempotency_key,
  ].map((part) => typeof part === "string" ? Buffer.from(part) : part!));
  const expiresAt = await queryStoryOSPostgres(`WITH changed AS (UPDATE storyos.project_command_challenges AS challenge
    SET challenge_rate_policy_revision = '${SHARED_REVISION}',
      challenge_rate_window_started_at = (SELECT max(window_started_at)
        FROM storyos.project_command_challenge_rate_windows AS window_row
        WHERE (window_row.owner_user_id, window_row.project_id, window_row.client_session_generation)
          = (challenge.owner_user_id, challenge.project_id, challenge.client_session_generation)
          AND window_row.policy_revision = '${SHARED_REVISION}'),
      nonce_digest = 'sha256:${createHash("sha256").update(nonce).digest("hex")}'
      ${expired ? ", expires_at = clock_timestamp() - interval '1 second'" : ""}
    WHERE ${where} RETURNING expires_at)
    SELECT to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS"Z"') FROM changed`);
  return { nonce, expires_at: expiresAt };
}

test("A repeated Author Edit Challenge request across a rate policy revision returns the original Challenge", async () => {
  const started = await startServer();
  try {
    const { baseUrl } = started;
    const prepared = await prepare(baseUrl, id("b60011"), "Repeated Challenge Novel", "b602");
    const { projectId, fetchImpl } = prepared;
    const sessionRequest: CreateEditorSessionRequest = { command_schema: "storyos.command.create-editor-session.request.v1",
      ...BINDING, correlation_id: id("b60031") };
    const session = await challenged(baseUrl, fetchImpl, projectId, "POST", "/api/v1/projects/{project_id}/editor-sessions",
      sessionRequest.command_schema, await digestCreateEditorSession(sessionRequest), id("b60032"),
      (antiForgery) => createEditorSession({ baseUrl, projectId, fetchImpl, idempotencyKey: id("b60032"),
        antiForgery, request: sessionRequest }));
    const context = { prepared, session };
    const state = async () => JSON.parse(await queryStoryOSPostgres(`SELECT json_build_object(
      'issued', (SELECT coalesce(sum(issued_count), 0) FROM storyos.project_command_challenge_rate_windows
        WHERE project_id = '${projectId}'::uuid AND policy_revision = '${AUTHOR_EDIT_REVISION}'),
      'admissions', (SELECT count(*) FROM storyos.author_command_admissions
        WHERE project_id = '${projectId}'::uuid AND command_kind = 'applyAuthorEdit'),
      'consumed', (SELECT count(*) FROM storyos.project_command_challenges
        WHERE project_id = '${projectId}'::uuid AND command_kind = 'applyAuthorEdit' AND consumed_at IS NOT NULL)
    )::text`));
    const challengeFor = async (request: ApplyAuthorEditRequest, key: string) => createProjectCommandChallenge({
      baseUrl, projectId, fetchImpl, request: { method: "POST",
        route_template: "/api/v1/projects/{project_id}/manuscript/author-edits", command_schema: request.command_schema,
        canonical_command_digest: await digestApplyAuthorEdit(request), idempotency_key: key } });

    const request = await authorEditRequest(started, context, { kind: "replace_selection", from: 0, to: 0, text: "Hello" },
      { kind: "authoritative" }, "1", "b604");
    const key = id("b60045");
    const issued = await challengeFor(request, key);
    const original = await issuedUnderSharedRevision(projectId, key, /*expired*/ false);
    assert.deepEqual(await state(), { issued: 1, admissions: 0, consumed: 0 });
    assert.deepEqual(await challengeFor(request, key), { ...issued, nonce: original.nonce });
    assert.deepEqual(await state(), { issued: 1, admissions: 0, consumed: 0 });
    const edit = { baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery: original.nonce, request };
    const applied = await applyAuthorEdit(edit);
    assert.equal(applied.effect.kind, "authoritative_applied");
    assert.deepEqual(await applyAuthorEdit(edit), applied);
    assert.deepEqual(await state(), { issued: 1, admissions: 1, consumed: 1 });

    const expiredRequest = await authorEditRequest(started, context, { kind: "replace_selection", from: 5, to: 5, text: "!" },
      { kind: "authoritative" }, "2", "b605");
    const expiredKey = id("b60055");
    const expiredIssued = await challengeFor(expiredRequest, expiredKey);
    const expired = await issuedUnderSharedRevision(projectId, expiredKey, /*expired*/ true);
    assert.deepEqual(await challengeFor(expiredRequest, expiredKey), { ...expiredIssued, ...expired });
    await assert.rejects(() => applyAuthorEdit({ baseUrl, projectId, fetchImpl, idempotencyKey: expiredKey,
      antiForgery: expired.nonce, request: expiredRequest }), (error) => {
      const protocolError = requireStoryOSProtocolError(error);
      return protocolError.status === 422
        && problemCode(error) === "challenge_invalid";
    });
    const { correlation_id: correlationId, ...outcome } = await getApplyAuthorEditOutcome({ baseUrl, projectId,
      idempotencyKey: expiredKey, antiForgery: expired.nonce, fetchImpl });
    assert.match(correlationId, UUID_V7);
    assert.deepEqual(outcome, {
      schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
      project_scope: { owner_user_id: USER_A, project_id: projectId },
      outcome: { outcome_kind: "rejected", reason: "challenge_expired_unconsumed" },
    });
    assert.deepEqual(await state(), { issued: 2, admissions: 1, consumed: 1 });
  } finally {
    await stopStoryOSServer(started.server);
  }
});
