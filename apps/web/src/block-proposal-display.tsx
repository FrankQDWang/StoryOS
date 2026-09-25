import { useEffect, useState } from "react";

import { getProposal } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  BlockProposalInspect, ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { ManuscriptEditor, type ManuscriptEditorProps } from "./manuscript-editor.tsx";
import type { BlockProposalProjection } from "./block-proposal-decoration.ts";
import { candidateProjectionFromJournal } from "./local-edit-journal.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export type ProposalLocator = {
  proposalId: string;
  runId: string;
  decisionId: string;
};

function storageKey(scope: ProjectScope): string {
  return `block_proposals:${scope.owner_user_id}:${scope.project_id}`;
}

export function readProposalLocators(scope: ProjectScope): ProposalLocator[] {
  try {
    const raw = globalThis.sessionStorage?.getItem(storageKey(scope));
    if (raw === null || raw === undefined) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((item): item is ProposalLocator => item !== null
      && typeof item === "object" && UUID.test(item.proposalId)
      && UUID.test(item.runId) && UUID.test(item.decisionId));
  } catch {
    return [];
  }
}

export function rememberProposalLocator(scope: ProjectScope, locator: ProposalLocator): ProposalLocator[] {
  const next = [...readProposalLocators(scope).filter((item) =>
    item.proposalId !== locator.proposalId), locator];
  globalThis.sessionStorage?.setItem(storageKey(scope), JSON.stringify(next));
  return next;
}

type ProposalRead = {
  locator: ProposalLocator;
  proposal?: BlockProposalInspect | undefined;
};

export function BlockProposalDisplay({
  scope, chapterId, authoritativeRevisionId, locators, refreshKey, safeToProject, ...editorProps
}: ManuscriptEditorProps & {
  scope: ProjectScope;
  chapterId: string;
  authoritativeRevisionId: string;
  locators: readonly ProposalLocator[];
  refreshKey: number;
  safeToProject: boolean;
}) {
  const [reads, setReads] = useState<ProposalRead[]>([]);
  const [candidateTexts, setCandidateTexts] = useState<Record<string, string>>({});
  const [settlementRefresh, setSettlementRefresh] = useState(0);
  const locatorKey = locators.map((item) =>
    `${item.proposalId}:${item.runId}:${item.decisionId}`).join("|");

  useEffect(() => {
    let active = true;
    setReads([]);
    void Promise.all(locators.map(async (locator): Promise<ProposalRead> => {
      try {
        const response = await getProposal({
          baseUrl: editorProps.baseUrl,
          fetchImpl: editorProps.fetchImpl,
          projectId: scope.project_id,
          proposalId: locator.proposalId,
        });
        if (response.project_scope.owner_user_id !== scope.owner_user_id
          || response.project_scope.project_id !== scope.project_id
          || response.proposal.proposal_id !== locator.proposalId
          || response.proposal.source.run_id !== locator.runId
          || response.proposal.source.decision_id !== locator.decisionId) {
          return { locator };
        }
        return { locator, proposal: response.proposal };
      } catch {
        return { locator };
      }
    })).then((result) => {
      if (active) setReads(result);
    });
    return () => { active = false; };
  }, [scope.owner_user_id, scope.project_id, chapterId, locatorKey, refreshKey,
    settlementRefresh,
    editorProps.baseUrl, editorProps.fetchImpl]);

  useEffect(() => {
    let active = true;
    const workspace = editorProps.persistWorkspace;
    if (workspace === undefined) {
      setCandidateTexts({});
      return () => { active = false; };
    }
    void Promise.all(reads.map(async ({ proposal }) => {
      if (proposal === undefined || proposal.chapter_id !== chapterId) return undefined;
      const operation = proposal.operations.find((item) =>
        item.manuscript_block_id === proposal.manuscript_block_id);
      if (operation === undefined) return undefined;
      const text = await candidateProjectionFromJournal(workspace, {
        proposal_id: proposal.proposal_id,
        operation_id: operation.operation_id,
        revision_id: proposal.revision_id,
        manuscript_block_id: proposal.manuscript_block_id,
      });
      return text === undefined ? undefined
        : [`${proposal.proposal_id}:${proposal.revision_id}`, text] as const;
    })).then((values) => {
      if (active) setCandidateTexts(Object.fromEntries(values.filter((item) => item !== undefined)));
    }).catch(editorProps.onFailure);
    return () => { active = false; };
  }, [reads, chapterId, editorProps.persistWorkspace, editorProps.onFailure]);

  const blockCounts = new Map<string, number>();
  for (const block of editorProps.blocks) {
    blockCounts.set(block.manuscript_block_id,
      (blockCounts.get(block.manuscript_block_id) ?? 0) + 1);
  }
  const projections: BlockProposalProjection[] = [];
  const unavailable: ProposalRead[] = [];
  const allHeadsKnown = reads.length === locators.length
    && reads.every((item) => item.proposal !== undefined);
  const expectedHeads = reads.flatMap(({ proposal }) => proposal?.chapter_id === chapterId
    ? [proposal.revision_id] : []).sort();
  for (const { locator, proposal } of reads) {
    if (proposal !== undefined && proposal.chapter_id !== chapterId) continue;
    const operation = proposal?.operations.find((item) =>
      item.manuscript_block_id === proposal.manuscript_block_id);
    const safe = proposal !== undefined && proposal.kind === "block_edit"
      && operation !== undefined
      && proposal.base_authoritative_revision_id === authoritativeRevisionId
      && blockCounts.get(proposal.manuscript_block_id) === 1 && safeToProject;
    if (!safe) {
      unavailable.push({ locator, proposal });
      continue;
    }
    const eligible = allHeadsKnown && editorProps.editable
      && proposal.generation === "ready" && proposal.validation === "valid"
      && proposal.closure === "open" && operation.resolution === "pending"
      && operation.reservation_state === "unresolved"
      && proposal.validation_receipt.kind === "present"
      && proposal.validation_receipt.result === "valid";
    projections.push({
      proposalId: proposal.proposal_id,
      operationId: operation.operation_id,
      revisionId: proposal.revision_id,
      blockId: proposal.manuscript_block_id,
      sourceRunId: proposal.source.run_id,
      sourceDecisionId: proposal.source.decision_id,
      text: candidateTexts[`${proposal.proposal_id}:${proposal.revision_id}`]
        ?? proposal.candidate_text,
      eligible,
      expectedHeads,
      localPending: candidateTexts[`${proposal.proposal_id}:${proposal.revision_id}`]
        !== undefined,
    });
  }

  return (
    <>
      <ManuscriptEditor {...editorProps} proposals={projections}
        onCandidateSettled={() => setSettlementRefresh((value) => value + 1)} />
      {unavailable.map(({ locator, proposal }) => (
        <p className="block-proposal-unavailable" data-proposal-unavailable={locator.proposalId}
          data-proposal-revision-id={proposal?.revision_id ?? ""}
          data-proposal-operation-id={proposal?.operation_id ?? ""}
          data-proposal-source-run-id={proposal?.source.run_id ?? locator.runId}
          data-proposal-source-decision-id={proposal?.source.decision_id ?? locator.decisionId}
          data-proposal-eligibility="unavailable"
          key={locator.proposalId}>候选文字暂不可用，请检查当前章节和正文。</p>
      ))}
    </>
  );
}
