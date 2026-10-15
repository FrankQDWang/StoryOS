import { useSyncExternalStore } from "react";
import type { Transaction } from "@tiptap/pm/state";
import { ReplaceStep } from "@tiptap/pm/transform";
import type { ProjectScope, SelectionReference }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readManuscriptParagraphs } from "./manuscript-doc.ts";
import {
  candidateRange, candidateTextReference, chapterTextReference, formatReferenceLabel, mapReferenceRange,
  mergeReferences, referenceLabel, referenceRange, SELECTION_REFERENCE_LIMIT, type CandidateTarget,
  type CandidateTextReference, type ChapterOrder, type ChapterTextReference, type ReferenceLabel,
} from "./selection-reference.ts";

/** One reference that the author added to the next message, with its label. */
export type DraftReference = {
  key: string;
  reference: SelectionReference;
  /** The target of a candidate reference, which becomes a Chapter text reference when the author accepts it. */
  candidate?: CandidateTarget;
  label: ReferenceLabel;
  /** The range of a Chapter text reference moved with author edits that its base Revision does not have yet. */
  moved: boolean;
};

/** The clipboard type of the references that a copy from the editor gives to the composer. */
export const REFERENCE_CLIPBOARD_TYPE = "application/x-storyos-selection-reference+json";

type Draft = { entries: DraftReference[]; run: SelectionReference[] };

const EMPTY: Draft = { entries: [], run: [] };
const listeners = new Set<() => void>();
const cache = new Map<string, Draft>();

function storageKey(scope: ProjectScope): string {
  return `selection_references:${scope.owner_user_id}:${scope.project_id}`;
}

function read(scope: ProjectScope): Draft {
  const key = storageKey(scope);
  const cached = cache.get(key);
  if (cached !== undefined) return cached;
  let draft = EMPTY;
  try {
    const value = JSON.parse(globalThis.sessionStorage?.getItem(key) ?? "null") as Draft | null;
    if (value !== null && Array.isArray(value.entries) && Array.isArray(value.run)) draft = value;
  } catch {
    draft = EMPTY;
  }
  cache.set(key, draft);
  return draft;
}

