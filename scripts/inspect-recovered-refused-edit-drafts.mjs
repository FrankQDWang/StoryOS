import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { getExportOperation, getRefusedEditDraft, StoryOSProtocolError } from "../generated/typescript/storyos-public-release-1/client.mjs";

const [container, baseUrl, expectedPath] = process.argv.slice(2);
const expectations = readFileSync(expectedPath, "utf8").trim().split("\n").map(JSON.parse);
assert.equal(expectations.length, 3);
const fetchImpl = (input, init) => {
  const headers = new Headers(init?.headers);
  headers.set("origin", baseUrl);
  headers.set("cookie", "storyos_session=session-a");
  return fetch(input, { ...init, headers });
};
let restoredDrafts = 0;
for (const expected of expectations) {
  assert.match(expected.projectId, /^[0-9a-f-]{36}$/);
  assert.match(expected.ownerUserId, /^[0-9a-f-]{36}$/);
  const tableEntries = Object.keys(expected.state).filter((table) => table !== "domain_receipts");
  const allowed = new Set(["projects", "authoritative_heads", "authoritative_revisions", "authoritative_commits",
    "author_action_entries", "project_activity_events", "scope_counters", "proposals", "proposal_heads",
    "proposal_revisions", "proposal_operations", "draft_artifacts", "draft_artifact_revisions", "draft_lifecycle_events"]);
  for (const table of tableEntries) assert.ok(allowed.has(table));
  const state = JSON.parse(execFileSync("docker", ["exec", container, "psql", "-X", "-v", "ON_ERROR_STOP=1",
    "-U", "postgres", "-Atc", `SELECT jsonb_build_object(${tableEntries.map((table) =>
      `'${table}', (SELECT coalesce(jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text), '[]'::jsonb)
        FROM storyos.${table} AS record WHERE record.owner_user_id='${expected.ownerUserId}'::uuid
        AND record.project_id='${expected.projectId}'::uuid)`).join(",")},
      'domain_receipts', (SELECT coalesce(jsonb_agg(to_jsonb(receipt) ORDER BY receipt.receipt_id), '[]'::jsonb)
        FROM storyos.domain_receipts AS receipt JOIN storyos.draft_lifecycle_events AS event
        USING (owner_user_id,project_id,receipt_id) WHERE event.owner_user_id='${expected.ownerUserId}'::uuid
        AND event.project_id='${expected.projectId}'::uuid))::text`], { encoding: "utf8", maxBuffer: 4 * 1024 * 1024 }));
  assert.deepEqual(state, expected.state);
  for (const retained of expected.drafts) {
    const query = () => getRefusedEditDraft({ baseUrl, projectId: expected.projectId,
      draftId: retained.draft.draft_id, fetchImpl });
    if (retained.available) {
      assert.deepEqual((await query()).draft, retained.draft);
    } else {
      await assert.rejects(query, (error) => error instanceof StoryOSProtocolError && error.status === 404);
    }
    restoredDrafts += 1;
  }
  for (const retained of expected.exports) {
    const operation = await getExportOperation({ baseUrl, projectId: expected.projectId,
      exportId: retained.exportId, fetchImpl });
    assert.equal(operation.status, "ready");
    assert.equal(operation.immutable_root, retained.root);
    const download = await fetchImpl(`${baseUrl}/api/v1/projects/${expected.projectId}/exports/${retained.exportId}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    assert.equal(download.status, retained.status);
    if (retained.status === 200) {
      assert.equal(createHash("sha256").update(new Uint8Array(await download.arrayBuffer())).digest("hex"), retained.bytesSha256);
    } else {
      assert.deepEqual(await download.json(), { schema_id: "storyos.problem.v1", code: "ineligible_lifecycle",
        message: "The Project Export Archive did not complete." });
    }
  }
}
assert.equal(restoredDrafts, 5);
console.log("Restored five public Refused Edit Drafts: complete payloads, digests, creation, Receipts, Heads, lifecycle, and archive copies unchanged");
