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

/**
 * Puts the text of a tab with an earlier base on the current Chapter, block by block (ADR 0107, section 2). A block that
 * the tab changed replaces the current block, so the later edit of a block wins. A block that only the other tab
 * changed keeps its current text. A changed block that the current Chapter does not have goes in as a paragraph after
 * the nearest earlier block of the tab: a block that the tab added keeps its id, and a block that the other tab deleted
 * gets `mintId()`. A block that the tab deleted leaves only when the other tab did not change it.
 */
export function blocksOnCurrent(text: EarlierBaseText, current: readonly ManuscriptParagraph[],
  mintId: () => string): ManuscriptParagraph[] {
  const base = new Map(text.base.map((block) => [block.manuscript_block_id, block]));
  const shown = new Map(text.shown.map((block) => [block.manuscript_block_id, block]));
  const changed = (block: ManuscriptParagraph) => {
    const before = base.get(block.manuscript_block_id);
    return before === undefined || !same(before, block);
  };
  const result = current.filter((block) => {
    const before = base.get(block.manuscript_block_id);
    return shown.has(block.manuscript_block_id) || before === undefined || !same(before, block);
  }).map((block) => {
    const mine = shown.get(block.manuscript_block_id);
    return copy(mine !== undefined && changed(mine) ? mine : block);
  });
  // The id in the result of each block of the tab that the result has.
  const placed = new Map(result.map((block) => [block.manuscript_block_id, block.manuscript_block_id]));
  for (const [index, block] of text.shown.entries()) {
    if (placed.has(block.manuscript_block_id) || !changed(block) || block.text === "") continue;
    const id = base.has(block.manuscript_block_id) ? mintId() : block.manuscript_block_id;
    const earlier = text.shown.slice(0, index).reverse().find((item) => placed.has(item.manuscript_block_id));
    const at = earlier === undefined ? 0
      : result.findIndex((item) => item.manuscript_block_id === placed.get(earlier.manuscript_block_id)) + 1;
    placed.set(block.manuscript_block_id, id);
    const first = result[0];
    if (at > 0 || first === undefined) {
      result.splice(at, 0, { manuscript_block_id: id, text: block.text, block_kind: "paragraph" });
      continue;
    }
    // The Author Edit primitives insert a block only after a block. At the start, the first block gets the text of the
    // tab, and its own text goes to the new block id after it.
    placed.set(first.manuscript_block_id, id);
    placed.set(block.manuscript_block_id, first.manuscript_block_id);
    result.splice(0, 1, { manuscript_block_id: first.manuscript_block_id, text: block.text, block_kind: "paragraph" },
      { ...first, manuscript_block_id: id });
  }
  return result;
}

/**
 * Gives the edits from the current blocks to `result`, one change in each, in the form that the editor captures. It
 * throws when a change has no Author Edit form, for example the removal of the first block. The Journal records them
 * as one replacement of the author, so they go in one Author Edit with one Revision and one Author Undo.
 */
export function stepsOnCurrent(current: readonly ManuscriptParagraph[], result: readonly ManuscriptParagraph[])
  : CapturedManuscriptEdit[] {
  const steps: CapturedManuscriptEdit[] = [];
  let now = current.map(copy);
  const step = (next: ManuscriptParagraph[]) => {
    const edit = captureManuscriptChange(now, next);
    if (edit === undefined) throw new Error("A catch-up step has no Author Edit form");
    steps.push(edit);
    now = next;
  };
  const kept = new Set(result.map((block) => block.manuscript_block_id));
  for (const removed of current.filter((block) => !kept.has(block.manuscript_block_id))) {
    const at = now.findIndex((block) => block.manuscript_block_id === removed.manuscript_block_id);
    if (removed.text !== "") step(now.map((block, index) => index === at ? { ...block, text: "" } : block));
    step(now.filter((_, index) => index !== at));
  }
  for (const [index, block] of result.entries()) {
    let at = now.findIndex((item) => item.manuscript_block_id === block.manuscript_block_id);
    if (at === -1) {
      at = now.findIndex((item) => item.manuscript_block_id === result[index - 1]?.manuscript_block_id) + 1;
      if (at === 0) throw new Error("A catch-up step has no Author Edit form");
      // As the editor does: a split at the end of the earlier block gives an empty paragraph, then the text goes in.
      step([...now.slice(0, at), { manuscript_block_id: block.manuscript_block_id, text: "",
        block_kind: "paragraph" }, ...now.slice(at)]);
    }
    if (now[at]!.text !== block.text) {
      step(now.map((item, position) => position === at ? { ...item, text: block.text } : item));
    }
    if (kind(now[at]!) !== kind(block)) {
      step(now.map((item, position) => position === at ? { ...item, block_kind: kind(block) } : item));
    }
  }
  if (!paragraphsEqual(now, result)) throw new Error("A catch-up step has no Author Edit form");
  return steps;
}
