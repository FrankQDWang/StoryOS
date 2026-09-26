import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { createProjectCommandChallenge, digestExportProjectArchive, exportProjectArchive, getExportOperation }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { uuidV7 } from "../../src/acceptance-journal.ts";
import { queryStoryOSPostgres, runStoryOSWorker, sessionFetch } from "./node-integration.ts";
import { zipStoreFiles } from "./archive.ts";
import { readFile, writeFile } from "node:fs/promises";
import type { BrowserContext, Page } from "playwright";
import type { CloseEditorFlowDraftRequest, CloseEditorFlowDraftResponse, RefusedEditDraftInspect }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DiscardRecord } from "../../src/refused-edit-discard.ts";

const USER = "018f0000-0000-7001-8000-000000000001";
export async function readProductionJournal(page: Page, projectId: string) {
  return page.evaluate((projectId) => new Promise<Record<string, Record<string, unknown>[]>>((resolve, reject) => {
    const open = indexedDB.open(`storyos-local-edit-journal:018f0000-0000-7001-8000-000000000001:${projectId}`);
    open.onupgradeneeded = () => open.transaction?.abort();
    open.onerror = () => reject(open.error);
    open.onsuccess = () => {
      const db = open.result;
      const stores = ["metadata", "partitions", "intents", "submission_groups", "payload_chains"];
      const transaction = db.transaction(stores);
      const values: Record<string, Record<string, unknown>[]> = {};
      for (const store of stores) { const read = transaction.objectStore(store).getAll();
        read.onsuccess = () => { values[store] = read.result as Record<string, unknown>[]; }; }
      transaction.oncomplete = () => { db.close(); resolve(values); };
      transaction.onerror = () => { db.close(); reject(transaction.error); };
    };
  }), projectId);
}
async function readObjects(page: Page, projectId: string, chapterId: string, proposalId: string) {
  return page.evaluate(async ({ projectId, chapterId, proposalId }) => Promise.all([
    `/api/v1/projects/${projectId}/chapters/${chapterId}`, `/api/v1/projects/${projectId}/proposals/${proposalId}`,
  ].map(async (url) => { const response = await fetch(url); if (!response.ok) throw new Error(`Object read ${response.status}`);
    const value = await response.json() as Record<string, unknown>; return value.chapter ?? value.proposal; })),
  { projectId, chapterId, proposalId });
}

