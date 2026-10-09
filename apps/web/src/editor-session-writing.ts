import { submitOnePendingAuthorEdit } from "./author-edit-submission.ts";
import { createChallengeAdmissionWait, type TimerHandle } from "./challenge-admission-wait.ts";
import { collectEligibleJournalPayload } from "./journal-payload-collection.ts";
import {
  AUTHOR_EDIT_BATCH_IDLE_MS,
  AUTHOR_EDIT_MAX_UNITS,
  candidateTextsFromJournal,
  createJournalUuid,
  persistCandidateSelection,
  persistStructuredSelection,
  persistJoinBlocks,
  persistMoveBlock,
  persistReplaceSelection,
  persistRetypeBlock,
  persistSplitBlock,
  persistContiguousReplacement,
  readJournalSnapshot,
  rebuildPendingProjection,
} from "./local-edit-journal.ts";
import type { CandidateSelectionEdit } from "./local-edit-journal.ts";
import type {
  EditorReadyState,
  EditorWorkspace,
  InputOrigin,
  PendingEditProjection,
  ReplaceSelectionEdit,
} from "./editor-types.ts";
import { flattenChapterBody, type CapturedManuscriptEdit, type ManuscriptParagraph } from "./manuscript-doc.ts";

import type { StructuredSelectionEdit } from "./structured-edit-capture.ts";

export type CapturedEdit = ReplaceSelectionEdit | (CapturedManuscriptEdit & { expectedProposalHeads?: string[] })
  | CandidateSelectionEdit | StructuredSelectionEdit;

/** The text of one Proposal operation that the controller shows, and the Revisions that this text continues. */
export interface CandidateText {
  readonly revisions: readonly string[];
  readonly text: string;
}

export interface EditorWritingSnapshot {
  readonly projection: PendingEditProjection;
  /** True while the projection shows input that is not in the Journal yet. */
  readonly local: boolean;
  /** Candidate texts keyed `proposal:operation`. */
  readonly candidates: ReadonlyMap<string, CandidateText>;
  /** Counts saved settlements, so a view can read the Proposal facts again. */
  readonly settlements: number;
}

/** The only owner of the Pending Edit Projection of one current-writer Editor Session (ADR 0046). */
export interface EditorSessionWritingController {
  /** Accepts one captured edit. `blocks` is the manuscript that the editor shows after the edit. */
  capture(edit: CapturedEdit, origin: InputOrigin, blocks?: readonly ManuscriptParagraph[]): void;
  /** Shows input that is not captured yet, or a wait, as `saving`. */
  showSaving(blocks?: readonly ManuscriptParagraph[]): void;
  snapshot(): EditorWritingSnapshot;
  subscribe(listener: () => void): () => void;
  /** Reads the Journal and installs the result if no input was captured and nothing was installed during the read. */
  refresh(): Promise<PendingEditProjection>;
  flush(): Promise<void>;
  whenIdle(): Promise<void>;
  /** True while captured input waits outside the Journal for an unknown candidate outcome. */
  holdsInput(): boolean;
  canAcceptInput(hardBoundary: boolean, candidate?: { proposalId: string; operationId: string }): boolean;
  setHoldSubmission(hold: boolean): void;
  fail(error: unknown): void;
  close(): void;
}

type Captured = { edit: CapturedEdit; origin: InputOrigin; createdAt: string };

const HARD_INPUT: readonly InputOrigin[] = ["composition_confirmation", "paste", "cut", "drop"];
const STRUCTURAL_INPUT: readonly InputOrigin[] = ["split_block", "join_blocks", "move_block", "retype_block"];

function candidateKey(edit: CapturedEdit): string | undefined {
  return "kind" in edit && edit.kind === "candidate_selection"
    ? `${edit.target.proposal_id}:${edit.target.operation_id}` : undefined;
}

