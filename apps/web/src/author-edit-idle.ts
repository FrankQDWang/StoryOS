import { submitOnePendingAuthorEdit } from "./author-edit-submission.ts";
import {
  AUTHOR_EDIT_BATCH_IDLE_MS,
  AUTHOR_EDIT_MAX_UNITS,
  createJournalUuid,
  persistReplaceSelection,
} from "./local-edit-journal.ts";
import type {
  EditorReadyState,
  EditorWorkspace,
  InputOrigin,
  PendingEditProjection,
  ReplaceSelectionEdit,
} from "./editor-types.ts";

type TimerHandle = number | ReturnType<typeof globalThis.setTimeout>;

export interface AuthorEditIdleController {
  persist(
    edit: ReplaceSelectionEdit,
    origin: InputOrigin,
    createdAt: string,
  ): Promise<void>;
  flush(): Promise<void>;
  whenIdle(): Promise<void>;
  fail(error: unknown): void;
  setHoldSubmission(hold: boolean): void;
  close(): void;
}

export function createAuthorEditIdleController({
  workspace,
  baseUrl,
  fetchImpl = globalThis.fetch,
  cryptoImpl = globalThis.crypto,
  persistIntent = persistReplaceSelection,
  submitGroup = submitOnePendingAuthorEdit,
  afterAppliedSettlement,
  onProjection,
  onFailure,
  setTimeoutImpl = (callback, timeout) => globalThis.setTimeout(callback, timeout),
  clearTimeoutImpl = (timer) => globalThis.clearTimeout(timer),
}: {
  workspace: EditorReadyState;
  baseUrl: string;
  fetchImpl?: typeof fetch;
  cryptoImpl?: Crypto;
  persistIntent?: (
    workspace: EditorWorkspace,
    edit: ReplaceSelectionEdit,
    cryptoImpl?: Crypto,
  ) => Promise<PendingEditProjection>;
  submitGroup?: (options: {
    workspace: EditorWorkspace;
    baseUrl: string;
    fetchImpl?: typeof fetch;
    cryptoImpl?: Crypto;
    onWriterFenced?: () => void;
  }) => Promise<PendingEditProjection>;
  afterAppliedSettlement?: (workspace: EditorWorkspace) => Promise<unknown> | unknown;
  onProjection: (projection: PendingEditProjection) => void;
  onFailure: (error: unknown) => void;
  setTimeoutImpl?: (callback: () => void, timeout: number) => TimerHandle;
  clearTimeoutImpl?: (timer: TimerHandle) => void;
}): AuthorEditIdleController {
  let pendingIntentCount = workspace.pending.unsettled_intent_count;
  let undoGroupId: string | undefined;
  let lastCompletedAt: number | undefined;
  let idleTimer: TimerHandle | undefined;
  let stopped = false;
  let failed = false;
  let holdSubmission = false;
  let queuedOperations = 0;
  let queue: Promise<void> = Promise.resolve();

  const fail = (error: unknown): void => {
    if (!failed) onFailure(error);
    failed = true;
  };

  const enqueue = (operation: () => Promise<void>): Promise<void> => {
    queuedOperations += 1;
    if (queuedOperations > AUTHOR_EDIT_MAX_UNITS) {
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

  const submitPending = async (): Promise<void> => {
    clearIdle();
    if (pendingIntentCount === 0 || holdSubmission) return;
    const projection = await submitGroup({
      workspace, baseUrl, fetchImpl, cryptoImpl,
      onWriterFenced: () => fail(new Error("Editor Session is read only")),
    });
    workspace.pending = projection;
    pendingIntentCount = projection.unsettled_intent_count;
    onProjection(projection);
    undoGroupId = undefined;
    if (projection.save_state === "saved" && afterAppliedSettlement) {
      await afterAppliedSettlement(workspace);
    }
    if (projection.save_state === "needs_attention") {
      throw new Error("Author Edit requires attention");
    }
  };

  const scheduleIdle = (): void => {
    clearIdle();
    idleTimer = setTimeoutImpl(() => {
      idleTimer = undefined;
      enqueue(submitPending);
    }, AUTHOR_EDIT_BATCH_IDLE_MS);
  };

  if (pendingIntentCount > 0) scheduleIdle();

  return {
    persist(edit, origin, createdAt) {
      return enqueue(async () => {
        const hardBoundary = origin === "composition_confirmation"
          || origin === "paste" || origin === "cut";
        const completedAt = Date.parse(createdAt);
        const idleBoundary = lastCompletedAt !== undefined
          && completedAt - lastCompletedAt > AUTHOR_EDIT_BATCH_IDLE_MS;
        if (hardBoundary || idleBoundary) await submitPending();
        if (!undoGroupId) undoGroupId = createJournalUuid(cryptoImpl);
        const projection = await persistIntent(workspace, {
          ...edit, inputOrigin: origin, undoGroupId, createdAt,
        }, cryptoImpl);
        workspace.pending = projection;
        pendingIntentCount = projection.unsettled_intent_count;
        lastCompletedAt = completedAt;
        onProjection(projection);
        if (hardBoundary || pendingIntentCount >= AUTHOR_EDIT_MAX_UNITS) await submitPending();
        else scheduleIdle();
      });
    },
    flush() {
      clearIdle();
      return enqueue(submitPending);
    },
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
    },
  };
}
