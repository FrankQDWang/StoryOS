import { describe, expect, it } from "vitest";

import {
  digestCreateChapter,
  digestCreateVolume,
  digestDeleteChapter,
  digestDeleteVolume,
  digestExportHumanReadableManuscript,
  digestRejectProposalOperations,
  digestSetCurrentChapter,
  digestTakeOverProjectWriter,
  digestUndoLatestAuthorAction,
  digestUpdateChapter,
  digestUpdateProject,
  digestUpdateVolume,
  digestWithdrawProposal,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { WithdrawProposalRequest }
  from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readAcceptanceJournal } from "../../src/acceptance-journal.ts";
import { createChallengeAdmissionWait } from "../../src/challenge-admission-wait.ts";
import { createOwnedChapter } from "../../src/create-chapter.ts";
import { createOwnedVolume } from "../../src/create-volume.ts";
import { deleteOwnedChapter } from "../../src/delete-chapter.ts";
import { deleteOwnedVolume } from "../../src/delete-volume.ts";
import { requestOwnedHumanReadableExport } from "../../src/export-human-readable.ts";
import { rebuildPendingProjection } from "../../src/local-edit-journal.ts";
import { rejectDisplayedBlockProposal } from "../../src/reject-block-proposal.ts";
import { renameOwnedProject } from "../../src/rename-project.ts";
import { setOwnedCurrentChapter } from "../../src/set-current-chapter.ts";
import { takeOverOwnedProjectWriter } from "../../src/take-over-writer.ts";
import { undoOwnedLatestAuthorAction } from "../../src/undo-latest-author-action.ts";
import { updateOwnedChapter } from "../../src/update-chapter.ts";
import { updateOwnedVolume } from "../../src/update-volume.ts";
import { withdrawDisplayedBlockProposal } from "../../src/withdraw-block-proposal.ts";
import { openJournalAppendTestWorkspace } from "./local-edit-journal-append-fixture.ts";
import { jsonResponse } from "./scenario.ts";

// After `422 challenge_invalid`, the next try of each command is a new flight (#1697, ADR 0038).
const VOLUME = "018f0000-0000-7001-8000-000000169701";
const CHAPTER = "018f0000-0000-7001-8000-000000169702";
const SESSION = "018f0000-0000-7001-8000-000000169703";
const REVISION = "018f0000-0000-7001-8000-000000169704";
const PROPOSAL = "018f0000-0000-7001-8000-000000169705";
const PROPOSAL_REVISION = "018f0000-0000-7001-8000-000000169706";
const FIRST_NONCE = "a".repeat(64);
const SECOND_NONCE = "b".repeat(64);

type Exchange = { step: "challenge" | "command"; key: number; nonce: string | null };
type Accepted = (request: unknown, idempotencyKey: string) => unknown;
type Digest = (request: never, cryptoImpl?: Crypto) => Promise<unknown>;
type Command = { send: () => Promise<unknown>; digest: Digest; read?: unknown; accepted?: Accepted;
  close?: () => Promise<void> };
type Answer = "challenge_invalid" | "unavailable" | "accepted";
/** The answer to the command number `sent` (from 1), which has the idempotency key number `key`. */
type Script = (sent: number, key: number) => Answer | Promise<Answer>;

const refusesFirstKey: Script = (_sent, key) => (key === 1 ? "challenge_invalid" : "accepted");
const unavailableOnce: Script = (sent) => (sent === 1 ? "unavailable" : "accepted");
const problem = (code: string, status: number) => jsonResponse({ schema_id: "storyos.problem.v1", code,
  message: "The command is refused." }, status);

/**
 * As the Server: one nonce and one digest for each idempotency key. A command must carry the nonce of its key and the
 * digest of its Challenge.
 */