export async function verifyProductionDiscard({ page, context, origin, projectId, chapterId, proposalId, draft, restart }: {
  page: Page; context: BrowserContext; origin: string; projectId: string; chapterId: string; proposalId: string;
  draft: RefusedEditDraftInspect; restart: () => Promise<void>;
}) {
  await page.reload();
  const surface = page.locator(`[data-refused-edit-draft="${draft.draft_id}"]`);
  await surface.locator("button[data-draft-discard]").waitFor();
  const before = await readObjects(page, projectId, chapterId, proposalId);
  const sessionRoute = (url: URL) => url.pathname.includes("/editor-sessions/");
  await page.route(sessionRoute, async (route) => {
    const original = await route.fetch(); const session = await original.json();
    await route.fulfill({ response: original, json: { ...session, writer: { kind: "read_only", reason: "secondary_session",
      observed_writer_generation: session.writer.writer_generation } } });
  });
  await surface.locator("button[data-draft-discard]").click();
  await surface.locator("button[data-draft-discard]:not([disabled])").waitFor();
  assert.equal((await readProductionJournal(page, projectId)).metadata!.filter((record) => String(record.key).startsWith("discard:")).length, 0);
  await page.unroute(sessionRoute);
  const queryRoute = (url: URL) => url.pathname.endsWith(`/refused-edit-drafts/${draft.draft_id}`);
  await page.route(queryRoute, async (route) => {
    const original = await route.fetch(); const value = await original.json();
    await route.fulfill({ response: original, json: { ...value, draft: { ...value.draft, payload_digest: "0".repeat(64) } } });
  });
  await surface.locator("button[data-draft-discard]").click();
  await page.locator(`[data-draft-unavailable="${draft.draft_id}"]`).waitFor();
  assert.equal((await readProductionJournal(page, projectId)).metadata!.filter((record) => String(record.key).startsWith("discard:")).length, 0);
  await page.unroute(queryRoute);
  await page.reload();
  await surface.locator("button[data-draft-discard]").waitFor();
  let posts = 0;
  let frozen: DiscardRecord | undefined;
  let reply: CloseEditorFlowDraftResponse | undefined;
  let nonce = "";
  let release!: () => void;
  const completed = new Promise<void>((resolve) => { release = resolve; });
  await page.route((url) => url.pathname.endsWith(`/drafts/${draft.draft_id}/closures`), async (route) => {
    posts += 1;
    const request = route.request().postDataJSON() as CloseEditorFlowDraftRequest;
    const key = route.request().headers()["idempotency-key"]!;
    nonce = route.request().headers()["x-storyos-anti-forgery"]!;
    frozen = (await readProductionJournal(page, projectId)).metadata!.find((record) => record.key === `discard:${draft.draft_id}`) as DiscardRecord;
    assert.ok(frozen);
    assert.deepEqual(frozen.group.frozen_request_body, request);
    assert.equal(frozen.group.idempotency_key, key);
    assert.deepEqual(request.close_editor_flow_draft_input, { draft_id: draft.draft_id, draft_kind: "refused_edit",
      source_current_draft_revision_id: draft.draft_revision_id, source_draft_payload_digest: draft.payload_digest,
      expected_closure: "open", close_reason: "abandoned", editor_session_id: frozen.editor_session_id,
      writer_generation: frozen.writer_generation, client_contract_revision: "storyos.web-client.release-1.v3",
      security_policy_revision: "storyos.web-security-policy.release-1.v1", correlation_id: request.close_editor_flow_draft_input.correlation_id });
    assert.ok(!JSON.stringify(frozen).includes("nonce"));
    const response = await route.fetch(); assert.equal(response.status(), 200, await response.text());
    reply = await response.json() as CloseEditorFlowDraftResponse;
    await route.abort("failed"); release();
  });
  await surface.locator("button[data-draft-discard]").click();
  await completed;
  assert.ok(frozen && reply && reply.effect.kind === "draft_closure_changed");
  const closed = { ...draft, closure: "closed", closure_event: reply.effect.event };
  await surface.locator("[data-draft-closed]").waitFor();
  assert.equal(await surface.locator("[data-draft-closed]").textContent(),
    `Closed: abandoned. Event: ${reply.effect.event.event_id}. Undo unavailable: non-skippable Barrier.`);
  await restart(); await page.reload();
  await surface.locator("[data-draft-closed]").waitFor();
  assert.equal(posts, 1);
  assert.equal(await surface.locator("button[data-draft-discard]").count(), 0);
  assert.deepEqual(await readObjects(page, projectId, chapterId, proposalId), before);
  assert.deepEqual((await readProductionJournal(page, projectId)).metadata!.find((record) => record.key === frozen!.key), frozen);
  await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin });
  const mutations: string[] = [];
  const track = (request: import("playwright").Request) => { if (request.method() !== "GET") mutations.push(request.url()); };
  page.on("request", track);
  await surface.locator("button[data-draft-copy]").click(); await surface.getByText("Copied").waitFor();
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "Complete mixed replacement");
  page.off("request", track); assert.deepEqual(mutations, []);
  const fetchImpl = sessionFetch(origin, "session-a");
  const options = { baseUrl: origin, projectId, fetchImpl };
  const request = { command_schema: "storyos.command.export-project-archive.request.v1", export_project_archive_input: {
    client_contract_revision: "storyos.web-client.release-1.v3", security_policy_revision: "storyos.web-security-policy.release-1.v1",
    correlation_id: uuidV7(crypto), archive_profile: "storyos.project-export.v1", archive_path_profile: "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1" } };
  const key = uuidV7(crypto);
  const challenge = await createProjectCommandChallenge({ ...options, request: { method: "POST",
    route_template: "/api/v1/projects/{project_id}/exports", command_schema: request.command_schema,
    canonical_command_digest: await digestExportProjectArchive(request), idempotency_key: key } });
  const admitted = await exportProjectArchive({ ...options, request, idempotencyKey: key, antiForgery: challenge.nonce });
  assert.equal(admitted.effect.kind, "admitted");
  if (admitted.effect.kind !== "admitted") throw new Error("Production Draft export not admitted");
  const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
  await runStoryOSWorker({ repositoryRoot, workerBinary: `${repositoryRoot}/target/release-package/storyos-worker`, args: ["--once"] });
  const exported = await getExportOperation({ ...options, exportId: admitted.effect.export_id });
  assert.equal(exported.status, "ready");
  if (exported.status !== "ready") throw new Error("Production Draft export not ready");
  const download = await fetchImpl(`${origin}/api/v1/projects/${projectId}/exports/${exported.export_id}`,
    { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
  assert.equal(download.status, 200);
  const bytes = new Uint8Array(await download.arrayBuffer());
  const entries = zipStoreFiles(bytes);
  for (const table of ["draft_artifacts", "draft_artifact_revisions", "draft_lifecycle_events", "draft_close_events", "domain_receipts", "author_action_entries"]) {
    const expected = JSON.parse(await queryStoryOSPostgres(`SELECT coalesce(jsonb_agg(to_jsonb(record) ORDER BY to_jsonb(record)::text), '[]'::jsonb)::text
      FROM storyos.${table} AS record WHERE owner_user_id='${USER}'::uuid AND project_id='${projectId}'::uuid`));
    const actual = JSON.parse(new TextDecoder().decode(entries.get(`canonical/${table}.json`)));
    const order = (a: Record<string, unknown>, b: Record<string, unknown>) =>
      JSON.stringify(a, Object.keys(a).sort()).localeCompare(JSON.stringify(b, Object.keys(b).sort()));
    assert.deepEqual(actual.sort(order), expected.sort(order));
  }
  for (const content of entries.values()) { const text = new TextDecoder().decode(content);
    assert.ok(!text.includes(nonce)); assert.ok(!text.includes(frozen.explicit_command_record_id)); }
  const archive = { exportId: exported.export_id, root: exported.immutable_root,
    bytesSha256: createHash("sha256").update(bytes).digest("hex") };
  const file = process.env.STORYOS_DISCARD_RECOVERY_EXPECTED;
  if (file !== undefined) {
    const journal = await readProductionJournal(page, projectId);
    journal.metadata = journal.metadata!.filter((record) => !String(record.key).startsWith("acceptance:")
      && !String(record.key).startsWith("rejection:"));
    const sessionStorage = await page.evaluate((projectId) => Object.fromEntries(Object.entries(window.sessionStorage)
      .filter(([key]) => key.endsWith(`:${projectId}`) && (key.startsWith("active_session:") || key.startsWith("block_proposals:")))), projectId);
    await writeFile(file, JSON.stringify({ projectId, chapterId, proposalId, draft: closed, objects: before, journal, sessionStorage, archive }));
  }
}

export async function verifyRestoredProductionDiscard(context: BrowserContext): Promise<void> {
  const file = process.env.STORYOS_DISCARD_RECOVERY_EXPECTED;
  const origin = process.env.STORYOS_DEV_SERVER;
  assert.ok(file && origin);
  const expected = JSON.parse(await readFile(file, "utf8")) as { projectId: string; chapterId: string; proposalId: string;
    draft: RefusedEditDraftInspect; objects: unknown[]; journal: Record<string, Record<string, unknown>[]>;
    sessionStorage: Record<string, string>; archive: { exportId: string; root: string; bytesSha256: string } };
  const page = await context.newPage();
  try {
    await page.goto(`${origin}/projects/${expected.projectId}`);
    await page.locator("[data-manuscript-editor]").waitFor();
    await page.evaluate(({ expected, user }) => new Promise<void>((resolve, reject) => {
      const open = indexedDB.open(`storyos-local-edit-journal:${user}:${expected.projectId}`);
      open.onerror = () => reject(open.error);
      open.onsuccess = () => {
        const db = open.result; const transaction = db.transaction(Object.keys(expected.journal), "readwrite");
        for (const [store, rows] of Object.entries(expected.journal)) for (const row of rows) transaction.objectStore(store).put(row);
        transaction.oncomplete = () => { db.close();
          for (const [key, value] of Object.entries(expected.sessionStorage)) sessionStorage.setItem(key, value); resolve(); };
        transaction.onerror = () => { db.close(); reject(transaction.error); };
      };
    }), { expected, user: USER });
    const mutations: string[] = [];
    page.on("request", (request) => { if (request.method() !== "GET") mutations.push(request.url()); });
    await page.reload();
    const surface = page.locator(`[data-refused-edit-draft="${expected.draft.draft_id}"]`);
    await surface.locator("[data-draft-closed]").waitFor();
    assert.deepEqual(await readObjects(page, expected.projectId, expected.chapterId, expected.proposalId), expected.objects);
    const restored = await page.evaluate(async ({ projectId, draft }) => {
      const response = await fetch(`/api/v1/projects/${projectId}/refused-edit-drafts/${draft.draft_id}`);
      if (!response.ok) throw new Error(`Restored Draft read ${response.status}`);
      return (await response.json()).draft;
    }, expected);
    assert.deepEqual(restored, expected.draft);
    assert.equal(await surface.locator("button[data-draft-discard]").count(), 0);
    await context.grantPermissions(["clipboard-read", "clipboard-write"], { origin });
    await surface.locator("button[data-draft-copy]").click(); await surface.getByText("Copied").waitFor();
    assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "Complete mixed replacement");
    assert.deepEqual(mutations, []);
    const fetchImpl = sessionFetch(origin, "session-a");
    const exported = await getExportOperation({ baseUrl: origin, projectId: expected.projectId,
      exportId: expected.archive.exportId, fetchImpl });
    assert.equal(exported.status, "ready");
    if (exported.status !== "ready") throw new Error("Restored archive unavailable");
    assert.equal(exported.immutable_root, expected.archive.root);
    const download = await fetchImpl(`${origin}/api/v1/projects/${expected.projectId}/exports/${expected.archive.exportId}`,
      { headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' } });
    assert.equal(download.status, 200);
    assert.equal(createHash("sha256").update(new Uint8Array(await download.arrayBuffer())).digest("hex"), expected.archive.bytesSha256);
    const current = await readProductionJournal(page, expected.projectId);
    const original = expected.journal.metadata!.find((record) => String(record.key).startsWith("discard:"));
    assert.deepEqual(current.metadata!.find((record) => record.key === original!.key), original);
    console.log(`Restored production Discard ${expected.projectId}/${expected.draft.draft_id}: exact local record, closed event and full Copy`);
  } finally { await page.close(); }
}
