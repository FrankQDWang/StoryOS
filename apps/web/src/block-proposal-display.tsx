import { useEffect, useRef, useState } from "react";

import { getProposal } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  BlockProposalInspect, ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { ManuscriptEditor, type ManuscriptEditorProps } from "./manuscript-editor.tsx";
import type { BlockProposalProjection } from "./block-proposal-decoration.ts";
import { candidateProjectionFromJournal } from "./local-edit-journal.ts";
import { acceptDisplayedBlockProposal, retryPendingDisplayedAcceptance } from "./accept-block-proposal.ts";
import { hasPendingDisplayedAcceptance, knownProblemDisplayedAcceptance, reconcileDisplayedAcceptance,
  settledDisplayedAcceptance } from "./acceptance-journal.ts";
import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE, historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";

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
  scope, chapterId, authoritativeRevisionId, locators, refreshKey, safeToProject,
  onAccepted, ...editorProps
}: ManuscriptEditorProps & {
  scope: ProjectScope;
  chapterId: string;
  authoritativeRevisionId: string;
  locators: readonly ProposalLocator[];
  refreshKey: number;
  safeToProject: boolean;
  onAccepted: () => Promise<void>;
}) {
  const [reads, setReads] = useState<ProposalRead[]>([]);
  const [candidateTexts, setCandidateTexts] = useState<Record<string, string>>({});
  const [settlementRefresh, setSettlementRefresh] = useState(0);
  const [decisionMessages, setDecisionMessages] = useState<Record<string, string>>({});
  const [knownProblems, setKnownProblems] = useState<Record<string, number>>({});
  const [accepting, setAccepting] = useState<string>();
  const [pendingAcceptances, setPendingAcceptances] = useState<string[]>([]);
  const [acceptanceChecked, setAcceptanceChecked] = useState(false);
  const refreshedAcceptance = useRef(new Set<string>());
  const acceptingRef = useRef(false);
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

  useEffect(() => {
    let active = true;
    setAcceptanceChecked(false);
    if (reads.length !== locators.length) return () => { active = false; };
    const workspace = editorProps.persistWorkspace;
    if (workspace === undefined) {
      setPendingAcceptances([]);
      setAcceptanceChecked(true);
      return () => { active = false; };
    }
    void Promise.all(reads.map(async ({ locator, proposal }) => {
      const problem = await knownProblemDisplayedAcceptance(workspace, locator.proposalId);
      if (active) {
        setKnownProblems((current) => {
          const next = { ...current };
          if (problem === undefined) delete next[locator.proposalId];
          else next[locator.proposalId] = problem.status;
          return next;
        });
      }
      const settled = await settledDisplayedAcceptance(workspace, locator.proposalId);
      if (settled !== undefined && active) {
        const message = settled.kind === "refused"
          ? "上次接受请求已被拒绝，候选文字仍保留。"
          : {
            applied: "已接受，正文已更新。",
            invalid: "候选文字的验证已失效，请检查当前结果。",
            conflicted: "正文已变化，候选文字尚未接受。",
            refused: "上次接受已被拒绝，候选文字仍保留。",
          }[settled.response.effect.kind];
        setDecisionMessages((current) => ({ [locator.proposalId]: message, ...current }));
      }
      if (proposal === undefined) return await hasPendingDisplayedAcceptance(workspace,
        locator.proposalId) ? locator.proposalId : undefined;
      const result = await reconcileDisplayedAcceptance(workspace, proposal);
      if (result === "applied" && !refreshedAcceptance.current.has(proposal.proposal_id)) {
        refreshedAcceptance.current.add(proposal.proposal_id);
        await onAccepted();
      }
      return result === "pending" || result === "applied" ? proposal.proposal_id : undefined;
    })).then((values) => {
      if (active) setPendingAcceptances(values.filter((value) => value !== undefined));
    }).catch(() => {
      if (active) setPendingAcceptances(reads.map(({ locator }) => locator.proposalId));
    }).finally(() => {
      if (active) setAcceptanceChecked(true);
    });
    return () => { active = false; };
  }, [reads, editorProps.persistWorkspace]);

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
    const pendingAcceptance = pendingAcceptances.includes(proposal.proposal_id);
    const eligible = allHeadsKnown && acceptanceChecked && editorProps.editable
      && proposal.generation === "ready" && proposal.validation === "valid"
      && proposal.closure === "open" && operation.resolution === "pending"
      && operation.reservation_state === "unresolved"
      && proposal.validation_receipt.kind === "present"
      && proposal.validation_receipt.result === "valid"
      && candidateTexts[`${proposal.proposal_id}:${proposal.revision_id}`] === undefined
      && accepting !== proposal.proposal_id && !pendingAcceptance;
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
      retryPending: pendingAcceptance && knownProblems[proposal.proposal_id] === undefined
        && accepting !== proposal.proposal_id,
      expectedHeads,
      localPending: candidateTexts[`${proposal.proposal_id}:${proposal.revision_id}`]
        !== undefined,
    });
  }

  const acceptDisplayed = (target: {
    proposalId: string;
    operationId: string;
    revisionId: string;
    text: string;
  }) => {
    if (acceptingRef.current) return;
    const displayed = reads.find(({ proposal }) => proposal?.proposal_id === target.proposalId)?.proposal;
    const receipt = displayed?.validation_receipt;
    const projection = projections.find((item) => item.proposalId === target.proposalId);
    const workspace = editorProps.persistWorkspace;
    if (displayed === undefined || (projection?.eligible !== true
        && projection?.retryPending !== true)
      || displayed.revision_id !== target.revisionId
      || displayed.candidate_text !== target.text
      || receipt?.kind !== "present"
      || receipt.result !== "valid"
      || displayed.operations.find((item) => item.operation_id === target.operationId
        && item.manuscript_block_id === projection.blockId) === undefined
      || workspace === undefined) {
      setDecisionMessages((current) => ({ ...current,
        [target.proposalId]: "候选文字已变化。请检查当前版本。" }));
      setSettlementRefresh((value) => value + 1);
      return;
    }
    acceptingRef.current = true;
    setAccepting(target.proposalId);
    void (async () => {
      try {
        if (editorProps.controllerRef.current?.hasIncompleteSemanticIntent()) {
          throw new Error("请先完成当前输入。");
        }
        await editorProps.controllerRef.current?.flush();
        if (workspace.partition.disposition !== "current_writer_open"
          || (!pendingAcceptances.includes(target.proposalId)
            && (workspace.pending.save_state !== "saved"
              || workspace.pending.unsettled_intent_count !== 0))) {
          throw new Error("请先保存候选文字。");
        }
        const current = await getProposal({
          baseUrl: editorProps.baseUrl,
          fetchImpl: editorProps.fetchImpl,
          projectId: scope.project_id,
          proposalId: target.proposalId,
        });
        if (current.project_scope.owner_user_id !== scope.owner_user_id
          || current.project_scope.project_id !== scope.project_id
          || current.proposal.source.run_id !== displayed.source.run_id
          || current.proposal.source.decision_id !== displayed.source.decision_id) {
          throw new Error("候选文字的项目身份已变化。");
        }
        const response = await acceptDisplayedBlockProposal({
          baseUrl: editorProps.baseUrl,
          fetchImpl: editorProps.fetchImpl,
          cryptoImpl: editorProps.cryptoImpl,
          workspace,
          proposalId: target.proposalId,
          operationId: target.operationId,
          proposalRevisionId: target.revisionId,
          validationReceiptId: receipt.validation_receipt_id,
          authoritativeRevisionId,
        });
        if (response.project_scope.owner_user_id !== scope.owner_user_id
          || response.project_scope.project_id !== scope.project_id
          || response.receipt.proposal_id !== target.proposalId
          || response.receipt.proposal_revision_id !== target.revisionId
          || response.receipt.selected_operation_ids.length !== 1
          || response.receipt.selected_operation_ids[0] !== target.operationId) {
          throw new Error("接受结果的身份不匹配。");
        }
        const message = {
          applied: "已接受，正文已更新。",
          invalid: "候选文字的验证已失效，请检查当前结果。",
          conflicted: "正文已变化，候选文字尚未接受。",
          refused: "此次接受已被拒绝，候选文字仍保留。",
        }[response.effect.kind];
        setDecisionMessages((current) => ({ ...current, [target.proposalId]: message }));
        setSettlementRefresh((value) => value + 1);
        try { await onAccepted(); } catch {
          setDecisionMessages((current) => ({ ...current,
            [target.proposalId]: "接受结果已记录。请刷新查看当前正文。" }));
        }
      } catch (error) {
        setDecisionMessages((current) => ({ ...current,
          [target.proposalId]: historicalAcknowledgementUnavailable(error)
            ? HISTORICAL_ACKNOWLEDGEMENT_MESSAGE
            : error instanceof Error && error.message.startsWith("请先")
              ? error.message : "接受结果暂不可确认。请刷新检查，或重试同一操作。" }));
        setSettlementRefresh((value) => value + 1);
        try { await onAccepted(); } catch { /* The frozen command remains available. */ }
      } finally {
        acceptingRef.current = false;
        setAccepting(undefined);
      }
    })();
  };

  const retryPending = (proposalId: string) => {
    const workspace = editorProps.persistWorkspace;
    if (acceptingRef.current || workspace === undefined) return;
    acceptingRef.current = true;
    setAccepting(proposalId);
    void (async () => {
      try {
        const response = await retryPendingDisplayedAcceptance({
          baseUrl: editorProps.baseUrl,
          fetchImpl: editorProps.fetchImpl,
          cryptoImpl: editorProps.cryptoImpl,
          workspace,
          proposalId,
        });
        if (response.project_scope.owner_user_id !== scope.owner_user_id
          || response.project_scope.project_id !== scope.project_id
          || response.receipt.proposal_id !== proposalId) {
          throw new Error("Acceptance result identity changed");
        }
        await onAccepted();
        setSettlementRefresh((value) => value + 1);
      } catch {
        setDecisionMessages((current) => ({ ...current,
          [proposalId]: "接受结果暂不可确认。请重试同一操作。" }));
        setSettlementRefresh((value) => value + 1);
        try { await onAccepted(); } catch { /* The frozen command remains available. */ }
      } finally {
        acceptingRef.current = false;
        setAccepting(undefined);
      }
    })();
  };

  return (
    <>
      <ManuscriptEditor {...editorProps}
        editable={editorProps.editable && acceptanceChecked && accepting === undefined
          && pendingAcceptances.length === 0}
        proposals={projections}
        onCandidateSettled={() => setSettlementRefresh((value) => value + 1)}
        onAcceptProposal={acceptDisplayed} />
      {reads.map(({ locator, proposal }) => {
        const message = proposal?.operation_resolution === "applied"
          ? !acceptanceChecked ? "正在同步正文。"
            : pendingAcceptances.includes(locator.proposalId)
              ? "正文已变化；此次接受结果尚未确认。请重试同一操作。"
            : decisionMessages[locator.proposalId]?.includes("请刷新")
            ? decisionMessages[locator.proposalId] : "已接受，正文已更新。"
          : proposal?.validation === "conflicted" ? "正文已变化，候选文字尚未接受。"
          : proposal?.validation === "invalid" ? "候选文字的验证已失效，请检查当前结果。"
          : proposal?.latest_acceptance_refusal.kind === "present"
            ? {
              stale_writer: "上次接受因编辑会话失效而被拒绝，候选文字仍保留。",
              session_changed: "上次接受因编辑会话变化而被拒绝，候选文字仍保留。",
              invalid_challenge: "上次接受请求已被拒绝，候选文字仍保留。",
            }[proposal.latest_acceptance_refusal.reason]
            : knownProblems[locator.proposalId] !== undefined
              ? `接受请求返回 HTTP ${knownProblems[locator.proposalId]}；候选文字仍保留。请检查当前结果。`
            : decisionMessages[locator.proposalId];
        return message === undefined ? null : (
          <p data-proposal-decision={locator.proposalId} role="status" key={locator.proposalId}>
            {message}
            {pendingAcceptances.includes(locator.proposalId)
              && knownProblems[locator.proposalId] === undefined
              && proposal?.operation_resolution === "applied" ? (
                <button type="button" disabled={accepting !== undefined}
                  onClick={() => retryPending(locator.proposalId)}>重试接受</button>
              ) : null}
          </p>
        );
      })}
      {unavailable.map(({ locator, proposal }) => (
        <p className="block-proposal-unavailable" data-proposal-unavailable={locator.proposalId}
          data-proposal-revision-id={proposal?.revision_id ?? ""}
          data-proposal-operation-id={proposal?.operation_id ?? ""}
          data-proposal-source-run-id={proposal?.source.run_id ?? locator.runId}
          data-proposal-source-decision-id={proposal?.source.decision_id ?? locator.decisionId}
          data-proposal-eligibility="unavailable"
          key={locator.proposalId}>
          {proposal?.operation_resolution === "applied"
            ? !acceptanceChecked ? "正在同步正文。"
              : pendingAcceptances.includes(locator.proposalId)
                ? "正文已变化；此次接受结果尚未确认。" : "已接受，正文已更新。"
            : "候选文字暂不可用，请检查当前章节和正文。"}
        </p>
      ))}
    </>
  );
}
