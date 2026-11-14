import { Fragment } from "react";
import type { ReactNode } from "react";

import type {
  AgentRunResultInspect, GetAgentRunResponse, GetManuscriptTreeResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RESEARCH_SYNTHESIS_SENTENCE, proposalSentence } from "./assistant-run-outcome.ts";
import { ProposalLocationLinks } from "./proposal-location-links.tsx";
import type { ProposalDestination } from "./proposal-navigation.ts";
import { ResearchTurn } from "./research-turn.tsx";
import type { AssistantContext } from "./writing-assistant-panel.tsx";

function resultLine(result: AgentRunResultInspect): string {
  switch (result.kind) {
    case "proposal":
      return proposalSentence(result.locations);
    case "research_artifact":
      return RESEARCH_SYNTHESIS_SENTENCE;
    case "answer":
      return result.text;
  }
}

/**
 * Each result of a settled Run in the order in which the model made them (ADR 0091 section 6): a Proposal with its
 * change sentence and locations, a Research Artifact with its research turn, and the final answer text. A Run with
 * one result shows it as the decision of that kind did before. The current turn also shows the sources of a Run with
 * no Research Artifact after its first result, and `children` go with the Research Artifact.
 */
export function RunResultList({ run, results, context, history, tree, sourceChapterId, onNavigate, children }: {
  run: GetAgentRunResponse;
  results: readonly AgentRunResultInspect[];
  context: AssistantContext | undefined;
  history: boolean;
  tree: GetManuscriptTreeResponse | undefined;
  sourceChapterId: string;
  onNavigate: ((destination: ProposalDestination) => void) | undefined;
  children?: ReactNode;
}) {
  const researched = results.some(result => result.kind === "research_artifact");
  return <>{results.map((result, index) => <Fragment key={index}>
    <p className="assistant-result" data-assistant-result={history ? undefined : ""}>{resultLine(result)}</p>
    {context !== undefined && (result.kind === "research_artifact" || (!history && !researched && index === 0))
      ? <ResearchTurn context={context} run={run} history={history}
        artifactId={result.kind === "research_artifact" ? result.research_artifact_id : undefined}>{children}</ResearchTurn>
      : null}
    {result.kind === "proposal" ? <ProposalLocationLinks locations={result.locations} tree={tree}
      sourceChapterId={sourceChapterId} onNavigate={onNavigate} /> : null}
  </Fragment>)}</>;
}
