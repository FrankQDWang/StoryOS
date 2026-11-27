import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it } from "vitest";

import { openEditorWorkspace, submitOnePendingAuthorEdit } from "../../src/editor-session.ts";
import { rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import {
  editorSessionLock, freezeRejectedVersion5Groups, freezeUnsentVersion5Groups,
} from "../../src/journal-version-5-upgrade.ts";
import { LocalRecoveryPanel } from "../../src/local-recovery-panel.tsx";
import candidateFixture from "../../../../fixtures/local-edit-journal/version-5-candidate-record.json";
import identifierFixture from "../../../../fixtures/local-edit-journal/version-5-identifier-text-group.json";
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
const SESSION_CREATE = `POST /api/v1/projects/${PROJECT}/editor-sessions`;
const CHALLENGE = `POST /api/v1/projects/${PROJECT}/anti-forgery-challenges`;
const AUTHOR_EDIT = `POST /api/v1/projects/${PROJECT}/manuscript/author-edits`;
const OUTCOME = `GET /api/v1/projects/${PROJECT}/manuscript/author-edit-outcomes/`;
const V2 = "storyos.command.apply-author-edit.request.v2";
const NEW_SESSION = "018f0000-0000-7001-8000-0000000000e1";
const OTHER_SESSION = "018f0000-0000-7001-8000-0000000000e2";

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
 * A fake Server that serves only the `allowed` requests of a test and throws on each other request. It serves the read
 * and the creation of an Editor Session, a Challenge, an Author Edit, and an outcome Query, and records each request.
 * It holds the Author Edit response. It holds the outcome Query response, or it answers that the Server rejected the
 * command (`rejected`). A `missing` Session read answers 404.
 */
function fakeServer(allowed: readonly string[], answers: { outcome?: "rejected"; session?: "missing" } = {}) {
  const requests: string[] = [];
  const nonces: (string | null)[] = [];
  const challenges: Record<string, unknown>[] = [];
  const edits: { body: Record<string, unknown>; key: string | null }[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const name = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    requests.push(name);
    if (!allowed.includes(name)) throw new Error(`unexpected request: ${name}`);
    if (name === SESSION_READ) {
      return answers.session === "missing"
        ? jsonResponse({ schema_id: "storyos.problem.v1", code: "resource_unavailable", message: "Not found." }, 404)
        : jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v2" });
    }
    if (name === SESSION_CREATE) {
      return jsonResponse({ ...scenario.session,
        editor_session: { ...scenario.session.editor_session, editor_session_id: NEW_SESSION } });
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
    nonces.push(requestHeaders(init).get("x-storyos-anti-forgery"));
    return answers.outcome === "rejected" ? jsonResponse({
      schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
      correlation_id: "018f0000-0000-7001-8000-000000000082", project_scope: scenario.project.project_scope,
      outcome: { outcome_kind: "rejected", reason: "challenge_expired_unconsumed" } }) : new Promise<Response>(() => {});
  };
  return { fetchImpl, requests, challenges, edits, nonces };
}

/** Opens the editor. A null `active` is a tab with no Editor Session in its session storage. */
async function openStored(server: ReturnType<typeof fakeServer>, active: string | null = SESSION) {
  if (active !== null) sessionStorage.setItem(ACTIVE_KEY, active);
  const state = await openEditorWorkspace({ baseUrl: location.origin, project: scenario.project,
    chapter: scenario.chapter, profile: scenario.profile, fetchImpl: server.fetchImpl });
  requireEditorReady(state);
  trackDatabase(state.database, databases);
  return state;
}

async function readStore(store: string) {
  const database = await requestResult(indexedDB.open(scenario.journalName));
  trackDatabase(database, databases);
  return requestResult(database.transaction(store).objectStore(store).getAll()) as Promise<Record<string, unknown>[]>;
}

/** The names of the writer fields in `value`, outside a frozen request, which keeps the form that the client sent. */
function writerFields(value: unknown, path = ""): string[] {
  if (value === null || typeof value !== "object") return [];
  return Object.entries(value).flatMap(([field, item]) => field === "frozen_request_body" ? []
    : [...(["writer_generation", "partition_disposition", "resulting_writer_generation"].includes(field)
      ? [`${path}${field}`] : []), ...writerFields(item, `${path}${field}.`)]);
}

it("shows the unsent text of a version 5 Journal, and the next save sends its group as a v2 request", async () => {
  const fixture = unsentFixture as Fixture;
  await restore(fixture);
  const unsent = rows(fixture, "submission_groups").find((group) =>
    (group.settlement as { kind: string }).kind === "unsettled")!;
  const server = fakeServer([SESSION_READ, CHALLENGE, AUTHOR_EDIT]);

  const state = await openStored(server);
  expect(state.openedProjection.body).toBe("Base!?");
  // The upgrade removes the writer fields of before ADR 0107 from each row. A frozen request keeps its form.
  const stored = await Promise.all(["partitions", "intents", "payload_chains", "submission_groups", "metadata"]
    .map(readStore));
  expect(writerFields(stored)).toEqual([]);
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

it("keeps the text of the author that has the partition identifier", async () => {
  const fixture = identifierFixture as Fixture;
  await restore(fixture);
  const [partition] = rows(fixture, "partitions");
  const [group] = rows(fixture, "submission_groups");
  const server = fakeServer([SESSION_READ, CHALLENGE, AUTHOR_EDIT]);

  const state = await openStored(server);
  expect(state.openedProjection.body).toBe(`Base${String(partition!.journal_partition_id)}`);
  void submitOnePendingAuthorEdit({ workspace: state, baseUrl: location.origin, fetchImpl: server.fetchImpl,
    cryptoImpl: crypto });
  await expect.poll(() => server.edits.length).toBe(1);

  expect({ requests: server.requests, units: server.edits[0]!.body.author_edit_units }).toEqual({
    requests: [SESSION_READ, CHALLENGE, AUTHOR_EDIT],
    units: (group!.frozen_request_body as Record<string, unknown>).author_edit_units });
});

it("opens the Session of the version 5 rows in a tab that has no stored Editor Session", async () => {
  // A new tab, or a browser that started again, has no Editor Session in its session storage.
  await restore(unsentFixture as Fixture);
  const first = fakeServer([CHALLENGE, SESSION_READ]);

  const state = await openStored(first, null);
  expect({ body: state.openedProjection.body, active: sessionStorage.getItem(ACTIVE_KEY), requests: first.requests })
    .toEqual({ body: "Base!?", active: SESSION, requests: [CHALLENGE, SESSION_READ] });

  // The same tab opens its Session again, as at a reload.
  closeTrackedDatabases(databases);
  const again = fakeServer([SESSION_READ]);
  expect({ body: (await openStored(again)).openedProjection.body, requests: again.requests })
    .toEqual({ body: "Base!?", requests: [SESSION_READ] });
});

/** `fixture` with the Editor Session `OTHER_SESSION`. The rows of this Session are not read, so no digest is checked. */
function otherSession(fixture: Fixture): Fixture {
  return JSON.parse(JSON.stringify(fixture).replaceAll(SESSION, OTHER_SESSION)) as Fixture;
}

// Another open tab has the Session of the version 5 rows. A new tab and a tab that has that Session stored, as a
// duplicated tab, open a new Session.
it.each([["no stored Editor Session", null], ["the same stored Editor Session", OTHER_SESSION]] as const)(
  "opens a new Session in a tab with %s when another open tab has the Session of the version 5 rows", async (_,
    stored) => {
  const fixture = otherSession(unsentFixture as Fixture);
  await restore(fixture);
  const lock = editorSessionLock(OTHER_SESSION);
  const frame = document.createElement("iframe");
  document.body.append(frame);
  let release!: () => void;
  await new Promise<void>((held) => {
    void frame.contentWindow!.navigator.locks.request(lock, () => {
      held();
      return new Promise<void>((done) => { release = done; });
    });
  });
  const server = fakeServer([CHALLENGE, SESSION_CREATE]);
  try {
    const state = await openStored(server, stored);

    expect({ body: state.openedProjection.body, session: state.session.editor_session.editor_session_id,
      requests: server.requests }).toEqual({ body: "Base", session: NEW_SESSION, requests: [CHALLENGE, SESSION_CREATE] });
  } finally {
    release();
    frame.remove();
  }
});

it("puts the text of the Session of the version 5 rows in local recovery when that Session does not open", async () => {
  await restore(unsentFixture as Fixture);
  const server = fakeServer([CHALLENGE, SESSION_READ, SESSION_CREATE], { session: "missing" });

  const state = await openStored(server, null);

  expect({ body: state.openedProjection.body, session: state.session.editor_session.editor_session_id,
    requests: server.requests }).toEqual({ body: "Base", session: NEW_SESSION,
    requests: [CHALLENGE, SESSION_READ, CHALLENGE, SESSION_CREATE] });
  expect(await recoveryItems(state, 1)).toEqual([["正文修改", "Base!?"]]);

  // The next new tab does not read that Session again.
  closeTrackedDatabases(databases);
  sessionStorage.removeItem(ACTIVE_KEY);
  const next = fakeServer([CHALLENGE, SESSION_CREATE]);
  await openStored(next, null);
  expect(next.requests).toEqual([CHALLENGE, SESSION_CREATE]);

  // A tab that has that Session stored opens it when the Server reads it again. The recovery text does not stop it.
  closeTrackedDatabases(databases);
  const stored = fakeServer([SESSION_READ]);
  expect({ body: (await openStored(stored)).openedProjection.body, requests: stored.requests })
    .toEqual({ body: "Base!?", requests: [SESSION_READ] });
});

// Two tabs opened the version 5 Journal at the same time, and both read the marker of the unsent group. The first tab
// completed the new freeze before the second tab read the group, or after that read and before the write of the second
// tab.
it.each(["before the read", "between the read and the write"] as const)(
  "accepts the new freeze of a version 5 group that another tab completed %s of this tab", async (order) => {
  const fixture = unsentFixture as Fixture;
  await restore(fixture);
  const server = fakeServer([SESSION_READ]);
  const state = await openStored(server);
  const frozen = (await readStore("submission_groups")).find((group) => group.command_schema === V2)!;
  const recorded = rows(fixture, "submission_groups").find((group) =>
    group.journal_submission_group_id === frozen.journal_submission_group_id)!;
  // The group as the upgrade wrote it, before the new freeze of the first tab.
  const upgraded = { ...frozen, command_schema: recorded.command_schema, idempotency_key: recorded.idempotency_key,
    frozen_request_body: recorded.frozen_request_body, frozen_request_digest: recorded.frozen_request_digest };
  const put = (value: Record<string, unknown>) => requestResult(state.database
    .transaction(["metadata", "submission_groups"], "readwrite").objectStore("submission_groups").put(value));
  await requestResult(state.database.transaction("metadata", "readwrite").objectStore("metadata")
    .put({ key: "version_5_unsent_groups", value: [frozen.journal_submission_group_id] }));
  let racing = order === "between the read and the write";
  if (racing) await put(upgraded);
  // The digest of the new request runs after the read of this tab. In the second order, the first tab commits then.
  const digest = async (algorithm: AlgorithmIdentifier, data: BufferSource) => {
    if (racing) {
      racing = false;
      await put(frozen);
    }
    return crypto.subtle.digest(algorithm, data);
  };
  // The proxy gives the same object with one field changed. Each other method keeps its own object as `this`.
  const replace = <T extends object>(target: T, name: string, value: unknown) => new Proxy(target, {
    get: (object, field) => field === name ? value : (Reflect.get(object, field) as () => unknown).bind(object),
  });
  const cryptoImpl = replace(crypto, "subtle", replace(crypto.subtle, "digest", digest));

  await freezeUnsentVersion5Groups(state.database, cryptoImpl);

  expect({ group: (await readStore("submission_groups")).find((group) =>
    group.journal_submission_group_id === frozen.journal_submission_group_id), requests: server.requests })
    .toEqual({ group: frozen, requests: [SESSION_READ] });
});

it("refuses a new freeze of a version 5 group that the client sent", async () => {
  await restore(sentFixture as Fixture);
  const server = fakeServer([SESSION_READ]);
  const state = await openStored(server);
  const [sent] = await readStore("submission_groups");
  await requestResult(state.database.transaction("metadata", "readwrite").objectStore("metadata")
    .put({ key: "version_5_unsent_groups", value: [sent!.journal_submission_group_id] }));

  await expect(freezeUnsentVersion5Groups(state.database, crypto)).rejects
    .toThrow("A version 5 group changed before its new freeze");
  expect({ groups: await readStore("submission_groups"), requests: server.requests })
    .toEqual({ groups: [sent], requests: [SESSION_READ] });
});

// The POST of each group got no answer. In the first fixture the outcome Query got no answer too.
it.each([["an unresolved outcome Query", sentFixture], ["a POST in flight", inFlightFixture]] as const)(
  "shows the text of a version 5 group with %s, and the next save asks for its outcome and sends no POST", async (_,
    recorded) => {
  const fixture = recorded as Fixture;
  await restore(fixture);
  const [sent] = rows(fixture, "submission_groups");
  const [capsule] = rows(fixture, "transport_capsules");
  const server = fakeServer([SESSION_READ, `${OUTCOME}${String(sent!.idempotency_key)}`]);

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

/** Shows the local recovery panel of `state`, and returns the label and the text of each item. */
async function recoveryItems(state: Awaited<ReturnType<typeof openStored>>, count: number) {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  try {
    await act(async () => root.render(createElement(LocalRecoveryPanel, { workspace: state, refreshKey: "0",
      onContinue: async () => {} })));
    await expect.poll(() => host.querySelectorAll("[data-local-recovery-text]").length).toBe(count);
    return [...host.querySelectorAll("article")].map((item) => [item.querySelector("p")?.textContent,
      item.querySelector("[data-local-recovery-text]")?.textContent]);
  } finally {
    await act(async () => root.unmount());
    host.remove();
  }
}

it("keeps the rows of a lower writer generation of the same Editor Session, and shows their text in local recovery", async () => {
  // A takeover and a takeover back gave one Editor Session rows in two partitions. The lower generation has the sent
  // group "!" of the recording, and the higher generation has the unsent group and the record "?".
  const lower = sentFixture as Fixture;
  await restore(bothGenerations(lower, secondGeneration(unsentFixture as Fixture)));
  const [partition] = rows(lower, "partitions");
  const server = fakeServer([SESSION_READ]);

  const state = await openStored(server);

  expect(state.openedProjection.body).toBe("Base!?");
  expect(await recoveryItems(state, 1)).toEqual([["正文修改", "Base!"]]);
  expect(await partitionRows(String(partition!.journal_partition_id)))
    .toEqual([rows(lower, "intents"), rows(lower, "submission_groups")]);
  expect(server.requests).toEqual([SESSION_READ]);
});

it("shows the unsent candidate text of a lower writer generation in local recovery", async () => {
  // The lower generation has only the candidate record "!" on the candidate "Candidate". Its Chapter text is the base.
  await restore(bothGenerations(candidateFixture as Fixture, secondGeneration(unsentFixture as Fixture)));
  const server = fakeServer([SESSION_READ]);

  const state = await openStored(server);

  expect(await recoveryItems(state, 1)).toEqual([["候选修改", "Candidate!"]]);
  expect(server.requests).toEqual([SESSION_READ]);
});

it("keeps the rows of a version 5 partition with an identifier that does not match its fields, and shows their text", async () => {
  const fixture = sentFixture as Fixture;
  const [partition] = rows(fixture, "partitions");
  const changed = { ...fixture, stores: fixture.stores.map((store) => store.name !== "partitions" ? store
    : { ...store, rows: [{ ...partition!, client_session_generation: "2" }] }) };
  await restore(changed);
  const server = fakeServer([SESSION_READ]);

  const state = await openStored(server);

  expect(state.openedProjection.body).toBe("Base");
  expect(await recoveryItems(state, 1)).toEqual([["正文修改", "Base!"]]);
  expect(await partitionRows(String(partition!.journal_partition_id)))
    .toEqual([rows(fixture, "intents"), rows(fixture, "submission_groups")]);
  expect(server.requests).toEqual([SESSION_READ]);
});

it("changes the partition with rows when a higher writer generation of the same Editor Session has no rows", async () => {
  // After a takeover, the earlier writer tab read its Session as a read-only observer of generation "2", with no rows.
  const fixture = sentFixture as Fixture;
  const [partition] = rows(fixture, "partitions");
  const observer = secondGeneration({ ...fixture, stores: fixture.stores.map((store) => ({ ...store,
    rows: store.name === "partitions" ? [{ ...partition!, disposition: "read_only_observer" }]
      : store.name === "metadata" ? store.rows.filter((row) => String(row.key).startsWith("active_base:")) : [] })) });
  await restore(bothGenerations(observer, fixture));
  const server = fakeServer([SESSION_READ]);

  const state = await openStored(server);

  expect({ body: state.openedProjection.body, requests: server.requests })
    .toEqual({ body: "Base!", requests: [SESSION_READ] });
});

/** The rows of `fixture` with its sent group rejected, as the version 5 client stored the rejection. */
function rejectedGroup(fixture: Fixture): Fixture {
  return { ...fixture, stores: fixture.stores.map((store) => store.name !== "submission_groups" ? store
    : { ...store, rows: store.rows.map(({ reconciliation: _reconciliation, ...group }) => ({ ...group,
      settlement: { kind: "outcome_query_rejected_no_admission", reason: "challenge_expired_unconsumed" } })) }) };
}

// The Server has no Admission of the rejected group, so a new idempotency key cannot repeat a command.
it.each([["the outcome Query of this page", false], ["the version 5 client", true]] as const)(
  "sends a version 5 group that %s rejected again as a v2 request at the next save", async (_, stored) => {
  const fixture = sentFixture as Fixture;
  await restore(stored ? rejectedGroup(fixture) : fixture);
  const [sent] = rows(fixture, "submission_groups");
  const outcome = `${OUTCOME}${String(sent!.idempotency_key)}`;
  const server = fakeServer([SESSION_READ, outcome, CHALLENGE, AUTHOR_EDIT], { outcome: "rejected" });
  const state = await openStored(server);
  if (!stored) {
    await submitOnePendingAuthorEdit({ workspace: state, baseUrl: location.origin, fetchImpl: server.fetchImpl,
      cryptoImpl: crypto });
  }
  expect((await rebuildPendingProjection(state)).body).toBe("Base!");

  void submitOnePendingAuthorEdit({ workspace: state, baseUrl: location.origin, fetchImpl: server.fetchImpl,
    cryptoImpl: crypto });
  await expect.poll(() => server.edits.length).toBe(1);

  const { body, key } = server.edits[0]!;
  const frozen = sent!.frozen_request_body as Record<string, unknown>;
  expect({ requests: server.requests, challenge: server.challenges[0]!.command_schema, schema: body.command_schema,
    writer: "writer_generation" in body, units: body.author_edit_units, newKey: key !== sent!.idempotency_key })
    .toEqual({ requests: [SESSION_READ, ...(stored ? [] : [outcome]), CHALLENGE, AUTHOR_EDIT], challenge: V2,
      schema: V2, writer: false, units: frozen.author_edit_units, newKey: true });
});

it.each(["before the read of this tab", "between the read and the write"] as const)(
  "keeps the new freeze of a rejected version 5 group that another tab completed %s", async (order) => {
  const fixture = rejectedGroup(sentFixture as Fixture);
  await restore(fixture);
  const server = fakeServer([SESSION_READ]);
  const state = await openStored(server);
  // The new freeze of the open is the freeze of the other tab.
  const [frozen] = await readStore("submission_groups");
  const [recorded] = rows(fixture, "submission_groups");
  const upgraded = { ...frozen, command_schema: recorded!.command_schema, idempotency_key: recorded!.idempotency_key,
    frozen_request_body: recorded!.frozen_request_body, frozen_request_digest: recorded!.frozen_request_digest,
    settlement: recorded!.settlement };
  const put = (value: Record<string, unknown>) => requestResult(state.database
    .transaction("submission_groups", "readwrite").objectStore("submission_groups").put(value));
  let racing = order === "between the read and the write";
  if (racing) await put(upgraded);
  const digest = async (algorithm: AlgorithmIdentifier, data: BufferSource) => {
    if (racing) {
      racing = false;
      await put(frozen!);
    }
    return crypto.subtle.digest(algorithm, data);
  };
  const replace = <T extends object>(target: T, name: string, value: unknown) => new Proxy(target, {
    get: (object, field) => field === name ? value : (Reflect.get(object, field) as () => unknown).bind(object),
  });

  const cryptoImpl = replace(crypto, "subtle", replace(crypto.subtle, "digest", digest));
  await freezeRejectedVersion5Groups(state.database, cryptoImpl);

  expect({ groups: await readStore("submission_groups"), requests: server.requests })
    .toEqual({ groups: [frozen], requests: [SESSION_READ] });
});

// The unsent group "!" of the recording is changed, and the record "?" follows it. The groups of the partition do not
// pass the checks of the client, so its text comes from its records.
it.each([
  ["a group that has no version 5 request", (group: Record<string, unknown>) => {
    const { writer_generation: _writer, ...request } = group.frozen_request_body as Record<string, unknown>;
    return { ...group, frozen_request_body: request };
  }],
  ["a version 5 group with a settlement that the client does not know", (group: Record<string, unknown>) =>
    ({ ...group, settlement: { kind: "unknown_settlement" } })],
] as const)("keeps the rows of a version 5 partition with %s, and shows their text", async (_, change) => {
  const fixture = unsentFixture as Fixture;
  const changed = { ...fixture, stores: fixture.stores.map((store) => store.name !== "submission_groups" ? store
    : { ...store, rows: store.rows.map((group) => (group.settlement as { kind: string }).kind === "unsettled"
      ? change(group) : group) }) };
  await restore(changed);
  const [partition] = rows(changed, "partitions");
  const server = fakeServer([SESSION_READ]);

  const state = await openStored(server);

  expect(state.openedProjection.body).toBe("Base");
  expect(await recoveryItems(state, 1)).toEqual([["正文修改", "Base!?"]]);
  expect({ rows: await partitionRows(String(partition!.journal_partition_id)), requests: server.requests })
    .toEqual({ rows: [rows(changed, "intents"), rows(changed, "submission_groups")], requests: [SESSION_READ] });
});

it("keeps a version 5 partition when a stored partition has its version 6 identifier", async () => {
  const fixture = sentFixture as Fixture;
  const [partition] = rows(fixture, "partitions");
  const blocking = { ...partition!, journal_partition_id: String(partition!.journal_partition_id)
    .replace(`:${SESSION}:1:`, `:${SESSION}:`) };
  await restore({ ...fixture, stores: fixture.stores.map((store) => store.name !== "partitions" ? store
    : { ...store, rows: [partition!, blocking] }) });
  const server = fakeServer([CHALLENGE, SESSION_CREATE]);

  const state = await openStored(server, null);

  expect(await recoveryItems(state, 1)).toEqual([["正文修改", "Base!"]]);
  expect({ partitions: (await readStore("partitions")).filter((row) => row.editor_session_id === SESSION),
    rows: await partitionRows(String(partition!.journal_partition_id)), requests: server.requests })
    .toEqual({ partitions: [partition, blocking], rows: [rows(fixture, "intents"), rows(fixture, "submission_groups")],
      requests: [CHALLENGE, SESSION_CREATE] });
});
