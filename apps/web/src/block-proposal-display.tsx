import { useEffect, useState } from "react";

import { getProposal } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  BlockProposalInspect, ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { ManuscriptEditor, type ManuscriptEditorProps } from "./manuscript-editor.tsx";
import type { BlockProposalProjection } from "./block-proposal-decoration.ts";

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
  proposal?: BlockProposalInspect;
};

export function BlockProposalDisplay({
  scope, chapterId, authoritativeRevisionId, locators, safeToProject, ...editorProps
}: ManuscriptEditorProps & {
  scope: ProjectScope;
  chapterId: string;
  authoritativeRevisionId: string;
  locators: readonly ProposalLocator[];
  safeToProject: boolean;
}) {
  const [reads, setReads] = useState<ProposalRead[]>([]);
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
  }, [scope.owner_user_id, scope.project_id, chapterId, locatorKey,
    editorProps.baseUrl, editorProps.fetchImpl]);

  const blockIds = new Set(editorProps.blocks.map((block) => block.manuscript_block_id));
  const projections: BlockProposalProjection[] = [];
  const unavailable: string[] = [];
  for (const { locator, proposal } of reads) {
    if (proposal !== undefined && proposal.chapter_id !== chapterId) continue;
    const operation = proposal?.operations.find((item) =>
      item.manuscript_block_id === proposal.manuscript_block_id);
    const safe = proposal !== undefined && proposal.kind === "block_edit"
      && operation !== undefined && operation.resolution === "pending"
      && operation.reservation_state === "unresolved"
      && proposal.generation === "ready" && proposal.validation === "valid"
      && proposal.closure === "open" && proposal.validation_receipt.kind === "present"
      && proposal.validation_receipt.result === "valid"
      && proposal.base_authoritative_revision_id === authoritativeRevisionId
      && blockIds.has(proposal.manuscript_block_id) && safeToProject;
    if (!safe) {
      unavailable.push(locator.proposalId);
      continue;
    }
    projections.push({
      proposalId: proposal.proposal_id,
      operationId: operation.operation_id,
      revisionId: proposal.revision_id,
      blockId: proposal.manuscript_block_id,
      sourceRunId: proposal.source.run_id,
      sourceDecisionId: proposal.source.decision_id,
      text: proposal.candidate_text,
    });
  }

  return (
    <>
      <ManuscriptEditor {...editorProps} proposals={projections} />
      {unavailable.map((proposalId) => (
        <p className="block-proposal-unavailable" data-proposal-unavailable={proposalId}
          key={proposalId}>候选文字暂不可用，请检查当前章节和正文。</p>
      ))}
    </>
  );
}
