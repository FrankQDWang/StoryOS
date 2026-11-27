import { AUTHOR_EDIT_MAX_NORMALIZED_PRIMITIVES, AUTHOR_EDIT_MAX_UNITS } from "./local-edit-journal.ts";
import { captureManuscriptChange, paragraphsEqual } from "./manuscript-doc.ts";
import type { CapturedManuscriptEdit, ManuscriptParagraph } from "./manuscript-doc.ts";

/** The text of a tab with an earlier base: the blocks of that base, and the blocks that the tab shows. */
export interface EarlierBaseText {
  readonly base: readonly ManuscriptParagraph[];
  readonly shown: readonly ManuscriptParagraph[];
}

const kind = (block: ManuscriptParagraph) => block.block_kind ?? "paragraph";
const same = (left: ManuscriptParagraph, right: ManuscriptParagraph) =>
  left.text === right.text && kind(left) === kind(right);
const copy = (block: ManuscriptParagraph): ManuscriptParagraph => ({ manuscript_block_id: block.manuscript_block_id,
  text: block.text, block_kind: kind(block) });
const paragraph = (manuscriptBlockId: string, text: string): ManuscriptParagraph =>
  ({ manuscript_block_id: manuscriptBlockId, text, block_kind: "paragraph" });

/**
 * The Block IDs that the tab moved: of the blocks that the base and the tab both have, the blocks out of the longest
 * run that keeps the order of the base.
 */
function movedBlocks(base: readonly ManuscriptParagraph[], shown: readonly ManuscriptParagraph[]): Set<string> {
  const at = new Map(base.map((block, index) => [block.manuscript_block_id, index]));
  const common = shown.filter((block) => at.has(block.manuscript_block_id));
  // The longest increasing run of base positions, by patience sorting.
  const tails: number[] = [];
  const previous: number[] = [];
  for (const [index, block] of common.entries()) {
    const position = at.get(block.manuscript_block_id)!;
    let low = 0;
    let high = tails.length;
    while (low < high) {
      const middle = (low + high) >> 1;
      if (at.get(common[tails[middle]!]!.manuscript_block_id)! < position) low = middle + 1;
      else high = middle;
    }
    previous[index] = low > 0 ? tails[low - 1]! : -1;
    tails[low] = index;
  }
  const kept = new Set<string>();
  for (let index = tails.at(-1) ?? -1; index !== -1; index = previous[index]!) {
    kept.add(common[index]!.manuscript_block_id);
  }
  return new Set(common.filter((block) => !kept.has(block.manuscript_block_id)).map((block) =>
    block.manuscript_block_id));
}

/**
 * Puts the text of a tab with an earlier base on the current Chapter, block by block (ADR 0107, section 2). A block that
 * the tab changed replaces the current block, so the later edit of a block wins. A block that only the other tab
 * changed keeps its current text. A block that the tab added or moved goes after the nearest earlier block of the tab,
 * with its Block ID. A changed block that the other tab deleted goes in as a paragraph at its place with `mintId()`,
 * when it has text. A block that the tab deleted leaves only when the other tab did not change it.
 */
export function blocksOnCurrent(text: EarlierBaseText, current: readonly ManuscriptParagraph[],
  mintId: () => string): ManuscriptParagraph[] {
  const base = new Map(text.base.map((block) => [block.manuscript_block_id, block]));
  const shown = new Map(text.shown.map((block) => [block.manuscript_block_id, block]));
  const now = new Map(current.map((block) => [block.manuscript_block_id, block]));
  const moved = movedBlocks(text.base, text.shown);
  const changed = (block: ManuscriptParagraph) => {
    const before = base.get(block.manuscript_block_id);
    return before === undefined || !same(before, block);
  };
  const result = current.filter((block) => {
    if (moved.has(block.manuscript_block_id)) return false;
    const before = base.get(block.manuscript_block_id);
    return shown.has(block.manuscript_block_id) || before === undefined || !same(before, block);
  }).map((block) => {
    const mine = shown.get(block.manuscript_block_id);
    return copy(mine !== undefined && changed(mine) ? mine : block);
  });
  // The Block ID in the result of each block of the tab that the result has.
  const placed = new Map(result.map((block) => [block.manuscript_block_id, block.manuscript_block_id]));
  for (const [index, block] of text.shown.entries()) {
    if (placed.has(block.manuscript_block_id)) continue;
    const there = now.get(block.manuscript_block_id);
    let entry: ManuscriptParagraph;
    if (there !== undefined) entry = copy(changed(block) ? block : there);
    else if (!base.has(block.manuscript_block_id)) entry = copy(block);
    else if (changed(block) && block.text !== "") entry = paragraph(mintId(), block.text);
    else continue;
    const earlier = text.shown.slice(0, index).reverse().find((item) => placed.has(item.manuscript_block_id));
    const at = earlier === undefined ? 0
      : result.findIndex((item) => item.manuscript_block_id === placed.get(earlier.manuscript_block_id)) + 1;
    placed.set(block.manuscript_block_id, entry.manuscript_block_id);
    result.splice(at, 0, entry);
  }
  return result;
}

