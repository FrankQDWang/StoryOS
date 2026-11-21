/** A range of UTF-16 code units in a text. An empty range is a position. */
export type TextRange = { from: number; to: number };

/** One run of a candidate text against the text of its target Block, with its range in each text. */
export type ProposalChangeRun = { changed: boolean; old: TextRange; new: TextRange };

/** A run in code points. */
type Run = { changed: boolean; old: string[]; new: string[] };

const TABLE_LIMIT = 1_000_000;
const JOIN_LIMIT = 3;

/**
 * The runs of a candidate text against the text of its target Block, by the rule of S10-REQ-004 (ADR 0100), in
 * their sequence. A candidate with no mark has none.
 */
export function proposalChangeRuns(old: string, candidate: string): ProposalChangeRun[] | undefined {
  if (old === "" || candidate === "" || old === candidate) return undefined;
  const a = Array.from(old);
  const b = Array.from(candidate);
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start += 1;
  let end = 0;
  while (end < a.length - start && end < b.length - start && a[a.length - 1 - end] === b[b.length - 1 - end]) end += 1;
  const runs: Run[] = [];
  append(runs, false, a.slice(0, start), b.slice(0, start));
  appendMiddle(runs, a.slice(start, a.length - end), b.slice(start, b.length - end));
  append(runs, false, a.slice(a.length - end), b.slice(b.length - end));
  joinShortUnchangedRuns(runs);
  const kept = runs.reduce((sum, run) => run.changed ? sum : sum + run.old.length, 0);
  if (kept < 4 || kept * 5 < Math.min(a.length, b.length)) return undefined;
  let oldAt = 0;
  let newAt = 0;
  return runs.map((run) => {
    const oldFrom = oldAt;
    const newFrom = newAt;
    oldAt += run.old.join("").length;
    newAt += run.new.join("").length;
    return { changed: run.changed, old: { from: oldFrom, to: oldAt }, new: { from: newFrom, to: newAt } };
  });
}

/** Adds code points to the end of the runs. Adjacent code points of the same kind make one run. */
function append(runs: Run[], changed: boolean, old: string[], candidate: string[]): void {
  if (old.length === 0 && candidate.length === 0) return;
  const last = runs.at(-1);
  if (last?.changed === changed) {
    last.old.push(...old);
    last.new.push(...candidate);
  } else {
    runs.push({ changed, old: [...old], new: [...candidate] });
  }
}

/** The runs of the middle parts from a longest common subsequence that removes before it adds. */
function appendMiddle(runs: Run[], a: string[], b: string[]): void {
  if (a.length * b.length > TABLE_LIMIT) {
    append(runs, true, a, b);
    return;
  }
  const width = b.length + 1;
  // The length of a common subsequence is at most 1,000 when the table is in the limit.
  const table = new Uint16Array((a.length + 1) * width);
  for (let i = a.length - 1; i >= 0; i -= 1) {
    for (let j = b.length - 1; j >= 0; j -= 1) {
      table[i * width + j] = a[i] === b[j] ? table[(i + 1) * width + j + 1]! + 1
        : Math.max(table[(i + 1) * width + j]!, table[i * width + j + 1]!);
    }
  }
  let i = 0;
  let j = 0;
  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && a[i] === b[j]) {
      append(runs, false, [a[i]!], [b[j]!]);
      i += 1;
      j += 1;
    } else if (i < a.length && (j === b.length || table[(i + 1) * width + j]! >= table[i * width + j + 1]!)) {
      append(runs, true, [a[i]!], []);
      i += 1;
    } else {
      append(runs, true, [], [b[j]!]);
      j += 1;
    }
  }
}

/** Joins each short unchanged run between two changed runs into one changed run, from the start again each time. */
function joinShortUnchangedRuns(runs: Run[]): void {
  const size = (run: Run) => Math.max(run.old.length, run.new.length);
  for (let index = 1; index < runs.length - 1; index += 1) {
    const before = runs[index - 1]!;
    const unchanged = runs[index]!;
    const after = runs[index + 1]!;
    const length = unchanged.old.length;
    if (unchanged.changed || length > JOIN_LIMIT || length > size(before) || length > size(after)
      || before.new.length === 0 && after.new.length === 0
      || before.old.length === 0 && after.old.length === 0) continue;
    runs.splice(index - 1, 3, { changed: true, old: [...before.old, ...unchanged.old, ...after.old],
      new: [...before.new, ...unchanged.new, ...after.new] });
    index = 0;
  }
}
