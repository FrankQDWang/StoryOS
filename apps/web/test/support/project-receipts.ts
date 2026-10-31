import assert from "node:assert/strict";

import { queryStoryOSPostgres } from "./node-integration";

const USER_A = "018f0000-0000-7001-8000-000000000001";
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u;

export type ProjectReceiptExpectation = Readonly<{
  /** The Receipt count of each command kind. */
  receipts: Readonly<Record<string, number>>;
  /** `listed` counts only the command kinds of `receipts`. The default `all` counts each kind. */
  kinds?: "all" | "listed";
  authorActions?: number;
}>;

/** Asserts the Receipts of the Projects that one journey made, so that a changed count names that journey. */
export async function assertProjectReceipts(
  projectIds: readonly string[],
  expected: ProjectReceiptExpectation,
): Promise<void> {
  assert.ok(projectIds.length > 0 && projectIds.every((projectId) => UUID.test(projectId)));
  const projects = projectIds.map((projectId) => `'${projectId}'::uuid`).join(", ");
  const observed = JSON.parse(await queryStoryOSPostgres(`
    SELECT json_build_object(
      'receipts', (SELECT coalesce(json_object_agg(command_kind, count), '{}') FROM (
        SELECT command_kind, count(*) FROM storyos.domain_receipts
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id IN (${projects})
        GROUP BY command_kind) AS receipts),
      'author_actions', (SELECT count(*) FROM storyos.author_action_entries
        WHERE owner_user_id = '${USER_A}'::uuid AND project_id IN (${projects}))
    )::text`)) as { receipts: Record<string, number>; author_actions: number };
  const kinds = expected.kinds === "listed"
    ? Object.keys(expected.receipts)
    : Object.keys(observed.receipts);
  assert.deepEqual({
    receipts: Object.fromEntries(kinds.map((kind) => [kind, observed.receipts[kind] ?? 0])),
    authorActions: expected.authorActions === undefined ? undefined : observed.author_actions,
  }, { receipts: expected.receipts, authorActions: expected.authorActions });
}
