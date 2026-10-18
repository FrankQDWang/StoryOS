import { inlineProjectionAnchor } from "./inline-proposal-decoration.ts";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";

import { getProposal, listChapterOpenProposals } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  BlockProposalInspect, ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { ManuscriptEditor, type ManuscriptEditorProps } from "./manuscript-editor.tsx";
import type { EditorReadyState } from "./editor-types.ts";
import { RefusedEditDraftDisplay } from "./refused-edit-draft-display.tsx";
import type { BlockProposalProjection } from "./block-proposal-decoration.ts";
import { canonicalDraftValue as canonical } from "./refused-edit-discard.ts";
import { readExpansionJournal, retryExpansion } from "./refused-edit-expansion.ts";
import { acceptDisplayedBlockProposal, retryPendingDisplayedAcceptance } from "./accept-block-proposal.ts";
import { acceptDraftCandidates } from "./selection-reference-draft.ts";
import { rejectDisplayedBlockProposal, rejectionJournalState,
  retryPendingDisplayedRejection } from "./reject-block-proposal.ts";
import { acceptanceJournalProposals, acceptanceSessionBlocked, hasPendingDisplayedAcceptance,
  knownProblemDisplayedAcceptance, reconcileDisplayedAcceptance,
  settledDisplayedAcceptance } from "./acceptance-journal.ts";
import { proposalConditionKind } from "./proposal-recovery-surface.ts";
import { dropStoredReplans } from "./stored-replan-records.ts";
import { authorWithdrawControlReady, dispatchDisplayedWithdraw, dispatchDisplayedWithdrawRetry,
  pendingWithdrawIds } from "./withdraw-block-proposal.ts";
import { ProposalDecisionStatus } from "./proposal-decision-status.tsx";
import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE, historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";

const ignoreChanges = () => () => {};

export type ProposalLocator = {
  proposalId: string;
  runId: string;
  decisionId: string;
};

// The Server list is the source of the open Proposals of a Chapter. A failed or truncated list leaves Proposal heads unknown.
type OpenProposalList = { kind: "reading" } | { kind: "failed" }
  | { kind: "listed"; complete: boolean; locators: ProposalLocator[] };

