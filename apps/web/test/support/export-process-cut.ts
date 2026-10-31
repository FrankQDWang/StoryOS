import assert from "node:assert/strict";

import type { DigestValue } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { USER_A, challenged, id, runWorker } from "./acceptance.ts";
import { createEmpty, restartServer, startServer } from "./http-harness.ts";
import { queryStoryOSPostgres, sessionFetch, stopStoryOSServer } from "./node-integration.ts";

const SESSION = "session-a";
const SERVER = { sessions: { [SESSION]: USER_A }, extraEnv: { STORYOS_WORKER: "0" } };
const EXPIRED_LEASE = { extraEnv: { STORYOS_EXPORT_LEASE_TTL_SECS: "0" } };

/** One export command of the export process cuts, with its fixed identifiers. */
export interface ExportFamily<Request extends { command_schema: string }> {
  readonly title: string;
  /** The two lead hex digits of the fixed idempotency keys and correlation IDs of the family. */
  readonly prefix: string;
  readonly route: string;
  readonly commandKind: string;
  /** The field that only a ready export has. */
  readonly readyField: string;
  request(correlationId: string): Request;
  digest(request: Request): Promise<DigestValue>;
  send(options: { baseUrl: string; projectId: string; fetchImpl: typeof fetch; idempotencyKey: string;
    antiForgery: string; request: Request }): Promise<{ acknowledgement: string; effect: ExportEffect }>;
  read(options: { baseUrl: string; projectId: string; exportId: string; fetchImpl: typeof fetch }): Promise<ExportRead>;
  /** Proves the ready output through the Server at this URL. */
  assertReady(ready: ExportRead, baseUrl: string, projectId: string, exportId: string): Promise<void>;
}

type ExportEffect = { kind: "admitted"; export_id: string } | { kind: string };

export interface ExportRead {
  readonly status: string;
  readonly export_id: string;
}

async function admit<Request extends { command_schema: string }>(family: ExportFamily<Request>, baseUrl: string,
  ids: { project: string; projectCorrelation: string; export: string; exportCorrelation: string }) {
  const { fetchImpl, projectId } = await createEmpty(baseUrl, SESSION, id(`${family.prefix}${ids.project}`),
    family.title, id(`${family.prefix}${ids.projectCorrelation}`));
  const request = family.request(id(`${family.prefix}${ids.exportCorrelation}`));
  const key = id(`${family.prefix}${ids.export}`);
  const admitted = await challenged(baseUrl, fetchImpl, projectId, "POST", family.route, request.command_schema,
    await family.digest(request), key,
    (antiForgery) => family.send({ baseUrl, projectId, fetchImpl, idempotencyKey: key, antiForgery, request }));
  if (!("export_id" in admitted.effect)) throw new Error(`${family.commandKind} must admit`);
  return { admitted, projectId, exportId: admitted.effect.export_id };
}

function read<Request extends { command_schema: string }>(family: ExportFamily<Request>, baseUrl: string,
  projectId: string, exportId: string) {
  return family.read({ baseUrl, projectId, exportId, fetchImpl: sessionFetch(baseUrl, SESSION) });
}

/**
 * An admitted export stays in progress across a Server process cut. A Worker claim without settlement
 * then gives `outcome_unknown`, and a takeover settles the export ready.
 */
export async function assertClaimWithoutSettlementTakesOver<Request extends { command_schema: string }>(
  family: ExportFamily<Request>): Promise<void> {
  await runWorker(["--once"], EXPIRED_LEASE);
  await runWorker(["--once"], EXPIRED_LEASE);
  let first = await startServer(SERVER);
  try {
    const { admitted, projectId, exportId } = await admit(family, first.baseUrl,
      { project: "02", projectCorrelation: "12", export: "22", exportCorrelation: "13" });
    assert.equal(admitted.acknowledgement, "accepted");
    assert.equal(admitted.effect.kind, "admitted");
    first = await restartServer(first, SERVER);
    const afterCut = await read(family, first.baseUrl, projectId, exportId);
    assert.equal(afterCut.status, "in_progress");
    assert.equal(afterCut.export_id, exportId);
    assert.equal(family.readyField in afterCut, false);
    await stopStoryOSServer(first.server);

    await runWorker(["--claim-only"], EXPIRED_LEASE);
    const afterClaim = await startServer(SERVER);
    try {
      const unknown = await read(family, afterClaim.baseUrl, projectId, exportId);
      assert.equal(unknown.status, "outcome_unknown");
      assert.equal(family.readyField in unknown, false);
      const receipts = await queryStoryOSPostgres(`
        SELECT count(*)::text
          FROM storyos.domain_receipts
         WHERE owner_user_id = '${USER_A}'::uuid
           AND project_id = '${projectId}'::uuid
           AND command_kind = '${family.commandKind}';
      `);
      assert.match(receipts, /0$/);
    } finally {
      await stopStoryOSServer(afterClaim.server);
    }

    await runWorker(["--once"], EXPIRED_LEASE);
    const afterSettle = await startServer(SERVER);
    try {
      const ready = await read(family, afterSettle.baseUrl, projectId, exportId);
      assert.equal(ready.status, "ready");
      await family.assertReady(ready, afterSettle.baseUrl, projectId, exportId);
    } finally {
      await stopStoryOSServer(afterSettle.server);
    }
  } catch (error) {
    await stopStoryOSServer(first.server);
    throw error;
  }
}
