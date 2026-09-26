import { appendFile } from "node:fs/promises";
import type { GetRefusedEditDraftResponse } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { queryStoryOSPostgres } from "./node-integration.ts";
import { USER_A } from "./acceptance.ts";

export async function retainRefusedEditRecoveryExpectation(
  projectId: string,
  drafts: { draft: GetRefusedEditDraftResponse["draft"]; available: boolean }[],
  exports: { exportId: string; root: string; status: number; bytesSha256?: string }[] = [],
): Promise<void> {
  const path = process.env.STORYOS_REFUSED_EDIT_RECOVERY_EXPECTED;
  if (path === undefined) return;
  const tables = ["projects", "authoritative_heads", "authoritative_revisions", "authoritative_commits",
    "author_action_entries", "project_activity_events", "scope_counters", "proposals", "proposal_heads",
    "proposal_revisions", "proposal_operations", "draft_artifacts", "draft_artifact_revisions", "draft_lifecycle_events", "draft_close_events", "draft_reopen_events", "draft_reopen_receipts"];
  const state = JSON.parse(await queryStoryOSPostgres(`SELECT jsonb_build_object(${tables.map((table) =>
    `'${table}', (SELECT coalesce(jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text), '[]'::jsonb)
      FROM storyos.${table} AS record WHERE record.owner_user_id = '${USER_A}'::uuid
      AND record.project_id = '${projectId}'::uuid)`).join(",")},
    'domain_receipts', (SELECT coalesce(jsonb_agg(to_jsonb(receipt) ORDER BY receipt.receipt_id), '[]'::jsonb)
      FROM storyos.domain_receipts AS receipt WHERE receipt.owner_user_id='${USER_A}'::uuid
      AND receipt.project_id='${projectId}'::uuid AND receipt.command_kind IN ('applyAuthorEdit','closeEditorFlowDraft','undoLatestAuthorAction') AND cardinality(receipt.draft_artifact_refs)>0))::text`));
  await appendFile(path, `${JSON.stringify({ projectId, ownerUserId: USER_A, drafts, exports, state })}\n`);
}
