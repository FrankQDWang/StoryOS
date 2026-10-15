import { useSyncExternalStore } from "react";
import type { Transaction } from "@tiptap/pm/state";
import { ReplaceStep } from "@tiptap/pm/transform";
import type { ProjectScope, SelectionReference }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { paragraphsEqual, readManuscriptParagraphs } from "./manuscript-doc.ts";
import {
  canAddReference, chapterTextReference, formatReferenceLabel, mapReferenceRange, mergeReferences,
  referenceLabel, referenceRange, type ChapterOrder, type ChapterTextReference, type ReferenceLabel,
} from "./selection-reference.ts";

/** One reference that the author added to the next message, with its label. */
export type DraftReference = {
  key: string;
  reference: ChapterTextReference;
  label: ReferenceLabel;
  /** The range moved with author edits that the base Revision does not have yet. */
  moved: boolean;
};

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

/** True when the next message can hold one more reference, with the references of the active AgentRun. */
export function draftCanAdd(draft: Draft): boolean {
  return canAddReference(mergeReferences(draft.run, draft.entries.map((entry) => entry.reference)));
}

/** Adds one reference and returns its key. A full draft or a repeated reference adds nothing. */
export function addDraftReference(scope: ProjectScope, entry: Omit<DraftReference, "key">): string | undefined {
  const draft = read(scope);
  if (!draftCanAdd(draft)
    || mergeReferences(draft.entries.map((item) => item.reference), [entry.reference]).length === draft.entries.length) {
    return undefined;
  }
  const key = `${Date.now()}:${Math.random()}`;
  write(scope, { ...draft, entries: [...draft.entries, { ...entry, key }] });
  return key;
}

export function removeDraftReference(scope: ProjectScope, key: string): void {
  const draft = read(scope);
  write(scope, { ...draft, entries: draft.entries.filter((entry) => entry.key !== key) });
}

/** Clears the references after a send. The history keeps their labels. */
export function clearDraftReferences(scope: ProjectScope): void {
  write(scope, { ...read(scope), entries: [] });
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
 * Moves the tracked references of one Chapter with an editor change. A reference whose text is all
 * deleted disappears. A change that the editor cannot map keeps the reference and stops its tracking.
 * Its base Revision then stays, and the Host blocks it when the head moved (ADR 0062).
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
  if (hydrate && before !== undefined && paragraphsEqual(before, blocks)) return;
  let changed = false;
  const entries: DraftReference[] = [];
  for (const entry of draft.entries) {
    if (entry.reference.chapter_id !== chapterId || !tracked.has(entry.key)) {
      entries.push(entry);
      continue;
    }
    if (hydrate) {
      // A replaced document keeps the reference when each block of its range keeps its text and order.
      const slice = (list: typeof blocks) => {
        const first = list.findIndex((block) => block.manuscript_block_id === entry.reference.start.manuscript_block_id);
        const last = list.findIndex((block) => block.manuscript_block_id === entry.reference.end.manuscript_block_id);
        return first < 0 || last < first ? undefined : JSON.stringify(list.slice(first, last + 1));
      };
      const kept = before !== undefined && slice(before) !== undefined && slice(before) === slice(blocks);
      if (!kept) tracked.delete(entry.key);
      entries.push(entry);
      continue;
    }
    const range = referenceRange(transaction.before, entry.reference);
    const mapped = range === undefined ? undefined : mapReferenceRange(range, transaction.mapping);
    if (range !== undefined && mapped === undefined) {
      tracked.delete(entry.key);
      changed = true;
      continue;
    }
    const reference = mapped === undefined ? undefined
      : chapterTextReference(transaction.doc, mapped, chapterId, entry.reference.base_authoritative_revision_id);
    if (reference === undefined) {
      tracked.delete(entry.key);
      entries.push(entry);
      continue;
    }
    changed = true;
    const label = tree === undefined ? undefined : referenceLabel(tree, blocks, reference);
    entries.push({ ...entry, reference, label: label ?? entry.label, moved: true });
  }
  if (changed) write(scope, { ...draft, entries });
}

/** Gives the moved references of one Chapter the saved Revision of the editor document. */
export function rebaseDraftReferences(scope: ProjectScope, chapterId: string, tracked: Set<string>,
  revisionId: string): void {
  const draft = read(scope);
  if (!draft.entries.some((entry) => entry.reference.chapter_id === chapterId && entry.moved
    && tracked.has(entry.key))) return;
  write(scope, { ...draft, entries: draft.entries.map((entry) =>
    entry.reference.chapter_id === chapterId && entry.moved && tracked.has(entry.key)
      ? { ...entry, moved: false, reference: { ...entry.reference, base_authoritative_revision_id: revisionId } }
      : entry) });
}

/** The keys of the references of one Chapter that the editor can track from its Revision. */
export function trackableDraftReferences(scope: ProjectScope, chapterId: string, revisionId: string): string[] {
  return read(scope).entries.filter((entry) => entry.reference.chapter_id === chapterId
    && entry.reference.base_authoritative_revision_id === revisionId && !entry.moved).map((entry) => entry.key);
}
