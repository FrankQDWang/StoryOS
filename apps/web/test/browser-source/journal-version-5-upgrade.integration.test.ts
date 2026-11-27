import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it } from "vitest";

import { openEditorWorkspace, submitOnePendingAuthorEdit } from "../../src/editor-session.ts";
import { LocalRecoveryPanel } from "../../src/local-recovery-panel.tsx";
import inFlightFixture from "../../../../fixtures/local-edit-journal/version-5-in-flight-group.json";
import sentFixture from "../../../../fixtures/local-edit-journal/version-5-sent-group.json";
import unsentFixture from "../../../../fixtures/local-edit-journal/version-5-unsent-group.json";
import {
  OWNER, PROJECT, SESSION, closeTrackedDatabases, createBrowserScenario, deleteJournal, jsonResponse, requestHeaders,
  requestResult, requireEditorReady, requireRequestBody, trackDatabase,
} from "./scenario.ts";

// The Journals of these tests are rows that the released client of `main` `54525595` wrote at version 5 (ADR 0099
// rule for a later version, ADR 0107). The fixture notes give the requests of the recording.
type Fixture = { version: number; stores: { name: string; keyPath: string | string[];
  indexes: { name: string; keyPath: string | string[]; unique: boolean }[]; rows: Record<string, unknown>[] }[] };
const ACTIVE_KEY = `active_session:${OWNER}:${PROJECT}`;
const SESSION_READ = `GET /api/v1/projects/${PROJECT}/editor-sessions/${SESSION}`;
const CHALLENGE = `POST /api/v1/projects/${PROJECT}/anti-forgery-challenges`;
const AUTHOR_EDIT = `POST /api/v1/projects/${PROJECT}/manuscript/author-edits`;
const OUTCOME = `GET /api/v1/projects/${PROJECT}/manuscript/author-edit-outcomes/`;
const V2 = "storyos.command.apply-author-edit.request.v2";

const databases = new Set<IDBDatabase>();
const scenario = createBrowserScenario();

afterEach(async () => {
  closeTrackedDatabases(databases);
  sessionStorage.removeItem(ACTIVE_KEY);
  await deleteJournal(scenario.journalName);
});

/** Writes the rows of `fixture` as the Journal of the scenario at version 5. */
async function restore(fixture: Fixture) {
  await deleteJournal(scenario.journalName);
  const request = indexedDB.open(scenario.journalName, fixture.version);
  request.onupgradeneeded = () => {
    for (const { name, keyPath, indexes, rows } of fixture.stores) {
      const store = request.result.createObjectStore(name, { keyPath });
      for (const index of indexes) store.createIndex(index.name, index.keyPath, { unique: index.unique });
      for (const row of rows) store.add(row);
    }
  };
  (await requestResult(request)).close();
}

function rows(fixture: Fixture, store: string) {
  return fixture.stores.find(({ name }) => name === store)!.rows;
}

/**
 * A fake Server that serves the read of the stored Editor Session, a Challenge, an Author Edit, and an outcome Query, and
 * records each request. It holds the Author Edit and outcome Query responses, so the test reads the request that the
 * save sent. It throws on each other request.
 */
function fakeServer() {
  const requests: string[] = [];
  const nonces: (string | null)[] = [];
  const challenges: Record<string, unknown>[] = [];
  const edits: { body: Record<string, unknown>; key: string | null }[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    requests.push(name);
    if (name === SESSION_READ) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v2" });
    }
    if (name === CHALLENGE) {
      challenges.push(requireRequestBody(init));
      return jsonResponse({ nonce: "b".repeat(64), expires_at: new Date(Date.now() + 60_000).toISOString(),
        limit_profile_revision: scenario.profile.limit_profile_revision });
    }
    if (name === AUTHOR_EDIT) {
      edits.push({ body: requireRequestBody(init), key: requestHeaders(init).get("idempotency-key") });
      return new Promise<Response>(() => {});
    }
    if (name.startsWith(OUTCOME)) {
      nonces.push(requestHeaders(init).get("x-storyos-anti-forgery"));
      return new Promise<Response>(() => {});
    }
    throw new Error(`unexpected request: ${name}`);
  };
  return { fetchImpl, requests, challenges, edits, nonces };
}

async function openStored(server: ReturnType<typeof fakeServer>) {
  sessionStorage.setItem(ACTIVE_KEY, SESSION);
  const state = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: scenario.chapter, profile: scenario.profile, fetchImpl: server.fetchImpl });
  requireEditorReady(state);
  trackDatabase(state.database, databases);
  return state;
}

it("shows the unsent text of a version 5 Journal, and the next save sends its group as a v2 request", async () => {
  const fixture = unsentFixture as Fixture;
  await restore(fixture);
  const unsent = rows(fixture, "submission_groups").find((group) =>
    (group.settlement as { kind: string }).kind === "unsettled")!;
  const server = fakeServer();

  const state = await openStored(server);
  expect(state.openedProjection.body).toBe("Base!?");
  void submitOnePendingAuthorEdit({ workspace: state, baseUrl: location.origin, fetchImpl: server.fetchImpl,
    cryptoImpl: crypto });
  await expect.poll(() => server.edits.length).toBe(1);

  const { body, key } = server.edits[0]!;
  const frozen = unsent.frozen_request_body as Record<string, unknown>;
  expect({ requests: server.requests, challenge: server.challenges[0]!.command_schema, schema: body.command_schema,
    writer: "writer_generation" in body, units: body.author_edit_units, newKey: key !== unsent.idempotency_key })
    .toEqual({ requests: [SESSION_READ, CHALLENGE, AUTHOR_EDIT], challenge: V2, schema: V2, writer: false,
      units: frozen.author_edit_units, newKey: true });
});

