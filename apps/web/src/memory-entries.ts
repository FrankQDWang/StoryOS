import { useEffect, useRef, useState } from "react";

import { listMemoryEntries } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { MemoryEntryInspect, ProjectScope }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { deleteMemoryEntryOnce } from "./delete-memory-entry.ts";
import type { MemoryDeleteFlight } from "./delete-memory-entry.ts";
import { sameScope } from "./project-instruction.ts";

/** The step of the delete control of one entry, and the sentence under the entry after a delete. */
export type DeleteStep = "confirm" | "sending" | "changed" | "failed";

/** The current Memory Entries of the settings page and the delete steps of each entry. */
export type MemoryEntries = {
  /** `undefined` while the first read or a read after "重新读取" is open. */
  entries: readonly MemoryEntryInspect[] | "failed" | undefined;
  steps: ReadonlyMap<string, DeleteStep>;
  readAgain: () => void;
  setStep: (memoryEntryId: string, step: DeleteStep | undefined) => void;
  /** Sends the delete of `entry`. */
  confirmDelete: (entry: MemoryEntryInspect) => Promise<void>;
};

/**
 * One read of `listMemoryEntries` for the whole settings page (ADR 0094 section 9), so that the sections "记忆" and
 * "人物" show the same entries. After `entry_changed`, the page reads the list again.
 */
export function useMemoryEntries({ scope, baseUrl, fetchImpl, cryptoImpl }: {
  scope: ProjectScope;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}): MemoryEntries {
  const [entries, setEntries] = useState<readonly MemoryEntryInspect[] | "failed">();
  const [reads, setReads] = useState(0);
  // Each entry has its own step and its own delete intent, so a delete of one entry does not change another entry.
  const [steps, setSteps] = useState<ReadonlyMap<string, DeleteStep>>(new Map());
  const flights = useRef(new Map<string, MemoryDeleteFlight>());
  // The entries that a delete removed. A read that started before the delete must not show them again.
  const removed = useRef(new Set<string>());
  const { owner_user_id: ownerUserId, project_id: projectId } = scope;
  useEffect(() => {
    let current = true;
    listMemoryEntries({ baseUrl, fetchImpl, projectId })
      .then((read) => sameScope(read.project_scope, { owner_user_id: ownerUserId, project_id: projectId })
        ? read.entries : "failed" as const, () => "failed" as const)
      .then((read) => {
        if (current) {
          setEntries(read === "failed" ? read : read.filter((item) => !removed.current.has(item.memory_entry_id)));
        }
      });
    return () => { current = false; };
  }, [baseUrl, fetchImpl, ownerUserId, projectId, reads]);
  const setStep = (memoryEntryId: string, step: DeleteStep | undefined) => setSteps((shown) => {
    const next = new Map(shown);
    if (step === undefined) next.delete(memoryEntryId);
    else next.set(memoryEntryId, step);
    return next;
  });
  const confirmDelete = async (entry: MemoryEntryInspect) => {
    const memoryEntryId = entry.memory_entry_id;
    setStep(memoryEntryId, "sending");
    const result = await deleteMemoryEntryOnce({ baseUrl, fetchImpl, cryptoImpl, scope, memoryEntryId,
      version: entry.version, flight: flights.current.get(memoryEntryId) });
    if (result.flight === undefined) flights.current.delete(memoryEntryId);
    else flights.current.set(memoryEntryId, result.flight);
    if (result.outcome === "deleted") {
      removed.current.add(memoryEntryId);
      setStep(memoryEntryId, undefined);
      // A delete of the part of the author of a character ends the intent result that names it (ADR 0101 section 4).
      const named = entry.entry_type === "character" && entry.origin === "author_request";
      setEntries((shown) => shown === "failed" || shown === undefined
        ? shown : shown.filter((item) => item.memory_entry_id !== memoryEntryId).map((item) => {
          if (!named || item.title !== entry.title) return item;
          const next = { ...item };
          delete next.intent;
          return next;
        }));
      return;
    }
    setStep(memoryEntryId, result.outcome);
    if (result.outcome === "changed") setReads((count) => count + 1);
  };
  const readAgain = () => {
    setEntries(undefined);
    setReads((count) => count + 1);
  };
  return { entries, steps, readAgain, setStep, confirmDelete };
}
