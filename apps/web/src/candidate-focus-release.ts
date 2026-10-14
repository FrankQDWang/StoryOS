import { useEffect, useRef } from "react";

import type { BlockProposalInspect } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ProposalFocus } from "./proposal-navigation.ts";

type CandidateFocusReport = (focus: ProposalFocus | undefined) => void;

/** Forwards each candidate focus, and reports no focus when a Proposal read shows that the focused candidate is not pending. */
export function useCandidateFocusRelease(
  reads: readonly { proposal?: BlockProposalInspect | undefined }[],
  onCandidateFocus: CandidateFocusReport | undefined,
): CandidateFocusReport {
  const focusedRef = useRef<ProposalFocus | undefined>(undefined);
  const reportRef = useRef(onCandidateFocus);
  reportRef.current = onCandidateFocus;
  useEffect(() => {
    const focused = focusedRef.current;
    if (focused === undefined) return;
    const proposal = reads.find((read) => read.proposal?.proposal_id === focused.proposalId)?.proposal;
    if (proposal === undefined || (proposal.closure === "open" && proposal.operations.some((operation) =>
      operation.operation_id === focused.operationId && operation.resolution === "pending"))) return;
    focusedRef.current = undefined;
    reportRef.current?.(undefined);
  }, [reads]);
  return (focus) => {
    focusedRef.current = focus;
    reportRef.current?.(focus);
  };
}
