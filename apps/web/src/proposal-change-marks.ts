import type { Node as ProseMirrorNode } from "@tiptap/pm/model";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet, type EditorView } from "@tiptap/pm/view";

import { proposalChangeRuns, type ProposalChangeRun } from "./proposal-change-runs.ts";

/** The runs of one candidate, with the document positions where its target text and its own text start. */
type CandidateMarks = { old: string; text: string; target: number; candidate: number;
  runs: readonly ProposalChangeRun[] };
/** The mark under the pointer: the index of its candidate in the marked candidates, and the index of its run. */
type Pair = { candidate: number; run: number };
type MarksState = { candidates: CandidateMarks[]; pair: Pair | undefined; decorations: DecorationSet; stale: boolean };

const marksKey = new PluginKey<MarksState>("storyosProposalChangeMarks");
const CHANGE_ATTRIBUTE = "data-proposal-change";

function candidateMarks(doc: ProseMirrorNode, previous: readonly CandidateMarks[]): CandidateMarks[] {
  const known = new Map(previous.map((marks) => [`${marks.old}\u0000${marks.text}`, marks.runs]));
  const result: CandidateMarks[] = [];
  let target: { node: ProseMirrorNode; start: number } | undefined;
  doc.forEach((node, offset) => {
    if (node.type.name !== "blockProposal") {
      target = { node, start: offset + 1 };
      return;
    }
    if (target === undefined || target.node.attrs.id !== node.attrs.blockId) return;
    const old = target.node.textContent;
    const text = node.textContent;
    const runs = known.get(`${old}\u0000${text}`) ?? proposalChangeRuns(old, text);
    if (runs !== undefined) result.push({ old, text, target: target.start, candidate: offset + 1, runs });
  });
  return result;
}

function tick(id: string, paired: boolean): HTMLElement {
  const element = document.createElement("span");
  element.className = paired ? "proposal-change-tick proposal-change-pair" : "proposal-change-tick";
  element.setAttribute(CHANGE_ATTRIBUTE, id);
  element.append(Object.assign(document.createElement("span"), { className: "proposal-change-tick-line" }));
  return element;
}

function changeDecorations(doc: ProseMirrorNode, candidates: readonly CandidateMarks[],
  pair: Pair | undefined): DecorationSet {
  const decorations: Decoration[] = [];
  candidates.forEach((marks, candidateIndex) => {
    marks.runs.forEach((run, runIndex) => {
      if (!run.changed) return;
      const id = `${candidateIndex}:${runIndex}`;
      const paired = pair?.candidate === candidateIndex && pair.run === runIndex;
      const from = marks.candidate + run.new.from;
      decorations.push(run.new.from === run.new.to
        ? Decoration.widget(from, () => tick(id, paired), { key: `${id}:${paired}` })
        : Decoration.inline(from, marks.candidate + run.new.to, {
          class: paired ? "proposal-change-mark proposal-change-pair" : "proposal-change-mark",
          [CHANGE_ATTRIBUTE]: id }));
      if (paired && run.old.to > run.old.from) {
        decorations.push(Decoration.inline(marks.target + run.old.from, marks.target + run.old.to,
          { class: "proposal-change-pair" }));
      }
    });
  });
  return DecorationSet.create(doc, decorations);
}

function marksState(doc: ProseMirrorNode, previous: readonly CandidateMarks[]): MarksState {
  const candidates = candidateMarks(doc, previous);
  return { candidates, pair: undefined, decorations: changeDecorations(doc, candidates, undefined), stale: false };
}

function pairAt(view: EditorView, target: EventTarget | null): Pair | undefined {
  const element = target instanceof Element ? target.closest(`[${CHANGE_ATTRIBUTE}]`) : null;
  if (element === null || !view.dom.contains(element)) return undefined;
  const [candidate, run] = (element.getAttribute(CHANGE_ATTRIBUTE) ?? "").split(":").map(Number);
  return candidate === undefined || run === undefined ? undefined : { candidate, run };
}

function showPair(view: EditorView, pair: Pair | undefined): void {
  const current = marksKey.getState(view.state);
  if (view.composing || current === undefined || current.stale
    || current.pair?.candidate === pair?.candidate && current.pair?.run === pair?.run) return;
  view.dispatch(view.state.tr.setMeta(marksKey, { pair }));
}

/**
 * The marks on the new words of each candidate, and the pair of the mark under the pointer (S10-REQ-004). They are
 * view decorations only. During an IME composition they do not change.
 */
export function proposalChangeMarks(): Plugin<MarksState> {
  let composing = false;
  return new Plugin<MarksState>({
    key: marksKey,
    state: {
      init: (_config, state) => marksState(state.doc, []),
      apply(transaction, value, _previous, state) {
        if (composing) {
          return transaction.docChanged
            ? { ...value, decorations: value.decorations.map(transaction.mapping, transaction.doc), stale: true }
            : value;
        }
        if (transaction.docChanged || value.stale) return marksState(state.doc, value.candidates);
        const meta = transaction.getMeta(marksKey) as { pair?: Pair } | undefined;
        if (meta === undefined || !("pair" in meta)) return value;
        return { ...value, pair: meta.pair, decorations: changeDecorations(state.doc, value.candidates, meta.pair) };
      },
    },
    props: {
      decorations: (state) => marksKey.getState(state)?.decorations,
      handleDOMEvents: {
        compositionstart: () => {
          composing = true;
          return false;
        },
        compositionend: (view) => {
          composing = false;
          // The marks follow the composed text after the editor reads it.
          setTimeout(() => {
            if (!view.isDestroyed && marksKey.getState(view.state)?.stale === true) {
              view.dispatch(view.state.tr.setMeta(marksKey, {}));
            }
          });
          return false;
        },
        mouseover: (view, event) => {
          showPair(view, pairAt(view, event.target));
          return false;
        },
        mouseout: (view, event) => {
          if (!(event.relatedTarget instanceof Node) || !view.dom.contains(event.relatedTarget)) {
            showPair(view, undefined);
          }
          return false;
        },
      },
    },
  });
}
