import type {
  GetAgentRunResponse, ManuscriptUseInspect,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { settledRun } from "./assistant-run-outcome.ts";

function manuscriptUseLine(use: ManuscriptUseInspect): string {
  switch (use.kind) {
    case "chapter_read":
      return `查阅了章节「${use.title}」`;
    case "keyword_search":
      return `在全书中搜索了「${use.query}」`;
    case "hybrid_search":
      return `按意思在全书中搜索了「${use.question}」`;
  }
}

/**
 * The Chapters that a settled Run read, the terms that it searched in the manuscript, and the questions that it
 * searched by meaning (ADR 0095 section 1). Each entry is one plain line, in the order of the Run read. The panel
 * shows the list after the Skills of the turn. It shows the list for the current turn and for each history turn, from
 * the Run read that it holds for the turn. A Run with no entry shows no list. The list has no control and shows no
 * text of a result (ADR 0040).
 */
export function ManuscriptUseList({ run }: { run: GetAgentRunResponse }) {
  const uses = settledRun(run) ? run.manuscript_uses ?? [] : [];
  return uses.length === 0 ? null
    : <ul className="assistant-skill-uses" aria-label="本次查阅的书稿" data-assistant-manuscript-uses="">
      {uses.map((use, index) => <li key={index}>{manuscriptUseLine(use)}</li>)}
    </ul>;
}
