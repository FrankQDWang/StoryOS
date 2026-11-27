import type { ChapterText } from "./chapter-copies.ts";
import type { EditorSessionWritingController } from "./editor-session-writing.ts";
import type { InputOrigin } from "./editor-types.ts";
import { AUTHOR_EDIT_MAX_UNITS, type JournalAuthorEdit } from "./local-edit-journal.ts";
import type { ManuscriptParagraph } from "./manuscript-doc.ts";

/** The input kinds that an Arriving Chapter does not accept (ADR 0105, section 3). */
const REFUSED_ORIGINS: ReadonlySet<unknown> = new Set<InputOrigin>(["move_block", "retype_block", "cut", "drop"]);

/** The part of the Editor Session Writing Controller that the editor uses for input, and the input check. */
export interface ArrivalSink extends Pick<EditorSessionWritingController,
  "capture" | "setComposing" | "fail" | "canAcceptInput" | "abandonUndo" | "showSaving"> {
  /** True when the Arriving Chapter accepts an input of the transaction origin `origin`. */
  accepts(origin: unknown): boolean;
  /** The editor gives each selection, so the editor of the hand-over can keep the caret. */
  select(selection: EditorSelection): void;
}

/** A selection in the editor document, as ProseMirror positions. */
export interface EditorSelection {
  readonly anchor: number;
  readonly head: number;
}

/** One captured edit of Arrival Input, with its origin and the blocks after it. */
export interface HeldEdit {
  readonly edit: JournalAuthorEdit;
  readonly origin: InputOrigin;
  readonly blocks: readonly ManuscriptParagraph[];
}

/**
 * The author input in one Arriving Chapter (ADR 0105). It is only in the memory of the tab: it is not in the Local
 * Edit Journal and not in the Pending Edit Projection.
 */
export interface ArrivalInput extends ArrivalSink {
  readonly chapterId: string;
  /** The title and the Revision of the copy that the author types on. */
  readonly title: string;
  readonly revisionId: string;
  held(): readonly HeldEdit[];
  /** True when the author typed: a held edit, or the text of an IME composition. */
  hasInput(): boolean;
  /** The text that the Arriving Chapter shows: the copy with each held edit and the open composition. */
  blocks(): readonly ManuscriptParagraph[];
  /** True after a stop. The input then accepts no more edit. */
  stopped(): boolean;
  /** True while an IME composition is open. The hand-over waits for its end (ADR 0105, section 4). */
  composing(): boolean;
  /** The last selection in the Arriving Chapter. */
  selection(): EditorSelection | undefined;
  stop(): void;
  subscribe(listener: () => void): () => void;
  /** A number that each change makes larger. */
  version(): number;
}

export function createArrivalInput(chapterId: string, copy: ChapterText): ArrivalInput {
  const held: HeldEdit[] = [];
  let shown = copy.blocks;
  // The text with an open IME composition, which is not a held edit until the composition ends.
  let composed: readonly ManuscriptParagraph[] | undefined;
  let halted = false;
  let composition = false;
  let selected: EditorSelection | undefined;
  let version = 0;
  const listeners = new Set<() => void>();
  const changed = () => {
    version += 1;
    for (const listener of listeners) listener();
  };
  const input: ArrivalInput = {
    chapterId,
    title: copy.title,
    revisionId: copy.revisionId,
    // The bound of the in-memory queue of ADR 0038 applies to Arrival Input too.
    accepts: (origin) => !halted && held.length < AUTHOR_EDIT_MAX_UNITS && !REFUSED_ORIGINS.has(origin),
    capture(edit, origin, blocks) {
      if (!input.accepts(origin)) return;
      held.push({ edit, origin, blocks: blocks ?? shown });
      shown = blocks ?? shown;
      composed = undefined;
      changed();
    },
    // An Arriving Chapter shows no suggestion, so it has no candidate input and no Author Undo.
    canAcceptInput: () => false,
    abandonUndo: () => {},
    // The end of a composition gives its edit to `capture`, or no edit when the author cancelled it. After a stop,
    // `capture` accepts no edit, so the text of the composition stays in the shown text for the notice.
    setComposing(composing) {
      composition = composing;
      if (composing) return;
      if (halted && composed !== undefined) shown = composed;
      composed = undefined;
      changed();
    },
    select(selection) { selected = selection; },
    showSaving(blocks) {
      if (blocks === undefined) return;
      composed = blocks;
      changed();
    },
    fail: () => { input.stop(); },
    held: () => held,
    hasInput: () => held.length > 0 || composed !== undefined || shown !== copy.blocks,
    blocks: () => composed ?? shown,
    stopped: () => halted,
    composing: () => composition,
    selection: () => selected,
    stop() {
      if (halted) return;
      halted = true;
      changed();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    version: () => version,
  };
  return input;
}
