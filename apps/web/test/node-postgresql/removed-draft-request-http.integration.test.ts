// Verification: {"phase":"http-main","after":[]}
import assert from "node:assert/strict";
import { test } from "vitest";

import { applyAuthorEdit, digestApplyAuthorEdit } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ApplyAuthorEditRequest } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres as queryPostgres, requireStoryOSProtocolError,
  stopStoryOSServer as stopRealServer } from "../support/node-integration.ts";
import { PROSE, USER_A, authorEditRequest, challenged, drainLeftoverWork, id, openedBlockProposal,
} from "../support/acceptance.ts";
import { startServer } from "../support/http-harness.ts";

// ADR 0073: an earlier client can still send these Refused Edit Draft shapes.
type EarlierClientRequest = Omit<ApplyAuthorEditRequest, "author_edit_units"> & Record<string, unknown>
  & { author_edit_units: Record<string, unknown>[] };

async function retainedState(projectId: string) {
  const tables = ["authoritative_heads", "authoritative_revisions", "author_command_admissions", "domain_receipts",
    "author_action_entries", "project_activity_events", "proposal_heads", "proposal_revisions", "draft_artifacts"];
  return queryPostgres(`SELECT jsonb_build_object(${tables.map((table) =>
    `'${table}', (SELECT coalesce(jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text), '[]'::jsonb)
      FROM storyos.${table} AS record WHERE record.owner_user_id = '${USER_A}'::uuid
      AND record.project_id = '${projectId}'::uuid)`).join(",")})::text`);
}

test("an earlier crossing edit or Draft retry gets invalid_request_shape and changes no record", async () => {
  const started = await startServer();
  try {
    await drainLeftoverWork();
    const opened = await openedBlockProposal(started, "be1");
    const { projectId, fetchImpl } = opened.prepared;
    const candidate = opened.revised.candidate_text;
    const blockId = opened.revised.manuscript_block_id;
    const fresh = async (ns: string) => await authorEditRequest(started, opened,
      { kind: "replace_selection", from: 0, to: 0, text: "" }, { kind: "acknowledged", proposal: opened.revised },
      "2", ns) as EarlierClientRequest;
    const crossing = await fresh("be21");
    crossing.author_edit_units = [{ normalized_primitives: [{ kind: "replace_structured_selection",
      replacement: [{ block_kind: "paragraph", text: "New passage" }] }],
    selection_snapshot: { coordinate_profile: "storyos.editor.ordered-source.v1", from: 6, to: 4,
      ordered_selection: { anchor: { source_index: 0, source_offset: 6 }, head: { source_index: 1, source_offset: 4 },
        sources: [
          { owner: { kind: "manuscript", manuscript_block_id: blockId }, coordinate_profile: "prosemirror-token-utf16.v1",
            from: 6, to: PROSE.length, block_kind: "paragraph", source_text: PROSE },
          { owner: { kind: "proposal", proposal_id: opened.revised.proposal_id, operation_id: opened.revised.operation_id,
            revision_id: opened.revised.revision_id, manuscript_block_id: blockId },
          coordinate_profile: "storyos.editor.utf16-code-unit.v1", from: 0, to: 4, block_kind: "paragraph",
          source_text: candidate },
        ] } } }];
    const retry = await fresh("be22");
    retry.retry_source = { kind: "draft_retry", source_draft_kind: "refused_edit", source_draft_id: id("be2201"),
      source_current_draft_revision_id: id("be2202"), source_draft_payload_digest: "a".repeat(64),
      expected_source_draft_closure: "open", selected_payload_range: { kind: "exact_structured_range",
        coordinate_profile: "storyos.draft-replacement.block-utf16.v1", from: { block_index: 0, offset: 0 },
        to: { block_index: 0, offset: 4 }, slice_digest: "b".repeat(64) } };
    const before = await retainedState(projectId);
    for (const [index, earlier] of [crossing, retry].entries()) {
      const key = id(`be3${index}`);
      const request = earlier as ApplyAuthorEditRequest;
      const refused = await challenged(started.baseUrl, fetchImpl, projectId, "POST",
        "/api/v1/projects/{project_id}/manuscript/author-edits", request.command_schema,
        await digestApplyAuthorEdit(request), key, (antiForgery) => applyAuthorEdit({ baseUrl: started.baseUrl,
          projectId, fetchImpl, request, idempotencyKey: key, antiForgery }).then(
          (response) => ({ status: 200, code: response.effect.kind }),
          (error: unknown) => { const protocol = requireStoryOSProtocolError(error);
            return { status: protocol.status, code: JSON.parse(String(protocol.responseBody)).code }; }));
      assert.deepEqual(refused, { status: 422, code: "invalid_request_shape" });
    }
    assert.equal(await retainedState(projectId), before);
    assert.equal(await queryPostgres(`SELECT count(*)::text FROM storyos.draft_artifacts`), "0");
  } finally { await stopRealServer(started.server); }
});