function write(scope: ProjectScope, draft: Draft): void {
  const key = storageKey(scope);
  cache.set(key, draft);
  try {
    globalThis.sessionStorage?.setItem(key, JSON.stringify(draft));
  } catch {
    // The draft stays in memory for this page.
  }
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

/** The references of the next message and of the active AgentRun of one Project. */
export function useReferenceDraft(scope: ProjectScope | undefined): Draft {
  return useSyncExternalStore(subscribe, () => scope === undefined ? EMPTY : read(scope));
}

/** The references that the author added to the next message. */
export function draftEntries(scope: ProjectScope): DraftReference[] {
  return read(scope).entries;
}

/**
 * True when at least one added reference is not in the draft yet. The next message must also hold the
 * added references with the references of the active AgentRun. A reference of the Run does not count again.
 */
export function draftCanAdd(draft: Draft, added: readonly SelectionReference[]): boolean {
  const entries = draft.entries.map((entry) => entry.reference);
  return mergeReferences(entries, added).length > entries.length
    && mergeReferences(mergeReferences(draft.run, entries), added).length <= SELECTION_REFERENCE_LIMIT;
}

/**
 * Adds the references that are not in the draft yet and returns their keys. When the draft cannot hold
 * all of them, it adds nothing.
 */
export function addDraftReferences(scope: ProjectScope,
  added: readonly Omit<DraftReference, "key">[]): string[] | undefined {
  const draft = read(scope);
  if (!draftCanAdd(draft, added.map((entry) => entry.reference))) return undefined;
  const entries = [...draft.entries];
  const keys: string[] = [];
  for (const entry of added) {
    if (mergeReferences(entries.map((item) => item.reference), [entry.reference]).length === entries.length) continue;
    const key = `${Date.now()}:${Math.random()}`;
    entries.push({ ...entry, key });
    keys.push(key);
  }
  write(scope, { ...draft, entries });
  return keys;
}

export function removeDraftReference(scope: ProjectScope, key: string): void {
  const draft = read(scope);
  write(scope, { ...draft, entries: draft.entries.filter((entry) => entry.key !== key) });
}

/**
 * Removes the sent references by their keys after the Host received them. The history keeps their labels.
 * A reference that the author added while the send waited stays for the next message.
 */
export function clearDraftReferences(scope: ProjectScope, keys: readonly string[]): void {
  const draft = read(scope);
  const entries = draft.entries.filter((entry) => !keys.includes(entry.key));
  if (entries.length !== draft.entries.length) write(scope, { ...draft, entries });
}

/** Records the references of the active AgentRun, or none when no Run is active. */
export function setRunReferences(scope: ProjectScope, run: SelectionReference[]): void {
  const draft = read(scope);
  if (JSON.stringify(draft.run) !== JSON.stringify(run)) write(scope, { ...draft, run });
}

export function draftLabels(entries: readonly DraftReference[]): string[] {
  return entries.map((entry) => formatReferenceLabel(entry.label));
}

/**
 * Moves the references of one Chapter with an editor change.
 * - A Chapter text reference moves only while the editor tracks it. A reference whose text is all deleted
 *   disappears. A change that the editor cannot map keeps the reference and stops its tracking. Its base
 *   Revision then stays, and the Host blocks it when the head moved (ADR 0062).
 * - A candidate reference moves while the editor shows its candidate, and takes the newest Proposal
 *   Revision. It disappears when its text is deleted or when its candidate goes from the document, for
 *   example after a rejection or a withdrawal.
 */
export function trackDraftReferences(scope: ProjectScope, chapterId: string, tracked: Set<string>,
  transaction: Transaction, tree: ChapterOrder | undefined): void {
  const draft = read(scope);
  // A step that replaces all of the document, as a hydrate does, has no position mapping.
  const hydrate = transaction.steps.some((step, index) => step instanceof ReplaceStep
    && step.from === 0 && step.to === transaction.docs[index]!.content.size);
  const blocks = readManuscriptParagraphs(transaction.doc);
  if (blocks === undefined) return;
  const before = readManuscriptParagraphs(transaction.before);

  const chapterEntry = (entry: DraftReference, reference: ChapterTextReference): DraftReference | undefined => {
    if (!tracked.has(entry.key)) return entry;
    if (hydrate) {
      // A replaced document keeps the reference when each block of its range keeps its text and order.
      const slice = (list: typeof blocks) => {
        const first = list.findIndex((block) => block.manuscript_block_id === reference.start.manuscript_block_id);
        const last = list.findIndex((block) => block.manuscript_block_id === reference.end.manuscript_block_id);
        return first < 0 || last < first ? undefined : JSON.stringify(list.slice(first, last + 1));
      };
      const kept = before !== undefined && slice(before) !== undefined && slice(before) === slice(blocks);
      if (!kept) tracked.delete(entry.key);
      // The replaced document can be a new Revision, so a kept reference waits for its rebase.
      return kept ? { ...entry, moved: true } : entry;
    }
    const range = referenceRange(transaction.before, reference);
    const mapped = range === undefined ? undefined : mapReferenceRange(range, transaction.mapping);
    if (range !== undefined && mapped === undefined) {
      tracked.delete(entry.key);
      return undefined;
    }
    const next = mapped === undefined ? undefined
      : chapterTextReference(transaction.doc, mapped, chapterId, reference.base_authoritative_revision_id);
    if (next === undefined) {
      tracked.delete(entry.key);
      return entry;
    }
    const label = tree === undefined ? undefined : referenceLabel(tree, blocks, { reference: next });
    return { ...entry, reference: next, label: label ?? entry.label, moved: true };
  };

  const candidateEntry = (entry: DraftReference, reference: CandidateTextReference): DraftReference | undefined => {
    const range = candidateRange(transaction.before, reference);
    // The editor did not show the candidate, for example before the Proposals of the Chapter load.
    if (range === undefined) return entry;
    let next: CandidateTextReference | undefined;
    if (hydrate) {
      // A replaced document keeps the reference while its candidate keeps the referenced text.
      const after = candidateRange(transaction.doc, reference);
      next = after === undefined || transaction.doc.textBetween(after.from, after.to)
        !== transaction.before.textBetween(range.from, range.to) ? undefined
        : candidateTextReference(transaction.doc, after, chapterId);
    } else {
      const mapped = mapReferenceRange(range, transaction.mapping);
      next = mapped === undefined ? undefined : candidateTextReference(transaction.doc, mapped, chapterId);
    }
    if (next === undefined) return undefined;
    return JSON.stringify(next) === JSON.stringify(reference) ? entry : { ...entry, reference: next };
  };

  let changed = false;
  const entries: DraftReference[] = [];
  for (const entry of draft.entries) {
    const next = entry.reference.chapter_id !== chapterId ? entry
      : entry.reference.kind === "chapter_text" ? chapterEntry(entry, entry.reference)
        : candidateEntry(entry, entry.reference);
    changed ||= next !== entry;
    if (next !== undefined) entries.push(next);
  }
  if (changed) write(scope, { ...draft, entries });
}

/**
 * Makes each reference to an accepted candidate a Chapter text reference at the Revision after the
 * acceptance. The accepted text replaces the source of its candidate, so the offsets move by the source
 * offset of an inline candidate.
 */
export function acceptDraftCandidates(scope: ProjectScope, proposalId: string, operationIds: readonly string[],
  revisionId: string): void {
  const draft = read(scope);
  const accepted = (entry: DraftReference) => entry.reference.kind === "proposal_candidate_text"
    && entry.reference.proposal_id === proposalId && operationIds.includes(entry.reference.operation_id)
    && entry.candidate !== undefined;
  if (!draft.entries.some(accepted)) return;
  write(scope, { ...draft, entries: draft.entries.map((entry) => {
    if (!accepted(entry) || entry.reference.kind !== "proposal_candidate_text" || entry.candidate === undefined) {
      return entry;
    }
    const { blockId, inlineFrom } = entry.candidate;
    const point = (offset: number) => ({ manuscript_block_id: blockId, offset: (inlineFrom ?? 0) + offset });
    return { key: entry.key, moved: false, label: { chapter: entry.label.chapter, part: entry.label.part },
      reference: { kind: "chapter_text", chapter_id: entry.reference.chapter_id,
        base_authoritative_revision_id: revisionId, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
        start: point(entry.reference.start_offset), end: point(entry.reference.end_offset) } };
  }) });
}

/** Gives the moved references of one Chapter the saved Revision of the editor document. */
export function rebaseDraftReferences(scope: ProjectScope, chapterId: string, tracked: Set<string>,
  revisionId: string): void {
  const draft = read(scope);
  const rebased = (entry: DraftReference) => entry.reference.chapter_id === chapterId && entry.moved
    && tracked.has(entry.key);
  if (!draft.entries.some(rebased)) return;
  write(scope, { ...draft, entries: draft.entries.map((entry) => !rebased(entry)
    || entry.reference.kind !== "chapter_text" ? entry
    : { ...entry, moved: false, reference: { ...entry.reference, base_authoritative_revision_id: revisionId } }) });
}

/** The keys of the Chapter text references of one Chapter that the editor can track from its Revision. */
export function trackableDraftReferences(scope: ProjectScope, chapterId: string, revisionId: string): string[] {
  return read(scope).entries.filter((entry) => entry.reference.kind === "chapter_text"
    && entry.reference.chapter_id === chapterId && entry.reference.base_authoritative_revision_id === revisionId
    && !entry.moved).map((entry) => entry.key);
}

/** The tracked references of the editor of the last copy, so that a paste of that copy is tracked too. */
let lastCopy: { id: string; tracked: Set<string> } | undefined;

/** The clipboard data of the references of an editor selection. */
export function copiedReferences(scope: ProjectScope, tracked: Set<string>,
  entries: readonly Omit<DraftReference, "key">[]): string {
  lastCopy = { id: `${Date.now()}:${Math.random()}`, tracked };
  return JSON.stringify({ scope, copy: lastCopy.id, entries });
}

/**
 * Adds the references of a paste to the draft and returns true. Data of another Project, data that is
 * not valid, and references that the draft cannot hold add nothing, and the paste then gives plain text.
 */
export function pasteReferences(scope: ProjectScope, data: string): boolean {
  let value: { scope?: ProjectScope; copy?: unknown; entries?: unknown };
  try {
    value = JSON.parse(data) as typeof value;
  } catch {
    return false;
  }
  if (value === null || typeof value !== "object" || value.scope?.owner_user_id !== scope.owner_user_id
    || value.scope.project_id !== scope.project_id || !Array.isArray(value.entries) || value.entries.length === 0
    || !value.entries.every((entry: Partial<DraftReference> | null) => entry !== null && typeof entry === "object"
      && (entry.reference?.kind === "chapter_text" || entry.reference?.kind === "proposal_candidate_text")
      && typeof entry.label?.chapter === "string" && typeof entry.moved === "boolean")) return false;
  const keys = addDraftReferences(scope, value.entries as Omit<DraftReference, "key">[]);
  if (keys === undefined) return false;
  if (lastCopy !== undefined && value.copy === lastCopy.id) for (const key of keys) lastCopy.tracked.add(key);
  return true;
}