function persistEdit(workspace: EditorWorkspace, edit: CapturedEdit,
  fields: { inputOrigin: InputOrigin; undoGroupId: string; createdAt: string }, cryptoImpl: Crypto) {
  if (!("kind" in edit)) return persistReplaceSelection(workspace, { ...edit, ...fields }, cryptoImpl);
  switch (edit.kind) {
    case "structured_selection": return persistStructuredSelection(workspace, { ...edit, ...fields }, cryptoImpl);
    case "candidate_selection": return persistCandidateSelection(workspace, { ...edit, ...fields }, cryptoImpl);
    case "split_block": return persistSplitBlock(workspace, { manuscript_block_id: edit.manuscript_block_id,
      offset: edit.offset, new_manuscript_block_id: edit.new_manuscript_block_id, resultingBody: edit.resultingBody,
      resultingBlocks: edit.resultingBlocks, ...fields }, cryptoImpl);
    case "join_blocks": return persistJoinBlocks(workspace, { left_manuscript_block_id: edit.left_manuscript_block_id,
      right_manuscript_block_id: edit.right_manuscript_block_id, caret: edit.caret, resultingBody: edit.resultingBody,
      resultingBlocks: edit.resultingBlocks, ...fields }, cryptoImpl);
    case "move_block": return persistMoveBlock(workspace, { manuscript_block_id: edit.manuscript_block_id,
      to_index: edit.to_index, resultingBody: edit.resultingBody, resultingBlocks: edit.resultingBlocks, ...fields },
    cryptoImpl);
    case "retype_block": return persistRetypeBlock(workspace, { manuscript_block_id: edit.manuscript_block_id,
      block_kind: edit.block_kind, resultingBody: edit.resultingBody, resultingBlocks: edit.resultingBlocks, ...fields },
    cryptoImpl);
    case "contiguous_replacement": return persistContiguousReplacement(workspace, { primitives: edit.primitives,
      from: edit.from, to: edit.to, resultingBody: edit.resultingBody, resultingBlocks: edit.resultingBlocks, ...fields },
    cryptoImpl);
    case "replace_block_selection": return persistReplaceSelection(workspace, { from: edit.from, to: edit.to,
      text: edit.text, resultingBody: edit.resultingBody, manuscript_block_id: edit.manuscript_block_id,
      ...(edit.expectedProposalHeads === undefined ? {} : { expectedProposalHeads: edit.expectedProposalHeads }),
      ...fields }, cryptoImpl);
  }
}

/** The settlement of the latest Journal group for one Proposal operation. */
async function candidateOutcome(workspace: EditorWorkspace, key: string)
  : Promise<{ kind: "revised"; from: string; to: string } | { kind: "unknown" } | { kind: "other" }> {
  const groups = (await readJournalSnapshot(workspace)).groups.filter((group) => {
    const target = group.frozen_request_body.proposal_target;
    return target != null && `${target.proposal_id}:${target.operation_id}` === key;
  });
  const group = groups.sort((left, right) =>
    left.covered_sequence_range.first - right.covered_sequence_range.first).at(-1);
  if (group === undefined || group.settlement.kind === "unsettled") return { kind: "unknown" };
  return group.settlement.kind === "zero_authority_receipt_settled" && group.settlement.effect.kind === "proposal_revised"
    ? { kind: "revised", from: group.frozen_request_body.proposal_target!.revision_id,
      to: group.settlement.effect.proposal_revision_id }
    : { kind: "other" };
}

