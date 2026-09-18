import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { getProposal } from "../generated/typescript/storyos-public-release-1/client.mjs";

const [container, baseUrl] = process.argv.slice(2);
const rows = JSON.parse(execFileSync("docker", ["exec", container, "psql", "-X", "-v", "ON_ERROR_STOP=1",
  "-U", "postgres", "-Atc", `SELECT json_agg(row_to_json(evidence) ORDER BY evidence.proposal_id) FROM (
    SELECT condition.*, to_jsonb(receipt) AS acceptance, to_jsonb(validation.*) AS historical_validation
    FROM storyos.proposal_validation_conditions AS condition
    JOIN storyos.acceptance_receipts AS receipt USING (owner_user_id, project_id, acceptance_receipt_id)
    JOIN storyos.validation_receipts AS validation ON
      (validation.owner_user_id, validation.project_id, validation.proposal_id, validation.proposal_revision_id) =
      (condition.owner_user_id, condition.project_id, condition.proposal_id, condition.proposal_revision_id)
  ) AS evidence`], { encoding: "utf8" }));
assert.equal(rows.length, 3);
const inspected = [];
for (const row of rows) {
  const options = { baseUrl, projectId: row.project_id, proposalId: row.proposal_id };
  const readAs = (session) => getProposal({ ...options, fetchImpl: (input, init) => {
    const headers = new Headers(init?.headers);
    headers.set("origin", baseUrl);
    headers.set("cookie", `storyos_session=${session}`);
    return fetch(input, { ...init, headers });
  } });
  const result = await readAs("session-a");
  assert.equal(row.historical_validation.result, "valid");
  if (result.proposal.revision_id === row.proposal_revision_id) {
    assert.equal(result.proposal.validation, row.validation);
    assert.deepEqual(result.proposal.condition_refs, row.conflict_id ? [row.conflict_id] : []);
  }
  const isolated = execFileSync("docker", ["exec", container, "psql", "-X", "-v", "ON_ERROR_STOP=1",
    "-U", "postgres", "-Atc", `BEGIN; SET LOCAL ROLE storyos_runtime;
      SET LOCAL storyos.owner_user_id = '018f0000-0000-7001-8000-000000000101';
      SET LOCAL storyos.project_id = '${row.project_id}';
      SELECT count(*) FROM storyos.proposal_validation_conditions; ROLLBACK;`], { encoding: "utf8" });
  assert.match(isolated, /\n0\nROLLBACK/);
  inspected.push({ evidence: row, proposal: result.proposal });
}
process.stdout.write(JSON.stringify(inspected));
