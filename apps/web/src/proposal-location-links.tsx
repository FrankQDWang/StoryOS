import type { GetAgentRunResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ProposalDestination } from "./proposal-navigation.ts";

export function ProposalLocationLinks({ run, sourceChapterId, onNavigate }: {
  run: GetAgentRunResponse;
  sourceChapterId: string;
  onNavigate?: ((destination: ProposalDestination) => void) | undefined;
}) {
  if (run.decision.kind !== "prose_change") return null;
  return <div className="proposal-locations">
    {run.decision.locations?.map((location) => {
      const outcome = location.outcome;
      if (outcome.kind === "refused") return <p key={`${location.chapter_id}:${location.manuscript_block_id}`}>{location.explanation}</p>;
      return <p key={outcome.operation_id}><button type="button" className="proposal-location"
        data-proposal-location={outcome.operation_id}
        onClick={() => onNavigate?.({ chapterId: location.chapter_id, focus: {
          proposalId: outcome.proposal_id, operationId: outcome.operation_id,
          revisionId: location.current?.revision_id ?? outcome.revision_id, blockId: location.manuscript_block_id,
        } })}><span className="location-title">{location.candidate_text.slice(0, 24)}<span aria-hidden="true">↗</span></span>
          <span className="location-reason">{location.explanation}</span></button></p>;
    })}
    {(run.decision.locations?.length ?? 0) > 0 ? <button type="button" data-proposal-return=""
      onClick={() => onNavigate?.({ chapterId: sourceChapterId })}>返回来源章节</button> : null}
  </div>;
}