/**
 * Gives the edits from the current blocks to `result` in the form that the editor captures. The Journal records them
 * as one replacement of the author, so they go in one Author Edit with one Revision and one Author Undo. A changed block
 * with the blocks after it that the result adds or removes is one replacement, as a paste is, and so is a run of new
 * blocks. The result is `undefined` when a change has no Author Edit form, or when the edits are more than one Author
 * Edit can have.
 */
export function stepsOnCurrent(current: readonly ManuscriptParagraph[], result: readonly ManuscriptParagraph[])
  : CapturedManuscriptEdit[] | undefined {
  const steps: CapturedManuscriptEdit[] = [];
  let now = current.map(copy);
  const step = (next: ManuscriptParagraph[]) => {
    const edit = captureManuscriptChange(now, next);
    if (edit === undefined) return false;
    steps.push(edit);
    now = next;
    return true;
  };
  const indexOf = (manuscriptBlockId: string) => now.findIndex((item) =>
    item.manuscript_block_id === manuscriptBlockId);
  const kept = new Set(result.map((block) => block.manuscript_block_id));
  // A changed block, the blocks after it that leave, and the blocks after it that come in.
  for (const [index, block] of result.entries()) {
    const at = indexOf(block.manuscript_block_id);
    if (at === -1 || now[at]!.text === block.text) continue;
    let removed = 0;
    while (at + 1 + removed < now.length && !kept.has(now[at + 1 + removed]!.manuscript_block_id)) removed += 1;
    let added = 0;
    while (index + 1 + added < result.length && indexOf(result[index + 1 + added]!.manuscript_block_id) === -1) added += 1;
    if (removed + added === 0) continue;
    // A replacement with no form leaves its changes to the steps below.
    step([...now.slice(0, at), { ...now[at]!, text: block.text },
      ...result.slice(index + 1, index + 1 + added).map((item) => paragraph(item.manuscript_block_id, item.text)),
      ...now.slice(at + 1 + removed)]);
  }
  // The blocks before `index` are the blocks of the result. The blocks that leave stay after them.
  for (const [index, block] of result.entries()) {
    let at = indexOf(block.manuscript_block_id);
    const inserted = at === -1;
    if (inserted) {
      // As the editor does: a split at the end of the earlier block gives an empty paragraph. At the start, the split
      // is at the end of the first block, and the new block then moves to the start.
      const after = Math.max(index - 1, 0);
      if (now.length === 0 || !step([...now.slice(0, after + 1), paragraph(block.manuscript_block_id, ""),
        ...now.slice(after + 1)])) return undefined;
      at = after + 1;
    }
    if (at !== index) {
      const next = [...now];
      next.splice(index, 0, ...next.splice(at, 1));
      if (!step(next)) return undefined;
    }
    // The new blocks after an inserted block go in with their text as one replacement, as a paste does.
    let added = 0;
    while (inserted && index + 1 + added < result.length
      && indexOf(result[index + 1 + added]!.manuscript_block_id) === -1) added += 1;
    if (added > 0 && block.text !== "") {
      step([...now.slice(0, index), { ...now[index]!, text: block.text },
        ...result.slice(index + 1, index + 1 + added).map((item) => paragraph(item.manuscript_block_id, item.text)),
        ...now.slice(index + 1)]);
    }
    if (now[index]!.text !== block.text
      && !step(now.map((item, position) => position === index ? { ...item, text: block.text } : item))) {
      return undefined;
    }
    if (kind(now[index]!) !== kind(block)
      && !step(now.map((item, position) => position === index ? { ...item, block_kind: kind(block) } : item))) {
      return undefined;
    }
  }
  // Each block that leaves is after the blocks of the result: its text goes, and it joins the block before it.
  while (now.length > result.length) {
    const at = now.length - 1;
    if (now[at]!.text !== "" && !step(now.map((item, position) => position === at ? { ...item, text: "" } : item))) {
      return undefined;
    }
    if (!step(now.slice(0, at))) return undefined;
  }
  if (!paragraphsEqual(now, result) || steps.length > AUTHOR_EDIT_MAX_UNITS || steps.some((edit) =>
    edit.kind === "contiguous_replacement" && edit.primitives.length > AUTHOR_EDIT_MAX_NORMALIZED_PRIMITIVES)) {
    return undefined;
  }
  return steps;
}
