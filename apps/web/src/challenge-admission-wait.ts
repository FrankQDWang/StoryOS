import { StoryOSProtocolError }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";

export type TimerHandle = number | ReturnType<typeof globalThis.setTimeout>;

export interface ChallengeAdmissionTimers {
  setTimeoutImpl?: (callback: () => void, timeout: number) => TimerHandle;
  clearTimeoutImpl?: (timer: TimerHandle) => void;
}

/** Repeats one command request while the Server refuses its Command Challenge with a rate limit. */
export interface ChallengeAdmissionWait {
  /**
   * Returns the result of a completed request, also when `cancel` occurred during that request.
   * Returns `undefined` when `cancel` occurs before or during a `Retry-After` wait. Other errors propagate.
   */
  retry<Result>(request: () => Promise<Result>, onWait?: () => void): Promise<Result | undefined>;
  /** Ends a current `Retry-After` wait at once and prevents later waits and attempts. */
  cancel(): void;
}

export function createChallengeAdmissionWait({
  setTimeoutImpl = (callback, timeout) => globalThis.setTimeout(callback, timeout),
  clearTimeoutImpl = (timer) => globalThis.clearTimeout(timer),
}: ChallengeAdmissionTimers = {}): ChallengeAdmissionWait {
  let cancelled = false;
  let pending: { timer: TimerHandle; resolve: () => void } | undefined;
  return {
    async retry(request, onWait) {
      while (!cancelled) {
        try {
          return await request();
        } catch (error) {
          // A Challenge rate limit only delays the command. The same request and idempotency key retry.
          if (!(error instanceof StoryOSProtocolError && error.status === 429)) throw error;
          if (cancelled) return undefined;
          onWait?.();
          const retryAfterSeconds = Math.max(1, error.retryAfterSeconds ?? 1);
          await new Promise<void>((resolve) => {
            pending = { timer: setTimeoutImpl(resolve, retryAfterSeconds * 1000), resolve };
          });
          pending = undefined;
        }
      }
      return undefined;
    },
    cancel() {
      cancelled = true;
      if (pending === undefined) return;
      clearTimeoutImpl(pending.timer);
      pending.resolve();
    },
  };
}