function fakeServer(command: Partial<Command>, script: Script) {
  const issued = new Map<string, { nonce: string; digest: string }>();
  const sent: Exchange[] = [];
  const correlations: string[] = [];
  let commands = 0;
  const number = (key: string) => [...issued.keys()].indexOf(key) + 1;
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if ((init?.method ?? "GET") === "GET") return jsonResponse(command.read);
    const text = String(init?.body);
    if (path.endsWith("/anti-forgery-challenges")) {
      const challenge = JSON.parse(text) as { idempotency_key: string; canonical_command_digest: unknown };
      const key = challenge.idempotency_key;
      const digest = JSON.stringify(challenge.canonical_command_digest);
      const known = issued.get(key) ?? { nonce: String.fromCharCode(97 + issued.size).repeat(64), digest };
      issued.set(key, known);
      sent.push({ step: "challenge", key: number(key), nonce: null });
      if (known.digest !== digest) return problem("idempotency_binding_conflict", 409);
      return jsonResponse({ nonce: known.nonce, expires_at: "2026-11-18T12:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    const headers = new Headers(init?.headers);
    const key = headers.get("idempotency-key") ?? "";
    const nonce = headers.get("x-storyos-anti-forgery");
    sent.push({ step: "command", key: number(key), nonce });
    correlations.push(/"correlation_id":"([^"]+)"/.exec(text)?.[1] ?? "");
    commands += 1;
    const answer = await script(commands, number(key));
    const digest = JSON.stringify(await command.digest?.(JSON.parse(text) as never, crypto));
    if (issued.get(key)?.nonce !== nonce || issued.get(key)?.digest !== digest || answer === "challenge_invalid") {
      return problem("challenge_invalid", 422);
    }
    if (answer === "unavailable") return problem("project_store_unavailable", 503);
    return jsonResponse(await (command.accepted?.(JSON.parse(text), key) ?? {}));
  };
  return { sent, correlations, fetchImpl };
}

const connection = (projectId: string, fetchImpl: typeof fetch) => (
  { baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, projectId });

async function undoCommand(_projectId: string, fetchImpl: typeof fetch): Promise<Command> {
  const test = await openJournalAppendTestWorkspace();
  const { workspace } = test;
  const projection = await rebuildPendingProjection(workspace);
  return { close: test.close, digest: digestUndoLatestAuthorAction, read: { ...workspace.session, author_undo_frontier_sequence: "3" },
    accepted: () => ({ effect: { kind: "unavailable", reason: "no_undoable_action" } }),
    send: () => undoOwnedLatestAuthorAction({ workspace, projection, baseUrl: location.origin, fetchImpl,
      cryptoImpl: crypto, isCurrent: () => true, challengeAdmission: createChallengeAdmissionWait(),
      onChallengeWait: () => {} }) };
}

async function withdrawCommand(_projectId: string, fetchImpl: typeof fetch): Promise<Command> {
  const test = await openJournalAppendTestWorkspace();
  const { workspace } = test;
  return {
    close: test.close,
    digest: digestWithdrawProposal,
    accepted: async (request, idempotencyKey) => {
      const withdrawal = request as WithdrawProposalRequest;
      return { schema_id: "storyos.command.withdraw-proposal.response.v1",
        project_scope: workspace.partition.project_scope,
        correlation_id: withdrawal.withdraw_proposal_input.correlation_id, effect: { kind: "resolved" },
        receipt: { proposal_id: PROPOSAL, proposal_revision_id: PROPOSAL_REVISION, result: "resolved",
          idempotency_key: idempotencyKey, command_digest: await digestWithdrawProposal(withdrawal, crypto) } };
    },
    send: () => withdrawDisplayedBlockProposal({ baseUrl: location.origin, fetchImpl, cryptoImpl: crypto, workspace,
      proposalId: PROPOSAL, proposalRevisionId: PROPOSAL_REVISION, targetRevisionId: REVISION }),
  };
}

const COMMANDS: Record<string, (projectId: string, fetchImpl: typeof fetch) => Command | Promise<Command>> = {
  "rename-project": (projectId, fetchImpl) => ({ digest: digestUpdateProject, send: () => renameOwnedProject({
    ...connection(projectId, fetchImpl), title: "Title", expectedProjectRevision: "4" }) }),
  "create-volume": (projectId, fetchImpl) => ({ digest: digestCreateVolume, send: () => createOwnedVolume({
    ...connection(projectId, fetchImpl), title: "Volume", expectedTreeRevision: "4" }) }),
  "update-volume": (projectId, fetchImpl) => ({ digest: digestUpdateVolume, send: () => updateOwnedVolume({
    ...connection(projectId, fetchImpl), volumeId: VOLUME, title: "Volume", order: "1", expectedTreeRevision: "4" }) }),
  "delete-volume": (projectId, fetchImpl) => ({ digest: digestDeleteVolume, send: () => deleteOwnedVolume({
    ...connection(projectId, fetchImpl), volumeId: VOLUME, expectedTreeRevision: "4" }) }),
  "create-chapter": (projectId, fetchImpl) => ({ digest: digestCreateChapter, send: () => createOwnedChapter({
    ...connection(projectId, fetchImpl), volumeId: VOLUME, title: "Chapter", expectedTreeRevision: "4" }) }),
  "update-chapter": (projectId, fetchImpl) => ({ digest: digestUpdateChapter, send: () => updateOwnedChapter({ ...connection(projectId, fetchImpl),
    chapterId: CHAPTER, title: "Chapter", order: "1", expectedTreeRevision: "4" }) }),
  "delete-chapter": (projectId, fetchImpl) => ({ digest: digestDeleteChapter, send: () => deleteOwnedChapter({
    ...connection(projectId, fetchImpl), chapterId: CHAPTER, expectedTreeRevision: "4" }) }),
  "set-current-chapter": (projectId, fetchImpl) => ({ digest: digestSetCurrentChapter, send: () => setOwnedCurrentChapter({
    ...connection(projectId, fetchImpl), chapterId: CHAPTER, expectedCurrentChapterId: VOLUME,
    expectedTargetRevisionId: REVISION, editorSessionId: SESSION }) }),
  "take-over-writer": (projectId, fetchImpl) => ({ digest: digestTakeOverProjectWriter, send: () => takeOverOwnedProjectWriter({
    ...connection(projectId, fetchImpl), editorSessionId: SESSION, observedWriterGeneration: "1" }) }),
  "export-human-readable": (projectId, fetchImpl) => ({ digest: digestExportHumanReadableManuscript, send: () => requestOwnedHumanReadableExport(
    connection(projectId, fetchImpl)) }),
  "undo-latest-author-action": undoCommand,
  "withdraw-block-proposal": withdrawCommand,
};

let projects = 0;
/** A module keeps its flights for the life of the page, so each test has its own Project. */
const nextProject = () => `018f0000-0000-7001-8000-0000001698${String(projects++).padStart(2, "0")}`;

/** The fake Server gets its answers from the command, and the command sends through the fake Server. */
async function connected(name: string, script: Script) {
  const late: Partial<Command> = {};
  const server = fakeServer(late, script);
  const command = await COMMANDS[name]!(nextProject(), server.fetchImpl);
  Object.assign(late, command);
  return { server, command };
}

describe.each(Object.keys(COMMANDS))("%s", (name) => {
  it("sends the next try with a new idempotency key and a new Challenge after a refused Challenge", async () => {
    const { server, command } = await connected(name, refusesFirstKey);
    try {
      await expect(command.send()).rejects.toMatchObject({ status: 422 });
      await command.send();
      expect(server.sent).toEqual([
        { step: "challenge", key: 1, nonce: null },
        { step: "command", key: 1, nonce: FIRST_NONCE },
        { step: "challenge", key: 2, nonce: null },
        { step: "command", key: 2, nonce: SECOND_NONCE },
      ]);
      expect(new Set(server.correlations).size).toBe(2);
    } finally {
      await command.close?.();
    }
  });

  it("keeps the idempotency key and the nonce after another error", async () => {
    const { server, command } = await connected(name, unavailableOnce);
    try {
      // A module that repeats the send by itself settles in the first call. The Withdrawal repeats on the next call.
      await command.send().catch(() => undefined);
      if (server.sent.length === 2) await command.send();
      expect(server.sent).toEqual([
        { step: "challenge", key: 1, nonce: null },
        { step: "command", key: 1, nonce: FIRST_NONCE },
        { step: "command", key: 1, nonce: FIRST_NONCE },
      ]);
    } finally {
      await command.close?.();
    }
  });
});

const FLIGHT_TABLE_MODULES = Object.keys(COMMANDS)
  .filter((name) => name !== "undo-latest-author-action" && name !== "withdraw-block-proposal");

it.each(FLIGHT_TABLE_MODULES)("%s keeps a newer flight when a refusal of the old flight comes late", async (name) => {
  let refuseFirst = () => {};
  let refuseSecond = () => {};
  const first = new Promise<Answer>((resolve) => { refuseFirst = () => resolve("challenge_invalid"); });
  const second = new Promise<Answer>((resolve) => { refuseSecond = () => resolve("challenge_invalid"); });
  const answers: (Answer | Promise<Answer>)[] = [first, second, "unavailable", "unavailable", "accepted"];
  const { server, command } = await connected(name, (sent) => answers[sent - 1]!);
  const commands = () => server.sent.filter(({ step }) => step === "command").length;

  const early = command.send();
  await expect.poll(commands).toBe(1);
  const late = command.send();
  await expect.poll(commands).toBe(2);
  refuseFirst();
  await expect(early).rejects.toMatchObject({ status: 422 });
  await expect(command.send()).rejects.toMatchObject({ status: 503 });
  refuseSecond();
  await expect(late).rejects.toMatchObject({ status: 422 });
  await command.send();

  expect(server.sent).toEqual([
    { step: "challenge", key: 1, nonce: null },
    { step: "command", key: 1, nonce: FIRST_NONCE },
    { step: "command", key: 1, nonce: FIRST_NONCE },
    { step: "challenge", key: 2, nonce: null },
    { step: "command", key: 2, nonce: SECOND_NONCE },
    { step: "command", key: 2, nonce: SECOND_NONCE },
    { step: "command", key: 2, nonce: SECOND_NONCE },
  ]);
});

it("settles a Rejection after a refused Challenge and does not send its nonce again", async () => {
  const test = await openJournalAppendTestWorkspace();
  const server = fakeServer({ digest: digestRejectProposalOperations }, refusesFirstKey);
  const reject = () => rejectDisplayedBlockProposal({ baseUrl: location.origin, fetchImpl: server.fetchImpl,
    cryptoImpl: crypto, workspace: test.workspace, proposalId: PROPOSAL, operationId: CHAPTER,
    proposalRevisionId: PROPOSAL_REVISION, targetRevisionId: REVISION });
  try {
    await expect(reject()).rejects.toMatchObject({ status: 422 });
    await expect(reject()).rejects.toThrow("A prior Rejection decision is unresolved");
    const { groups } = await readAcceptanceJournal(test.workspace);
    expect(groups.map((group) => (group.settlement as { kind: string }).kind)).toEqual(["pre_admission_problem"]);
    expect(server.sent).toEqual([
      { step: "challenge", key: 1, nonce: null },
      { step: "command", key: 1, nonce: FIRST_NONCE },
    ]);
  } finally {
    await test.close();
  }
});
