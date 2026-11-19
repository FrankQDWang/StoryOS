import type { DigestValue, GetEditorSessionResponse }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { createJournalUuid } from "../../src/local-edit-journal.ts";
import {
  SESSION, chapterRevision, createAppliedAuthorEditResponse, createBrowserScenario, jsonResponse,
  requestHeaders, requireDigestValue, requireRequestBody,
} from "./scenario.ts";

/** A fake Server that applies each Author Edit and moves the Editor Session to the new revision. */
export function appliedAuthorEditTransport(scenario: ReturnType<typeof createBrowserScenario>,
  currentBody: () => string) {
  let position = 0;
  let commandDigest: DigestValue | undefined;
  let session: GetEditorSessionResponse = {
    ...scenario.session, schema_id: "storyos.query.editor-session.response.v1",
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      commandDigest = requireDigestValue(
        requireRequestBody(init).canonical_command_digest, "command digest",
      );
      return jsonResponse({ nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(session);
    if (path.endsWith("/manuscript/author-edits")) {
      const body = currentBody();
      const request = requireRequestBody(init);
      const idempotencyKey = requestHeaders(init).get("idempotency-key");
      if (!commandDigest || !idempotencyKey) throw new Error("missing admitted identity");
      const revisionId = createJournalUuid();
      const response = createAppliedAuthorEditResponse({ request, commandDigest,
        idempotencyKey, body, projectActivityPosition: String(++position),
        priorRevisionId: session.base_snapshot.authoritative_head_revision_id,
        authoritativeRevisionId: revisionId, commandId: createJournalUuid(),
        authorCommandAdmissionId: createJournalUuid(), receiptId: createJournalUuid(),
        authoritativeCommitId: createJournalUuid(),
      });
      const digest = new Uint8Array(await crypto.subtle.digest(
        "SHA-256", new TextEncoder().encode(body),
      ));
      session = { ...session, base_snapshot: { ...session.base_snapshot,
        snapshot_id: createJournalUuid(), project_activity_position: String(position),
        authoritative_head_revision_id: revisionId,
        materialized_revision: chapterRevision(revisionId, body),
        materialized_payload_digest: { algorithm: "sha256",
          profile: "storyos.canonical-payload.sha256.v1",
          value_hex_lowercase: [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join(""),
        },
      } };
      return jsonResponse(response);
    }
    throw new Error(`unexpected request ${path}`);
  };
  return { fetchImpl, session: () => session };
}