export function createEditorSessionWritingController({
  workspace,
  baseUrl,
  fetchImpl = globalThis.fetch,
  cryptoImpl = globalThis.crypto,
  submitGroup = submitOnePendingAuthorEdit,
  onFailure,
  now = Date.now,
  setTimeoutImpl = (callback, timeout) => globalThis.setTimeout(callback, timeout),
  clearTimeoutImpl = (timer) => globalThis.clearTimeout(timer),
}: {
  workspace: EditorReadyState;
  baseUrl: string;
  fetchImpl?: typeof fetch;
  cryptoImpl?: Crypto;
  submitGroup?: typeof submitOnePendingAuthorEdit;
  onFailure: (error: unknown) => void;
  now?: () => number;
  setTimeoutImpl?: (callback: () => void, timeout: number) => TimerHandle;
  clearTimeoutImpl?: (timer: TimerHandle) => void;
}): EditorSessionWritingController {
  let pendingIntentCount = workspace.pending.author_edit_unsettled_intent_count ?? workspace.pending.unsettled_intent_count;
  let pendingTarget = pendingIntentCount > 0 ? "recovered" : undefined;
  let submissionClosed = pendingIntentCount > 0;
  // Captured input that is not in the Journal yet.
  let unjournaled = 0;
  let captured = 0;
  let installs = 0;
  let undoGroupId: string | undefined;
  let lastCompletedAt: number | undefined;
  let idleTimer: TimerHandle | undefined;
  const challengeAdmission = createChallengeAdmissionWait({ setTimeoutImpl, clearTimeoutImpl });
  let stopped = false;
  let failed = false;
  // After a failed edit, later captured input stays visible and held. The Journal refuses appends (ADR 0046).
  let attention = false;
  let holdSubmission = false;
  let queuedOperations = 0;
  let queue: Promise<void> = Promise.resolve();
  // The candidate whose latest group has an unknown outcome. Input waits in `held` until it settles.
  let unknownCandidate: string | undefined;
  let openCandidate: string | undefined;
  const held: Captured[] = [];
  const successors = new Map<string, string>();
  const listeners = new Set<() => void>();
  let state: EditorWritingSnapshot = { projection: workspace.pending, local: false, candidates: new Map(), settlements: 0 };

  const publish = (next: Partial<EditorWritingSnapshot>): void => {
    state = { ...state, ...next };
    for (const listener of [...listeners]) listener();
  };

  const install = (projection: PendingEditProjection, settled = false): void => {
    workspace.pending = projection;
    installs += 1;
    const local = unjournaled > 0;
    // Newer captured input stays visible over an older Journal projection.
    publish({
      projection: local && projection.save_state !== "needs_attention"
        ? { ...projection, body: state.projection.body, blocks: state.projection.blocks, save_state: "saving" }
        : projection,
      local,
      ...(settled ? { settlements: state.settlements + 1 } : {}),
    });
  };

  /** Shows input that is not in the Journal yet. `blocks` is the manuscript that the editor shows. */
  const showLocal = (blocks: readonly ManuscriptParagraph[] | undefined, newIntents: number): void => {
    if (state.projection.save_state === "needs_attention") return;
    publish({ local: true, projection: { ...state.projection, save_state: "saving",
      unsettled_intent_count: state.projection.unsettled_intent_count + newIntents,
      ...(blocks === undefined ? {} : { body: flattenChapterBody(blocks),
        blocks: blocks.map((block) => ({ manuscript_block_id: block.manuscript_block_id,
          block_kind: block.block_kind === "heading" ? "heading" as const : "paragraph" as const, text: block.text })) }) } });
  };

  const setCandidate = (key: string, revisions: readonly string[], text: string): void => {
    const current = state.candidates.get(key);
    const merged = [...new Set([...(current?.revisions ?? []), ...revisions])];
    publish({ candidates: new Map(state.candidates).set(key, { revisions: merged, text }) });
  };

  const fail = (error: unknown): void => {
    if (!failed) onFailure(error);
    failed = true;
    submissionClosed = true;
  };

  const enqueue = (operation: () => Promise<void>): Promise<void> => {
    queuedOperations += 1;
    if (queuedOperations + held.length > AUTHOR_EDIT_MAX_UNITS) {
      queuedOperations -= 1;
      fail(new Error("Manual input queue limit failed"));
      return queue;
    }
    queue = queue.then(async () => {
      if (stopped || failed) return;
      await operation();
    }).catch(fail).finally(() => { queuedOperations -= 1; });
    return queue;
  };

  const clearIdle = (): void => {
    if (idleTimer !== undefined) clearTimeoutImpl(idleTimer);
    idleTimer = undefined;
  };

  const enterAttention = (): void => {
    if (attention) return;
    attention = true;
    unknownCandidate = undefined;
    submissionClosed = true;
    onFailure(new Error("Author Edit requires attention"));
  };

  const submitPending = async (): Promise<void> => {
    clearIdle();
    if (pendingIntentCount === 0 || holdSubmission || attention
      || workspace.pending.save_state === "needs_attention") return;
    submissionClosed = true;
    const submittedCandidate = openCandidate;
    // The same frozen group retries after a Challenge rate limit.
    const projection = await challengeAdmission.retry(() => submitGroup({
      workspace, baseUrl, fetchImpl, cryptoImpl,
      onWriterFenced: () => fail(new Error("Editor Session is read only")),
    }));
    if (projection === undefined) return;
    pendingIntentCount = projection.author_edit_unsettled_intent_count ?? projection.unsettled_intent_count;
    if (pendingIntentCount === 0) pendingTarget = undefined;
    if (projection.save_state === "saved" && pendingIntentCount === 0) submissionClosed = false;
    undoGroupId = undefined;
    if (submittedCandidate !== undefined) {
      const outcome = await candidateOutcome(workspace, submittedCandidate);
      unknownCandidate = outcome.kind === "unknown" ? submittedCandidate : undefined;
      if (outcome.kind === "revised") {
        successors.set(`${submittedCandidate}:${outcome.from}`, outcome.to);
        const shown = state.candidates.get(submittedCandidate);
        if (shown !== undefined) setCandidate(submittedCandidate, [outcome.to], shown.text);
      }
      if (outcome.kind === "other") {
        install(projection);
        enterAttention();
        return;
      }
    }
    if (projection.save_state === "saved") await collectEligibleJournalPayload(workspace);
    install(projection, projection.save_state === "saved");
    if (projection.save_state === "needs_attention") {
      enterAttention();
      return;
    }
    // A still-unknown Outcome Query leaves the group saving. Query the same
    // identity again. Do not obtain a new challenge or send a new command.
    if (projection.save_state === "saving" && pendingIntentCount > 0 && !holdSubmission) scheduleIdle();
  };

  const rebind = (edit: CapturedEdit): CapturedEdit => {
    if (!("kind" in edit) || edit.kind !== "candidate_selection") return edit;
    const key = candidateKey(edit)!;
    let revision = edit.target.revision_id;
    let heads = edit.expectedProposalHeads;
    for (let next = successors.get(`${key}:${revision}`); next !== undefined;
      next = successors.get(`${key}:${revision}`)) {
      const earlier = revision;
      heads = heads.map((head) => head === earlier ? next! : head);
      revision = next;
    }
    return revision === edit.target.revision_id ? edit
      : { ...edit, target: { ...edit.target, revision_id: revision }, expectedProposalHeads: heads };
  };

  /** Journals one captured edit. Returns false when the edit must stay held. */
  const append = async ({ edit: capturedEdit, origin, createdAt }: Captured): Promise<boolean> => {
    if (attention) return false;
    const edit = rebind(capturedEdit);
    const hardBoundary = HARD_INPUT.includes(origin) || STRUCTURAL_INPUT.includes(origin)
      || ("kind" in edit && edit.kind === "structured_selection");
    const completedAt = Date.parse(createdAt);
    const target = "kind" in edit && edit.kind === "candidate_selection" ? JSON.stringify(edit.target) : "authoritative";
    const idleBoundary = lastCompletedAt !== undefined && completedAt - lastCompletedAt > AUTHOR_EDIT_BATCH_IDLE_MS;
    if (hardBoundary || idleBoundary || pendingTarget !== undefined && pendingTarget !== target) await submitPending();
    if (unknownCandidate !== undefined || attention) return false;
    undoGroupId ??= createJournalUuid(cryptoImpl);
    const projection = await persistEdit(workspace, edit, { inputOrigin: origin, undoGroupId, createdAt }, cryptoImpl);
    unjournaled -= 1;
    pendingIntentCount = projection.author_edit_unsettled_intent_count ?? projection.unsettled_intent_count;
    pendingTarget = target;
    openCandidate = candidateKey(edit);
    lastCompletedAt = completedAt;
    install(projection);
    if (hardBoundary || pendingIntentCount >= AUTHOR_EDIT_MAX_UNITS) await submitPending();
    else scheduleIdle();
    return true;
  };

  const releaseHeld = async (): Promise<void> => {
    while (held.length > 0 && unknownCandidate === undefined && !attention && !stopped && !failed) {
      if (!await append(held[0]!)) return;
      held.shift();
    }
  };

  const settle = async (): Promise<void> => {
    await submitPending();
    await releaseHeld();
  };

  const scheduleIdle = (): void => {
    clearIdle();
    idleTimer = setTimeoutImpl(() => {
      idleTimer = undefined;
      submissionClosed = true;
      enqueue(settle);
    }, AUTHOR_EDIT_BATCH_IDLE_MS);
  };

  if (pendingIntentCount > 0) scheduleIdle();
  void candidateTextsFromJournal(workspace).then((texts) => {
    if (stopped) return;
    const candidates = new Map<string, CandidateText>(texts);
    for (const [key, value] of state.candidates) candidates.set(key, value);
    publish({ candidates });
  }).catch(fail);

  return {
    capture(edit, origin, blocks) {
      captured += 1;
      unjournaled += 1;
      if (HARD_INPUT.includes(origin)) submissionClosed = true;
      const key = candidateKey(edit);
      if (key !== undefined && "kind" in edit && edit.kind === "candidate_selection") {
        setCandidate(key, [edit.target.revision_id], edit.resultingBody);
      }
      showLocal(blocks, blocks === undefined ? 1 : 0);
      const item = { edit, origin, createdAt: new Date(now()).toISOString() };
      void enqueue(async () => {
        if (held.length === 0 && await append(item)) return;
        held.push(item);
        await releaseHeld();
      });
    },
    showSaving: (blocks) => { showLocal(blocks, 0); },
    snapshot: () => state,
    subscribe(listener) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    async refresh() {
      const capturedBefore = captured;
      const installsBefore = installs;
      const projection = await rebuildPendingProjection(workspace);
      // A read that started before newer input or a newer install is stale (ADR 0046).
      if (!stopped && captured === capturedBefore && installs === installsBefore && unjournaled === 0) install(projection);
      return state.projection;
    },
    flush() {
      clearIdle();
      if (pendingIntentCount > 0 || unjournaled > 0) submissionClosed = true;
      return enqueue(settle);
    },
    canAcceptInput(hardBoundary, candidate) {
      if (stopped || failed || attention
        || (workspace.pending.author_edit_unsettled_intent_count ?? workspace.pending.unsettled_intent_count)
          !== workspace.pending.unsettled_intent_count) return false;
      // Input in the candidate of the open or in-progress group continues it (ADR 0046).
      if (candidate !== undefined && openCandidate === `${candidate.proposalId}:${candidate.operationId}`) return true;
      return !submissionClosed && (!hardBoundary || pendingIntentCount === 0 && unjournaled === 0);
    },
    holdsInput: () => held.length > 0,
    async whenIdle() {
      await Promise.resolve();
      await queue;
    },
    fail,
    setHoldSubmission(hold) {
      holdSubmission = hold;
      if (hold) clearIdle();
      else if (pendingIntentCount > 0) scheduleIdle();
    },
    close() {
      stopped = true;
      clearIdle();
      challengeAdmission.cancel();
      listeners.clear();
    },
  };
}
