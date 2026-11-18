import { describe, expect, it } from "vitest";

import { digestWithdrawProposal } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
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
type Command = { send: () => Promise<unknown>; read?: unknown; accepted?: Accepted; close?: () => Promise<void> };

/**
 * As the Server: one nonce and one digest for each idempotency key, and a command must carry the nonce of its key.
 * `answerFirst` is the answer to the first command that has the nonce of its key.
 */
function fakeServer(command: Pick<Command, "read" | "accepted">, answerFirst: "challenge_invalid" | "unavailable") {
  const keys: string[] = [];
  const digests = new Map<string, string>();
  const sent: Exchange[] = [];
  const correlations: string[] = [];
  let answered = false;
  const nonceOf = (key: string) => (keys.indexOf(key) === 0 ? FIRST_NONCE : SECOND_NONCE);
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if ((init?.method ?? "GET") === "GET") return jsonResponse(command.read);
    const text = String(init?.body);
    if (path.endsWith("/anti-forgery-challenges")) {
      const challenge = JSON.parse(text) as { idempotency_key: string; canonical_command_digest: unknown };
      const digest = JSON.stringify(challenge.canonical_command_digest);
      const key = challenge.idempotency_key;
      if (!keys.includes(key)) keys.push(key);
      sent.push({ step: "challenge", key: keys.indexOf(key) + 1, nonce: null });
      if ((digests.get(key) ?? digest) !== digest) {
        return jsonResponse({ schema_id: "storyos.problem.v1", code: "idempotency_binding_conflict",
          message: "The binding conflicts." }, 409);
      }
      digests.set(key, digest);
      return jsonResponse({ nonce: nonceOf(key), expires_at: "2026-11-18T12:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    const headers = new Headers(init?.headers);
    const key = headers.get("idempotency-key") ?? "";
    const nonce = headers.get("x-storyos-anti-forgery");
    sent.push({ step: "command", key: keys.indexOf(key) + 1, nonce });
    correlations.push(/"correlation_id":"([^"]+)"/.exec(text)?.[1] ?? "");
    const dead = answerFirst === "challenge_invalid" && nonce === FIRST_NONCE;
    if (nonce !== nonceOf(key) || dead) {
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "challenge_invalid",
        message: "The challenge is invalid." }, 422);
    }
    if (answerFirst === "unavailable" && !answered) {
      answered = true;
      return jsonResponse({ schema_id: "storyos.problem.v1", code: "project_store_unavailable",
        message: "The Project store is unavailable." }, 503);
    }
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
  return { close: test.close, read: { ...workspace.session, author_undo_frontier_sequence: "3" },
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
  "rename-project": (projectId, fetchImpl) => ({ send: () => renameOwnedProject({
    ...connection(projectId, fetchImpl), title: "Title", expectedProjectRevision: "4" }) }),
  "create-volume": (projectId, fetchImpl) => ({ send: () => createOwnedVolume({
    ...connection(projectId, fetchImpl), title: "Volume", expectedTreeRevision: "4" }) }),
  "update-volume": (projectId, fetchImpl) => ({ send: () => updateOwnedVolume({
    ...connection(projectId, fetchImpl), volumeId: VOLUME, title: "Volume", order: "1", expectedTreeRevision: "4" }) }),
  "delete-volume": (projectId, fetchImpl) => ({ send: () => deleteOwnedVolume({
    ...connection(projectId, fetchImpl), volumeId: VOLUME, expectedTreeRevision: "4" }) }),
  "create-chapter": (projectId, fetchImpl) => ({ send: () => createOwnedChapter({
    ...connection(projectId, fetchImpl), volumeId: VOLUME, title: "Chapter", expectedTreeRevision: "4" }) }),
  "update-chapter": (projectId, fetchImpl) => ({ send: () => updateOwnedChapter({ ...connection(projectId, fetchImpl),
    chapterId: CHAPTER, title: "Chapter", order: "1", expectedTreeRevision: "4" }) }),
  "delete-chapter": (projectId, fetchImpl) => ({ send: () => deleteOwnedChapter({
    ...connection(projectId, fetchImpl), chapterId: CHAPTER, expectedTreeRevision: "4" }) }),
  "set-current-chapter": (projectId, fetchImpl) => ({ send: () => setOwnedCurrentChapter({
    ...connection(projectId, fetchImpl), chapterId: CHAPTER, expectedCurrentChapterId: VOLUME,
    expectedTargetRevisionId: REVISION, editorSessionId: SESSION }) }),
  "take-over-writer": (projectId, fetchImpl) => ({ send: () => takeOverOwnedProjectWriter({
    ...connection(projectId, fetchImpl), editorSessionId: SESSION, observedWriterGeneration: "1" }) }),
  "export-human-readable": (projectId, fetchImpl) => ({ send: () => requestOwnedHumanReadableExport(
    connection(projectId, fetchImpl)) }),
  "undo-latest-author-action": undoCommand,
  "withdraw-block-proposal": withdrawCommand,
};

let projects = 0;
/** A module keeps its flights for the life of the page, so each test has its own Project. */
const nextProject = () => `018f0000-0000-7001-8000-0000001698${String(projects++).padStart(2, "0")}`;

/** The fake Server gets its answers from the command, and the command sends through the fake Server. */
async function connected(name: string, answerFirst: "challenge_invalid" | "unavailable") {
  const late: Pick<Command, "read" | "accepted"> = {};
  const server = fakeServer(late, answerFirst);
  const command = await COMMANDS[name]!(nextProject(), server.fetchImpl);
  late.read = command.read;
  if (command.accepted !== undefined) late.accepted = command.accepted;
  return { server, command };
}

describe.each(Object.keys(COMMANDS))("%s", (name) => {
  it("sends the next try with a new idempotency key and a new Challenge after a refused Challenge", async () => {
    const { server, command } = await connected(name, "challenge_invalid");
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
    const { server, command } = await connected(name, "unavailable");
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

it("settles a Rejection after a refused Challenge and does not send its nonce again", async () => {
  const test = await openJournalAppendTestWorkspace();
  const server = fakeServer({}, "challenge_invalid");
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