async function readOpenProposalList(scope: ProjectScope, chapterId: string, baseUrl: string,
  fetchImpl: typeof fetch): Promise<OpenProposalList> {
  try {
    const response = await listChapterOpenProposals({ baseUrl, fetchImpl, projectId: scope.project_id, chapterId });
    if (response.project_scope.owner_user_id !== scope.owner_user_id
      || response.project_scope.project_id !== scope.project_id || response.chapter_id !== chapterId) {
      return { kind: "failed" };
    }
    return { kind: "listed", complete: response.completeness === "complete",
      locators: response.proposals.map(({ proposal_id, source }) => source.kind === "agent_run_decision"
        ? { proposalId: proposal_id, runId: source.run_id, decisionId: source.decision_id }
        : { proposalId: proposal_id, runId: "", decisionId: "" }) };
  } catch {
    return { kind: "failed" };
  }
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
  persistWorkspace: EditorReadyState | undefined;
  onFailure: (error: unknown) => void;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}) {
  const [reads, setReads] = useState<ProposalRead[]>([]);
  const writing = useSyncExternalStore(editorProps.writing?.subscribe ?? ignoreChanges,
    () => editorProps.writing?.snapshot());
  const settlements = writing?.settlements ?? 0;
  // The writing controller supplies the candidate text, also input that it holds (ADR 0046).
  const candidateText = (proposal: BlockProposalInspect, operationId: string) => {
    const shown = writing?.candidates.get(`${proposal.proposal_id}:${operationId}`);
    return shown?.revisions.includes(proposal.revision_id) === true ? shown.text : undefined;
  };
  const [discardHold, setDiscardHold] = useState(false);
  const [settlementRefresh, setSettlementRefresh] = useState(0);
  const [decisionMessages, setDecisionMessages] = useState<Record<string, string>>({});
  const [knownProblems, setKnownProblems] = useState<Record<string, {
    status: number; code: string; message: string; terminal: boolean;
  }>>({});
  const [accepting, setAccepting] = useState<string>();
  const [pendingAcceptances, setPendingAcceptances] = useState<string[]>([]);
  const [decisionGeneration, setDecisionGeneration] = useState(0);
  const [acceptanceCheck, setAcceptanceCheck] = useState<{
    workspace: EditorReadyState | undefined; proposalIds: string; generation: number;
  }>();
  const [recoveredProposalIds, setRecoveredProposalIds] = useState<string[]>([]);
  const [openList, setOpenList] = useState<OpenProposalList>({ kind: "reading" });
  const [journalPendingIds, setJournalPendingIds] = useState<string[]>([]);
  const [pendingRejections, setPendingRejections] = useState<string[]>([]);
  const [pendingWithdrawals, setPendingWithdrawals] = useState<string[]>([]);
  const [sessionBlockedIds, setSessionBlockedIds] = useState<string[]>([]);
  const [settledRejections, setSettledRejections] = useState<Record<string,
    "resolved" | "conflicted" | "refused">>({});
  const [recoveryUnavailable, setRecoveryUnavailable] = useState(false);
  const [recoveryCheck, setRecoveryCheck] = useState<{ workspace: EditorReadyState; generation: number }>();
  const refreshedAcceptance = useRef(new Set<string>());
  const acceptingRef = useRef(false);
  const knownLocators = [...locators, ...(openList.kind === "listed" ? openList.locators : []).filter((item) =>
    !locators.some((locator) => locator.proposalId === item.proposalId))];
  const effectiveLocators = [...knownLocators, ...recoveredProposalIds.filter((id) =>
    !knownLocators.some((locator) => locator.proposalId === id)).map((proposalId) =>
    ({ proposalId, runId: "", decisionId: "" }))];
  const locatorKey = effectiveLocators.map((item) =>
    `${item.proposalId}:${item.runId}:${item.decisionId}`).join("|");
  const proposalIdsKey = (items: readonly { proposalId: string }[]) =>
    [...new Set(items.map((item) => item.proposalId))].sort().join("|");
  // A re-check keeps the last result. After a new workspace, a new Proposal, or a completed decision, a new check is necessary.
  const recoveryChecked = editorProps.persistWorkspace === undefined
    || (recoveryCheck?.workspace === editorProps.persistWorkspace
      && recoveryCheck.generation === decisionGeneration);
  const acceptanceChecked = acceptanceCheck !== undefined
    && acceptanceCheck.workspace === editorProps.persistWorkspace
    && acceptanceCheck.proposalIds === proposalIdsKey(effectiveLocators)
    && acceptanceCheck.generation === decisionGeneration;
  // Only a Proposal that the Local Edit Journal knows can hold a local Acceptance. A Proposal that a later read adds does not stop text input.
  const journalProposalsChecked = acceptanceCheck !== undefined
    && acceptanceCheck.workspace === editorProps.persistWorkspace
    && acceptanceCheck.generation === decisionGeneration
    && recoveredProposalIds.every((id) => acceptanceCheck.proposalIds.split("|").includes(id));
  const finishDecision = () => {
    acceptingRef.current = false;
    setAccepting(undefined);
    setDecisionGeneration((value) => value + 1);
  };

  useEffect(() => {
    let active = true;
    const workspace = editorProps.persistWorkspace;
    if (workspace === undefined) return () => { active = false; };
    const generation = decisionGeneration;
    void Promise.all([acceptanceJournalProposals(workspace), rejectionJournalState(workspace),
      readExpansionJournal(workspace), pendingWithdrawIds(workspace), dropStoredReplans(workspace)])
      .then(async ([acceptance, rejection, expansions, withdrawals]) => {
      for (const entry of expansions) if (entry.observation === undefined) {
        const response = await retryExpansion(workspace, entry.record, editorProps.baseUrl, editorProps.fetchImpl, () => active);
        entry.observation = { key: `expansion-observation:${entry.record.key}`, record_key: entry.record.key, response };
        await onAccepted();
      }
      if (!active) return;
      setRecoveredProposalIds([...new Set([...acceptance.proposalIds,
        ...rejection.proposalIds, ...expansions.flatMap(({ observation }) =>
          observation?.response.effect.kind === "proposal_created_from_draft" ? [observation.response.effect.proposal_id] : [])])]);
      setJournalPendingIds([...new Set([...acceptance.unresolvedIds,
        ...rejection.pendingIds])]);
      setPendingRejections(rejection.pendingIds);
      setPendingWithdrawals(withdrawals);
      setSettledRejections(rejection.settledResults);
      setRecoveryUnavailable(false);
    }).catch(() => {
      if (active) setRecoveryUnavailable(true);
    }).finally(() => {
      if (active) setRecoveryCheck({ workspace, generation });
    });
    return () => { active = false; };
  }, [editorProps.persistWorkspace, settlementRefresh, settlements, decisionGeneration]);

  useEffect(() => {
    let active = true;
    void readOpenProposalList(scope, chapterId, editorProps.baseUrl, editorProps.fetchImpl).then((next) => {
      if (active) setOpenList((current) => canonical(current) === canonical(next) ? current : next);
    });
    return () => { active = false; };
  }, [scope.owner_user_id, scope.project_id, chapterId, refreshKey, settlementRefresh, settlements,
    decisionGeneration, locators.map((locator) => locator.proposalId).join("|"),
    editorProps.baseUrl, editorProps.fetchImpl]);

  useEffect(() => {
    let active = true;
    void Promise.all(effectiveLocators.map(async (locator): Promise<ProposalRead> => {
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
          || (locator.runId !== "" && (response.proposal.source.kind !== "agent_run_decision"
            || response.proposal.source.run_id !== locator.runId))
          || (locator.decisionId !== ""
            && (response.proposal.source.kind !== "agent_run_decision"
              || response.proposal.source.decision_id !== locator.decisionId))) {
          return { locator };
        }
        return { locator: locator.runId === "" && response.proposal.source.kind === "agent_run_decision" ? {
          proposalId: locator.proposalId,
          runId: response.proposal.source.run_id,
          decisionId: response.proposal.source.decision_id,
        } : locator, proposal: response.proposal };
      } catch {
        return { locator };
      }
    })).then((result) => {
      // Navigation can reorder the same locators. If the result does not change, no new Acceptance check is necessary.
      const unordered = (items: readonly ProposalRead[]) => canonical([...items].sort((left, right) =>
        left.locator.proposalId.localeCompare(right.locator.proposalId)));
      if (active) setReads((current) => unordered(current) === unordered(result) ? current : result);
    });
    return () => { active = false; };
  }, [scope.owner_user_id, scope.project_id, chapterId, locatorKey, refreshKey,
    settlementRefresh, settlements,
    editorProps.baseUrl, editorProps.fetchImpl]);

  useEffect(() => {
    let active = true;
    if (!recoveryChecked || reads.length !== effectiveLocators.length) {
      return () => { active = false; };
    }
    const workspace = editorProps.persistWorkspace;
    const checked = { workspace, proposalIds: proposalIdsKey(reads.map(({ locator }) => locator)),
      generation: decisionGeneration };
    if (workspace === undefined) {
      setPendingAcceptances([]);
      setSessionBlockedIds([]);
      setAcceptanceCheck(checked);
      return () => { active = false; };
    }
    const blockedIds: string[] = [];
    void Promise.all(reads.map(async ({ locator, proposal }) => {
      if (await acceptanceSessionBlocked(workspace, locator.proposalId)) {
        blockedIds.push(locator.proposalId);
      }
      const problem = await knownProblemDisplayedAcceptance(workspace, locator.proposalId);
      if (active) {
        setKnownProblems((current) => {
          const next = { ...current };
          if (problem === undefined) delete next[locator.proposalId];
          else next[locator.proposalId] = problem;
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
      if (active && !acceptingRef.current && result === "applied"
        && !refreshedAcceptance.current.has(proposal.proposal_id)) {
        refreshedAcceptance.current.add(proposal.proposal_id);
        await onAccepted();
      }
      return result === "pending" || result === "applied" ? proposal.proposal_id : undefined;
    })).then((values) => {
      if (!active) return;
      setSessionBlockedIds([...new Set(blockedIds)]);
      setPendingAcceptances([...new Set([...journalPendingIds.filter((id) =>
        !pendingRejections.includes(id)),
        ...values.filter((value) => value !== undefined)])]);
    }).catch(() => {
      if (active) setPendingAcceptances(reads.map(({ locator }) => locator.proposalId));
    }).finally(() => {
      if (active) setAcceptanceCheck(checked);
    });
    return () => { active = false; };
  }, [reads, editorProps.persistWorkspace, recoveryChecked, decisionGeneration,
    journalPendingIds.join("|"), pendingRejections.join("|")]);

  const blockCounts = new Map<string, number>();
  for (const block of editorProps.blocks) {
    blockCounts.set(block.manuscript_block_id,
      (blockCounts.get(block.manuscript_block_id) ?? 0) + 1);
  }
  const projections: BlockProposalProjection[] = [];
  const unavailable: ProposalRead[] = [];
  const allHeadsKnown = recoveryChecked && openList.kind === "listed" && openList.complete
    && reads.length === effectiveLocators.length
    && reads.every((item) => item.proposal !== undefined);
  const expectedHeads = reads.flatMap(({ proposal }) => proposal?.chapter_id === chapterId && proposal.closure === "open"
      && proposal.operations.some((item) => item.resolution === "pending" && item.reservation_state === "unresolved")
    ? [proposal.revision_id] : []).sort();
  const pendingOperations = reads.flatMap<ProposalRead & { operation: BlockProposalInspect["operations"][number] | undefined }>(({ locator, proposal }) => proposal === undefined
    ? [{ locator, proposal, operation: undefined }]
    : proposal.operations.filter((item) => item.resolution === "pending")
      .map((operation) => ({ locator, proposal, operation })));
  // The complete Server list decides which Proposals the editor shows. A locator only causes an earlier list read (ADR 0063).
  const listedIds = openList.kind === "listed" && openList.complete
    ? new Set(openList.locators.map((item) => item.proposalId)) : undefined;
  for (const { locator, proposal, operation } of pendingOperations) {
    if (proposal !== undefined && proposal.chapter_id !== chapterId) continue;
    if (listedIds !== undefined && !listedIds.has(locator.proposalId)) continue;
    const condition = proposal === undefined ? "absent" : proposalConditionKind(proposal);
    const anchored = proposal !== undefined
      && (proposal.kind === "block_edit" || proposal.kind === "reversal" || proposal.kind === "inline_edit")
      && operation !== undefined
      && blockCounts.get(operation?.manuscript_block_id ?? "") === 1 && safeToProject;
    const baseMatches = proposal?.base_authoritative_revision_id === authoritativeRevisionId;
    // Acceptance of a Proposal on an earlier base replaces the current text of its target (ADR 0064).
    const acceptableValidation = (result: string) => result === "valid" || result === "conflicted";
    const inlineAnchor = proposal === undefined ? undefined
      : inlineProjectionAnchor(proposal, editorProps.blocks, authoritativeRevisionId);
    if (!anchored || proposal === undefined || operation === undefined) {
      unavailable.push({ locator, proposal });
      continue;
    }
    const writerOpen = editorProps.persistWorkspace?.partition.disposition === "current_writer_open"
      && editorProps.persistWorkspace.session.writer.kind === "current_writer";
    const sessionBlocked = sessionBlockedIds.includes(proposal.proposal_id);
    const pendingAcceptance = pendingAcceptances.includes(proposal.proposal_id);
    const localText = candidateText(proposal, operation.operation_id);
    const controlsReady = allHeadsKnown && acceptanceChecked && !recoveryUnavailable
      && journalPendingIds.length === 0 && writerOpen && !sessionBlocked
      && editorProps.editable && accepting !== proposal.proposal_id;
    const problem = knownProblems[proposal.proposal_id];
    const eligible = controlsReady
      && proposal.generation === "ready" && acceptableValidation(proposal.validation)
      && condition !== "proposal_recovery_conflict"
      && (proposal.kind !== "inline_edit" || inlineAnchor !== undefined || !baseMatches)
      && proposal.closure === "open" && operation.resolution === "pending"
      && operation.reservation_state === "unresolved"
      && proposal.validation_receipt.kind === "present"
      && acceptableValidation(proposal.validation_receipt.result)
      && (problem === undefined || problem.code === "acceptance_session_ineligible")
      && !pendingAcceptance;
    const rejectEligible = controlsReady && proposal.closure === "open" && operation.resolution === "pending";
    const withdrawEligible = authorWithdrawControlReady({
      controlsReady, condition, closure: proposal.closure, resolution: operation.resolution,
      pendingAcceptance, pendingRejection: pendingRejections.includes(proposal.proposal_id),
      pendingWithdraw: pendingWithdrawals.includes(proposal.proposal_id),
    });
    projections.push({
      inlineProposal: proposal.kind === "inline_edit",
      ...(baseMatches && inlineAnchor !== undefined ? { inlineAnchor } : {}),
      proposalId: proposal.proposal_id,
      operationId: operation.operation_id,
      focused: editorProps.focusProposal?.proposalId === proposal.proposal_id
        && editorProps.focusProposal.operationId === operation.operation_id
        && editorProps.focusProposal.revisionId === proposal.revision_id,
      pendingOperationIds: proposal.operations.filter((item) => item.resolution === "pending").map((item) => item.operation_id).sort(),
      revisionId: proposal.revision_id,
      blockId: operation.manuscript_block_id,
      sourceRunId: proposal.source.kind === "agent_run_decision" ? proposal.source.run_id : "",
      sourceDecisionId: proposal.source.kind === "agent_run_decision" ? proposal.source.decision_id : "",
      text: localText ?? operation.candidate_text,
      eligible,
      candidateEditable: rejectEligible && condition !== "proposal_recovery_conflict"
        && proposal.generation === "ready" && proposal.validation !== "invalid"
        && (proposal.kind !== "inline_edit" || inlineAnchor !== undefined || !baseMatches) && !pendingAcceptance,
      retryPending: pendingAcceptance && knownProblems[proposal.proposal_id] === undefined
        && accepting !== proposal.proposal_id,
      rejectEligible,
      retryRejection: pendingRejections.includes(proposal.proposal_id)
        && accepting !== proposal.proposal_id,
      earlierBase: eligible && !baseMatches,
      withdrawEligible,
      copyEligible: operation.candidate_text.length > 0
        && (condition !== "absent" || proposal.validation === "invalid" || !writerOpen || sessionBlocked),
      conditionKind: condition,
      validity: proposal.validation,
      sessionEligible: writerOpen && !sessionBlocked,
      expectedHeads,
      localPending: localText !== undefined,
    });
  }

  if (editorProps.writing !== undefined) {
    editorProps.writing.setInlineProposalAnchors(reads.flatMap(({ proposal }) =>
      proposal?.chapter_id === chapterId && proposal.kind === "inline_edit"
        && expectedHeads.length === 1 && expectedHeads[0] === proposal.revision_id
        && inlineProjectionAnchor(proposal, editorProps.blocks, authoritativeRevisionId) !== undefined
        ? proposal.anchors.map(({ manuscript_block_id, coordinate_profile, from, to, base_slice_digest }) =>
          ({ manuscript_block_id, coordinate_profile, from, to, base_slice_digest })) : []));
  }

  const acceptDisplayed = (target: {
    proposalId: string;
    operationId: string;
    revisionId: string;
    text: string;
    operationIds?: readonly string[] | undefined;
  }) => {
    if (acceptingRef.current) return;
    const displayed = reads.find(({ proposal }) => proposal?.proposal_id === target.proposalId)?.proposal;
    const receipt = displayed?.validation_receipt;
    const projection = projections.find((item) => item.proposalId === target.proposalId
      && item.operationId === target.operationId);
    const workspace = editorProps.persistWorkspace;
    if (displayed === undefined || (projection?.eligible !== true
        && projection?.retryPending !== true)
      || displayed.revision_id !== target.revisionId
      || projection.text !== target.text
      || receipt?.kind !== "present"
      || (receipt.result !== "valid" && receipt.result !== "conflicted")
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
        if (editorProps.writing?.hasIncompleteInput()) throw new Error("请先完成当前输入。");
        await editorProps.writing?.flush();
        const projection = editorProps.writing?.snapshot().projection;
        if (workspace.partition.disposition !== "current_writer_open"
          || (!pendingAcceptances.includes(target.proposalId)
            && (projection?.save_state !== "saved" || projection.unsettled_intent_count !== 0))) {
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
          || canonical(current.proposal.source) !== canonical(displayed.source)) {
          throw new Error("候选文字的项目身份已变化。");
        }
        const response = await acceptDisplayedBlockProposal({
          baseUrl: editorProps.baseUrl,
          fetchImpl: editorProps.fetchImpl,
          cryptoImpl: editorProps.cryptoImpl,
          workspace,
          proposalId: target.proposalId,
          operationId: target.operationId,
          operationIds: target.operationIds,
          proposalRevisionId: target.revisionId,
          validationReceiptId: receipt.validation_receipt_id,
          authoritativeRevisionId,
        });
        if (response.project_scope.owner_user_id !== scope.owner_user_id
          || response.project_scope.project_id !== scope.project_id
          || response.receipt.proposal_id !== target.proposalId
          || response.receipt.proposal_revision_id !== target.revisionId
          || JSON.stringify(response.receipt.selected_operation_ids)
            !== JSON.stringify(target.operationIds ?? [target.operationId])) {
          throw new Error("接受结果的身份不匹配。");
        }
        const message = {
          applied: "已接受，正文已更新。",
          invalid: "候选文字的验证已失效，请检查当前结果。",
          conflicted: "正文已变化，候选文字尚未接受。",
          refused: "此次接受已被拒绝，候选文字仍保留。",
        }[response.effect.kind];
        const refresh = response.effect.kind !== "applied" || !refreshedAcceptance.current.has(target.proposalId);
        if (response.effect.kind === "applied") {
          refreshedAcceptance.current.add(target.proposalId);
          // A reference to the accepted candidate text now refers to the same text in the Chapter (ADR 0062).
          acceptDraftCandidates(scope, target.proposalId, response.receipt.selected_operation_ids,
            response.effect.authoritative_revision.revision_id);
        }
        setDecisionMessages((current) => ({ ...current, [target.proposalId]: message }));
        setSettlementRefresh((value) => value + 1);
        try { if (refresh) await onAccepted(); } catch {
          setDecisionMessages((current) => ({ ...current,
            [target.proposalId]: "接受结果已记录。请刷新查看当前正文。" }));
        }
      } catch (error) {
        setDecisionMessages((current) => ({ ...current,
          [target.proposalId]: historicalAcknowledgementUnavailable(error)
            ? HISTORICAL_ACKNOWLEDGEMENT_MESSAGE
            : error instanceof Error && error.message === "Acceptance session is closed"
              ? "请先恢复写作会话。系统不会自动提交。"
            : error instanceof Error && error.message.startsWith("请先")
              ? error.message : "接受结果暂不可确认。请刷新检查，或重试同一操作。" }));
        setSettlementRefresh((value) => value + 1);
        try { await onAccepted(); } catch { /* The frozen command remains available. */ }
      } finally {
        finishDecision();
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
        const refresh = response.effect.kind !== "applied" || !refreshedAcceptance.current.has(proposalId);
        if (response.effect.kind === "applied") {
          refreshedAcceptance.current.add(proposalId);
          acceptDraftCandidates(scope, proposalId, response.receipt.selected_operation_ids,
            response.effect.authoritative_revision.revision_id);
        }
        if (refresh) await onAccepted();
        setSettlementRefresh((value) => value + 1);
      } catch {
        setDecisionMessages((current) => ({ ...current,
          [proposalId]: "接受结果暂不可确认。请重试同一操作。" }));
        setSettlementRefresh((value) => value + 1);
        try { await onAccepted(); } catch { /* The frozen command remains available. */ }
      } finally {
        finishDecision();
      }
    })();
  };

  const rejectDisplayed = (target: {
    proposalId: string; operationId: string; revisionId: string; text: string; operationIds?: readonly string[] | undefined;
  }) => {
    if (acceptingRef.current) return;
    if (pendingRejections.includes(target.proposalId)) {
      retryRejection(target.proposalId);
      return;
    }
    const displayed = reads.find(({ proposal }) => proposal?.proposal_id === target.proposalId)?.proposal;
    const projection = projections.find((item) => item.proposalId === target.proposalId
      && item.operationId === target.operationId);
    const workspace = editorProps.persistWorkspace;
    if (displayed === undefined || workspace === undefined
      || projection?.rejectEligible !== true
      || displayed.revision_id !== target.revisionId
      || projection.text !== target.text
      || displayed.operations.find((item) => item.operation_id === target.operationId
        && item.manuscript_block_id === projection.blockId) === undefined) {
      setDecisionMessages((current) => ({ ...current,
        [target.proposalId]: "候选文字已变化。请检查当前版本。" }));
      setSettlementRefresh((value) => value + 1);
      return;
    }
    acceptingRef.current = true;
    setAccepting(target.proposalId);
    void (async () => {
      try {
        if (editorProps.writing?.hasIncompleteInput()) throw new Error("请先完成当前输入。");
        await editorProps.writing?.flush();
        const projection = editorProps.writing?.snapshot().projection;
        if (workspace.partition.disposition !== "current_writer_open"
          || projection?.save_state !== "saved" || projection.unsettled_intent_count !== 0) {
          throw new Error("请先保存候选文字。");
        }
        const current = await getProposal({ baseUrl: editorProps.baseUrl,
          fetchImpl: editorProps.fetchImpl, projectId: scope.project_id,
          proposalId: target.proposalId });
        if (current.project_scope.owner_user_id !== scope.owner_user_id
          || current.project_scope.project_id !== scope.project_id
          || current.proposal.revision_id !== target.revisionId
          || canonical(current.proposal.source) !== canonical(displayed.source)
          || current.proposal.operations.find((item) => item.operation_id === target.operationId
            && item.resolution === "pending") === undefined) {
          throw new Error("候选文字的身份已变化。");
        }
        const response = await rejectDisplayedBlockProposal({ baseUrl: editorProps.baseUrl,
            fetchImpl: editorProps.fetchImpl, cryptoImpl: editorProps.cryptoImpl,
            workspace, proposalId: target.proposalId, operationId: target.operationId, operationIds: target.operationIds,
            proposalRevisionId: target.revisionId, targetRevisionId: authoritativeRevisionId });
        if (response.project_scope.owner_user_id !== scope.owner_user_id
          || response.project_scope.project_id !== scope.project_id
          || response.receipt.proposal_id !== target.proposalId
          || response.receipt.proposal_revision_id !== target.revisionId
          || JSON.stringify(response.receipt.selected_pending_operation_ids)
            !== JSON.stringify(target.operationIds ?? [target.operationId])) {
          throw new Error("拒绝结果的身份不匹配。");
        }
        const message = { resolved: "已拒绝，正文保持不变。",
          conflicted: "正文已变化，拒绝结果请检查。",
          refused: "此次拒绝未生效，请检查当前候选文字。" }[response.effect.kind];
        setDecisionMessages((current) => ({ ...current, [target.proposalId]: message }));
        setSettlementRefresh((value) => value + 1);
        await onAccepted();
      } catch (error) {
        setDecisionMessages((current) => ({ ...current,
          [target.proposalId]: historicalAcknowledgementUnavailable(error)
            ? HISTORICAL_ACKNOWLEDGEMENT_MESSAGE
            : error instanceof Error && error.message.startsWith("请先")
              ? error.message : "拒绝结果暂不可确认。请刷新检查，或重试同一操作。" }));
        setSettlementRefresh((value) => value + 1);
        try { await onAccepted(); } catch { /* The frozen command remains available. */ }
      } finally {
        finishDecision();
      }
    })();
  };

  const retryRejection = (proposalId: string) => {
    const workspace = editorProps.persistWorkspace;
    if (acceptingRef.current || workspace === undefined) return;
    acceptingRef.current = true;
    setAccepting(proposalId);
    void (async () => {
      try {
        const response = await retryPendingDisplayedRejection({
          baseUrl: editorProps.baseUrl, fetchImpl: editorProps.fetchImpl,
          cryptoImpl: editorProps.cryptoImpl, workspace, proposalId,
        });
        if (response.project_scope.owner_user_id !== scope.owner_user_id
          || response.project_scope.project_id !== scope.project_id
          || response.receipt.proposal_id !== proposalId) {
          throw new Error("Rejection result identity changed");
        }
        setDecisionMessages((current) => ({ ...current, [proposalId]: {
          resolved: "已拒绝，正文保持不变。",
          conflicted: "正文已变化，拒绝结果请检查。",
          refused: "此次拒绝未生效，请检查当前候选文字。",
        }[response.effect.kind] }));
        setSettlementRefresh((value) => value + 1);
        await onAccepted();
      } catch {
        setDecisionMessages((current) => ({ ...current,
          [proposalId]: "拒绝结果暂不可确认。请重试同一操作。" }));
        setSettlementRefresh((value) => value + 1);
      } finally {
        finishDecision();
      }
    })();
  };

  const writerReady = () => editorProps.persistWorkspace?.partition.disposition === "current_writer_open"
    && editorProps.persistWorkspace.session.writer.kind === "current_writer";

  const withdrawControl = {
    baseUrl: editorProps.baseUrl, fetchImpl: editorProps.fetchImpl,
    cryptoImpl: editorProps.cryptoImpl, workspace: editorProps.persistWorkspace,
    authoritativeRevisionId,
    markBusy: (proposalId: string) => { acceptingRef.current = true; setAccepting(proposalId); },
    markIdle: finishDecision,
    report: (proposalId: string, message: string) => setDecisionMessages((current) =>
      ({ ...current, [proposalId]: message })),
    refresh: () => setSettlementRefresh((value) => value + 1),
    onAccepted,
  };
  const withdrawDisplayed = (target: {
    proposalId: string; operationId: string; revisionId: string; text: string;
  }) => {
    const displayed = reads.find(({ proposal }) => proposal?.proposal_id === target.proposalId)?.proposal;
    const projection = projections.find((item) => item.proposalId === target.proposalId
      && item.operationId === target.operationId);
    dispatchDisplayedWithdraw({
      ...withdrawControl, target, pending: pendingWithdrawals.includes(target.proposalId),
      busy: acceptingRef.current, displayedRevisionId: displayed?.revision_id,
      eligible: projection?.withdrawEligible === true, writerReady: writerReady(),
    });
  };
  const retryWithdraw = (proposalId: string) => {
    dispatchDisplayedWithdrawRetry({
      ...withdrawControl, proposalId, busy: acceptingRef.current,
    });
  };

  const copyDisplayed = (proposalId: string) => {
    const displayed = reads.find(({ proposal }) => proposal?.proposal_id === proposalId)?.proposal;
    if (displayed === undefined || displayed.candidate_text.length === 0) return;
    void (async () => {
      const current = await getProposal({ baseUrl: editorProps.baseUrl,
        fetchImpl: editorProps.fetchImpl, projectId: scope.project_id, proposalId });
      if (current.project_scope.project_id !== scope.project_id
        || current.proposal.proposal_id !== proposalId) return;
      await navigator.clipboard.writeText(current.proposal.candidate_text);
      setDecisionMessages((current) => ({ ...current, [proposalId]: "已复制候选文字。" }));
    })().catch(() => {
      setDecisionMessages((current) => ({ ...current,
        [proposalId]: "候选文字暂不能复制。请检查当前版本。" }));
    });
  };

  return (
    <>
      <ManuscriptEditor {...editorProps}
        editable={editorProps.editable && !discardHold && !recoveryUnavailable
          && journalPendingIds.length === 0
          && (acceptanceChecked || journalProposalsChecked || (locators.length === 0
            && recoveredProposalIds.length === 0 && writing?.projection.unsettled_intent_count === 0))
          && accepting === undefined
          && pendingAcceptances.length === 0}
        proposals={projections}
        onCandidateSettled={(proposalId) => {
          if (typeof proposalId === "string" && proposalId.length > 0) {
            setRecoveredProposalIds((current) => current.includes(proposalId)
              ? current : [...current, proposalId]);
          }
          setSettlementRefresh((value) => value + 1);
        }}
        onAcceptProposal={acceptDisplayed} onRejectProposal={rejectDisplayed}
        onWithdrawProposal={withdrawDisplayed}
        onCopyProposal={copyDisplayed} />
      <RefusedEditDraftDisplay workspace={editorProps.persistWorkspace} scope={scope}
        baseUrl={editorProps.baseUrl} fetchImpl={editorProps.fetchImpl}
        refreshKey={`${refreshKey}:${settlementRefresh}:${settlements}`} onHoldChange={setDiscardHold}
        onProjection={async () => { await editorProps.writing?.refresh().catch(editorProps.onFailure); }}
        retryDraft={editorProps.writing?.retryDraft}
        onResult={() => setSettlementRefresh((value) => value + 1)} />
      {reads.flatMap(({ proposal }) => proposal?.source.kind === "refused_edit_draft" ? [
        <section className="editor-recovery" key={proposal.proposal_id} data-proposal-id={proposal.proposal_id} aria-label="Draft Proposal">
          <header className="editor-recovery-heading">
            <span>From a preserved edit</span>
            <h2>Proposal from your draft</h2>
            <p>{proposal.closure === "closed"
              ? proposal.operation_resolution === "applied" ? "Accepted into the manuscript." : "This proposal is closed."
              : proposal.validation === "valid" ? "The complete preserved text is ready for review. Accept it to change the manuscript."
                : proposal.validation === "invalid" ? "The proposal needs review before it can be accepted."
                  : "The proposal is waiting for validation before it can be accepted."}</p>
          </header>
          {proposal.candidate_blocks?.map((block, index) => block.block_kind === "heading"
            ? <h3 key={index} data-proposal-structured-block style={{ whiteSpace: "pre-wrap" }}>{block.text}</h3>
            : <p key={index} data-proposal-structured-block style={{ whiteSpace: "pre-wrap" }}>{block.text}</p>)}
        </section>] : [])}
      {recoveryUnavailable ? <p role="alert">接受记录暂不可读取，请检查本地数据。</p> : null}
      {reads.map(({ locator, proposal }) => (
        <ProposalDecisionStatus key={locator.proposalId} proposalId={locator.proposalId}
          proposal={proposal} problem={knownProblems[locator.proposalId]}
          rejectionResult={settledRejections[locator.proposalId]}
          writerOpen={editorProps.persistWorkspace?.partition.disposition === "current_writer_open"
            && editorProps.persistWorkspace.session.writer.kind === "current_writer"}
          sessionBlocked={sessionBlockedIds.includes(locator.proposalId)}
          pendingRejection={pendingRejections.includes(locator.proposalId)}
          pendingWithdraw={pendingWithdrawals.includes(locator.proposalId)}
          pendingAcceptance={pendingAcceptances.includes(locator.proposalId)}
          decisionMessage={decisionMessages[locator.proposalId]}
          acceptanceChecked={acceptanceChecked} retryDisabled={accepting !== undefined}
          onRetryPending={() => retryPending(locator.proposalId)}
          onRetryRejection={() => retryRejection(locator.proposalId)}
          onRetryWithdraw={() => retryWithdraw(locator.proposalId)} />
      ))}
      {unavailable.filter(({ locator, proposal }) => proposal?.operation_resolution !== "applied"
        || !acceptanceChecked || pendingAcceptances.includes(locator.proposalId)).map(({ locator, proposal }) => (
        <p className="block-proposal-unavailable" data-proposal-unavailable={locator.proposalId}
          data-proposal-revision-id={proposal?.revision_id ?? ""}
          data-proposal-operation-id={proposal?.operation_id ?? ""}
          data-proposal-source-run-id={proposal?.source.kind === "agent_run_decision" ? proposal.source.run_id : locator.runId}
          data-proposal-source-decision-id={proposal?.source.kind === "agent_run_decision" ? proposal.source.decision_id : locator.decisionId}
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
