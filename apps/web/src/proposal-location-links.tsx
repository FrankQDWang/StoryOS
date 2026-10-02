import type { GetAgentRunResponse, GetManuscriptTreeResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ProposalDestination } from "./proposal-navigation.ts";

export function ProposalLocationLinks({ run, tree, sourceChapterId, onNavigate }: {
  run: GetAgentRunResponse;
  tree?: GetManuscriptTreeResponse | undefined;
  sourceChapterId: string;
  onNavigate?: ((destination: ProposalDestination) => void) | undefined;
}) {
  if (run.decision.kind !== "prose_change") return null;
  return <div className="proposal-locations">
    {run.decision.locations?.map((location, index, locations) => {
      const outcome = location.outcome;
      if (outcome.kind === "refused") return <p key={`${location.chapter_id}:${location.manuscript_block_id}`}>{location.explanation}</p>;
      return <p key={outcome.operation_id}><button type="button" className="proposal-location"
        data-proposal-location={outcome.operation_id}
        onClick={() => onNavigate?.({ chapterId: location.chapter_id, focus: {
          proposalId: outcome.proposal_id, operationId: outcome.operation_id,
          revisionId: location.current?.revision_id ?? outcome.revision_id, blockId: location.manuscript_block_id,
        } })}><span className="location-title">{tree?.volumes.flatMap(volume => volume.chapters).find(chapter => chapter.chapter_id === location.chapter_id)?.title}
            · 第{locations.slice(0, index + 1).filter(item => item.chapter_id === location.chapter_id).length}处修改<span aria-hidden="true">↗</span></span>
          <span className="location-reason">{location.explanation}</span></button></p>;
    })}
    {(run.decision.locations?.length ?? 0) > 0 ? <button type="button" data-proposal-return=""
      onClick={() => onNavigate?.({ chapterId: sourceChapterId })}>返回来源章节</button> : null}
  </div>;
}