// The POST of each group got no answer. In the first fixture the outcome Query got no answer too.
it.each([["an unresolved outcome Query", sentFixture], ["a POST in flight", inFlightFixture]] as const)(
  "shows the text of a version 5 group with %s, and the next save asks for its outcome and sends no POST", async (_,
    recorded) => {
  const fixture = recorded as Fixture;
  await restore(fixture);
  const [sent] = rows(fixture, "submission_groups");
  const [capsule] = rows(fixture, "transport_capsules");
  const server = fakeServer();

  const state = await openStored(server);
  expect(state.openedProjection.body).toBe("Base!");
  void submitOnePendingAuthorEdit({ workspace: state, baseUrl: location.origin, fetchImpl: server.fetchImpl,
    cryptoImpl: crypto });
  await expect.poll(() => server.nonces.length).toBe(1);

  // The outcome Query names the idempotency key and the nonce of the version 5 send (ADR 0046).
  expect({ requests: server.requests, nonces: server.nonces }).toEqual({
    requests: [SESSION_READ, `${OUTCOME}${String(sent!.idempotency_key)}`],
    nonces: [(capsule!.exact_client_controlled_headers as Record<string, string>)["X-StoryOS-Anti-Forgery"]] });
});

/**
 * The rows of `fixture` as the rows of the writer generation "2" of the same Editor Session, as after a takeover and a
 * takeover back. Each frozen request keeps the writer generation of the recording, so its digest stays.
 */
function secondGeneration(fixture: Fixture): Fixture {
  const prior = `:${SESSION}:1:`;
  return { ...fixture, stores: fixture.stores.map((store) => ({ ...store, rows: store.rows.map((row) => {
    const next = JSON.parse(JSON.stringify(row).replaceAll(prior, `:${SESSION}:2:`)) as Record<string, unknown>;
    return "writer_generation" in next ? { ...next, writer_generation: "2" } : next;
  }) })) };
}

/** The rows of both fixtures in one Journal. The global metadata rows come from `later`. */
function bothGenerations(earlier: Fixture, later: Fixture): Fixture {
  return { ...later, stores: later.stores.map((store) => {
    const keys = new Set(store.rows.map((row) => JSON.stringify(row.key)));
    const kept = rows(earlier, store.name).filter((row) => store.name !== "metadata" || !keys.has(JSON.stringify(row.key)));
    return { ...store, rows: [...kept, ...store.rows] };
  }) };
}

async function partitionRows(partitionId: string) {
  const database = await requestResult(indexedDB.open(scenario.journalName));
  trackDatabase(database, databases);
  const transaction = database.transaction(["intents", "submission_groups"]);
  return Promise.all(["intents", "submission_groups"].map((store) =>
    requestResult(transaction.objectStore(store).index("partition").getAll(partitionId))));
}

it("keeps the rows of a lower writer generation of the same Editor Session, and shows their text in local recovery", async () => {
  // A takeover and a takeover back gave one Editor Session rows in two partitions. The lower generation has the sent
  // group "!" of the recording, and the higher generation has the unsent group and the record "?".
  const lower = sentFixture as Fixture;
  await restore(bothGenerations(lower, secondGeneration(unsentFixture as Fixture)));
  const [partition] = rows(lower, "partitions");
  const server = fakeServer();

  const state = await openStored(server);
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () => root.render(createElement(LocalRecoveryPanel, { workspace: state, refreshKey: "0",
      onContinue: async () => {} })));

    expect(state.openedProjection.body).toBe("Base!?");
    await expect.poll(() => host.querySelector("[data-local-recovery-text]")?.textContent).toBe("Base!");
    expect(await partitionRows(String(partition!.journal_partition_id)))
      .toEqual([rows(lower, "intents"), rows(lower, "submission_groups")]);
    expect(server.requests).toEqual([SESSION_READ]);
  } finally {
    await act(async () => root.unmount());
    host.remove();
  }
});

it("changes the partition with rows when a higher writer generation of the same Editor Session has no rows", async () => {
  // After a takeover, the earlier writer tab read its Session as a read-only observer of generation "2", with no rows.
  const fixture = sentFixture as Fixture;
  const [partition] = rows(fixture, "partitions");
  const observer = secondGeneration({ ...fixture, stores: fixture.stores.map((store) => ({ ...store,
    rows: store.name === "partitions" ? [{ ...partition!, disposition: "read_only_observer" }]
      : store.name === "metadata" ? store.rows.filter((row) => String(row.key).startsWith("active_base:")) : [] })) });
  await restore(bothGenerations(observer, fixture));
  const server = fakeServer();

  const state = await openStored(server);

  expect(state.openedProjection.body).toBe("Base!");
});
