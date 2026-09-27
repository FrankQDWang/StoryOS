import { useEffect, useRef, useState } from "react";

import { getProposal } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  BlockProposalInspect, ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { ManuscriptEditor, type ManuscriptEditorProps } from "./manuscript-editor.tsx";
import { RefusedEditDraftDisplay } from "./refused-edit-draft-display.tsx";
import type { BlockProposalProjection } from "./block-proposal-decoration.ts";
import { candidateProjectionFromJournal } from "./local-edit-journal.ts";
import { canonicalDraftValue as canonical } from "./refused-edit-discard.ts";
import { readExpansionJournal, retryExpansion } from "./refused-edit-expansion.ts";
import { acceptDisplayedBlockProposal, retryPendingDisplayedAcceptance } from "./accept-block-proposal.ts";
import { rejectDisplayedBlockProposal, rejectionJournalState,
  retryPendingDisplayedRejection } from "./reject-block-proposal.ts";
import { acceptanceJournalProposals, acceptanceSessionBlocked, hasPendingDisplayedAcceptance,
  knownProblemDisplayedAcceptance, reconcileDisplayedAcceptance,
  settledDisplayedAcceptance } from "./acceptance-journal.ts";
import { proposalConditionKind, recoveryStatus, type SessionPosture }
  from "./proposal-recovery-surface.ts";
import { pendingReplanIds, replanDisplayedBlockProposal, retryPendingDisplayedReplan }
  from "./replan-block-proposal.ts";
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
  const [discardHold, setDiscardHold] = useState(false);
  const [settlementRefresh, setSettlementRefresh] = useState(0);
  const [decisionMessages, setDecisionMessages] = useState<Record<string, string>>({});
  const [knownProblems, setKnownProblems] = useState<Record<string, {
    status: number; code: string; message: string; terminal: boolean;
  }>>({});
  const [accepting, setAccepting] = useState<string>();
  const [pendingAcceptances, setPendingAcceptances] = useState<string[]>([]);
  const [acceptanceChecked, setAcceptanceChecked] = useState(false);
  const [recoveredProposalIds, setRecoveredProposalIds] = useState<string[]>([]);
  const [journalPendingIds, setJournalPendingIds] = useState<string[]>([]);
  const [pendingRejections, setPendingRejections] = useState<string[]>([]);
  const [pendingReplans, setPendingReplans] = useState<string[]>([]);
  const [sessionBlockedIds, setSessionBlockedIds] = useState<string[]>([]);
  const [settledRejections, setSettledRejections] = useState<Record<string,
    "resolved" | "conflicted" | "refused">>({});
  const [recoveryUnavailable, setRecoveryUnavailable] = useState(false);
  const [recoveryChecked, setRecoveryChecked] = useState(false);
  const refreshedAcceptance = useRef(new Set<string>());
  const acceptingRef = useRef(false);
  const effectiveLocators = [...locators, ...recoveredProposalIds.filter((id) =>
    !locators.some((locator) => locator.proposalId === id)).map((proposalId) =>
    ({ proposalId, runId: "", decisionId: "" }))];
  const locatorKey = effectiveLocators.map((item) =>
    `${item.proposalId}:${item.runId}:${item.decisionId}`).join("|");

  useEffect(() => {
    let active = true;
    const workspace = editorProps.persistWorkspace;
    if (workspace === undefined) {
      setRecoveryChecked(true);
      return () => { active = false; };
    }
    setRecoveryChecked(false);
    void Promise.all([acceptanceJournalProposals(workspace), rejectionJournalState(workspace),
      readExpansionJournal(workspace), pendingReplanIds(workspace)])
      .then(async ([acceptance, rejection, expansions, replans]) => {
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
      setPendingReplans(replans);
      setSettledRejections(rejection.settledResults);
      setRecoveryUnavailable(false);
    }).catch(() => {
      if (active) setRecoveryUnavailable(true);
    }).finally(() => {
      if (active) setRecoveryChecked(true);
    });
    return () => { active = false; };
  }, [editorProps.persistWorkspace, settlementRefresh]);

  useEffect(() => {
    let active = true;
    setReads([]);
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
    if (!recoveryChecked || reads.length !== effectiveLocators.length) {
      return () => { active = false; };
    }
    const workspace = editorProps.persistWorkspace;
    if (workspace === undefined) {
      setPendingAcceptances([]);
      setSessionBlockedIds([]);
      setAcceptanceChecked(true);
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
      if (result === "applied" && !refreshedAcceptance.current.has(proposal.proposal_id)) {
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
      if (active) setAcceptanceChecked(true);
    });
    return () => { active = false; };
  }, [reads, editorProps.persistWorkspace, recoveryChecked, journalPendingIds.join("|"),
    pendingRejections.join("|")]);

  const blockCounts = new Map<string, number>();
  for (const block of editorProps.blocks) {
    blockCounts.set(block.manuscript_block_id,
      (blockCounts.get(block.manuscript_block_id) ?? 0) + 1);
  }
  const projections: BlockProposalProjection[] = [];
  const unavailable: ProposalRead[] = [];
  const allHeadsKnown = recoveryChecked && reads.length === effectiveLocators.length
    && reads.every((item) => item.proposal !== undefined);
  const expectedHeads = reads.flatMap(({ proposal }) => proposal?.chapter_id === chapterId
    ? [proposal.revision_id] : []).sort();
  for (const { locator, proposal } of reads) {
    if (proposal !== undefined && proposal.chapter_id !== chapterId) continue;
    const operation = proposal?.operations.find((item) =>
      item.manuscript_block_id === proposal.manuscript_block_id);
    const condition = proposal === undefined ? "absent" : proposalConditionKind(proposal);
    const anchored = proposal !== undefined && proposal.kind === "block_edit"
      && operation !== undefined
      && blockCounts.get(proposal.manuscript_block_id) === 1 && safeToProject;
    const baseMatches = proposal?.base_authoritative_revision_id === authoritativeRevisionId;
    const conditionVisible = condition !== "absent" || proposal?.validation === "invalid"
      || proposal?.validation === "pending";
    if (!anchored || proposal === undefined || operation === undefined
      || (!baseMatches && !conditionVisible)) {
      unavailable.push({ locator, proposal });
      continue;
    }
    const writerOpen = editorProps.persistWorkspace?.partition.disposition === "current_writer_open"
      && editorProps.persistWorkspace.session.writer.kind === "current_writer";
    const sessionBlocked = sessionBlockedIds.includes(proposal.proposal_id);
    const pendingAcceptance = pendingAcceptances.includes(proposal.proposal_id);
    const localPending = candidateTexts[`${proposal.proposal_id}:${proposal.revision_id}`] !== undefined;
    const controlsReady = allHeadsKnown && acceptanceChecked && !recoveryUnavailable
      && journalPendingIds.length === 0 && writerOpen && !sessionBlocked
      && editorProps.editable && accepting !== proposal.proposal_id && !localPending;
    const problem = knownProblems[proposal.proposal_id];
    const eligible = controlsReady && baseMatches
      && proposal.generation === "ready" && proposal.validation === "valid"
      && condition === "absent"
      && proposal.closure === "open" && operation.resolution === "pending"
      && operation.reservation_state === "unresolved"
      && proposal.validation_receipt.kind === "present"
      && proposal.validation_receipt.result === "valid"
      && (problem === undefined || problem.code === "acceptance_session_ineligible")
      && !pendingAcceptance;
    const rejectEligible = controlsReady && condition !== "proposal_recovery_conflict"
      && proposal.closure === "open" && operation.resolution === "pending"
      && !pendingReplans.includes(proposal.proposal_id);
    const replanEligible = controlsReady
      && (condition === "proposal_conflict" || condition === "proposal_recovery_conflict")
      && proposal.closure === "open" && operation.resolution === "pending"
      && !pendingAcceptance && !pendingRejections.includes(proposal.proposal_id)
      && !pendingReplans.includes(proposal.proposal_id);
    projections.push({
      proposalId: proposal.proposal_id,
      operationId: operation.operation_id,
      revisionId: proposal.revision_id,
      blockId: proposal.manuscript_block_id,
      sourceRunId: proposal.source.kind === "agent_run_decision" ? proposal.source.run_id : "",
      sourceDecisionId: proposal.source.kind === "agent_run_decision" ? proposal.source.decision_id : "",
      text: candidateTexts[`${proposal.proposal_id}:${proposal.revision_id}`]
        ?? proposal.candidate_text,
      eligible,
      retryPending: pendingAcceptance && knownProblems[proposal.proposal_id] === undefined
        && accepting !== proposal.proposal_id,
      rejectEligible,
      retryRejection: pendingRejections.includes(proposal.proposal_id)
        && accepting !== proposal.proposal_id,
      replanEligible,
      copyEligible: proposal.candidate_text.length > 0,
      conditionKind: condition,
      validity: proposal.validation,
      sessionEligible: writerOpen && !sessionBlocked,
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
            : error instanceof Error && error.message === "Acceptance session is closed"
              ? "请先恢复写作会话。系统不会自动提交。"
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

  const rejectDisplayed = (target: {
    proposalId: string; operationId: string; revisionId: string; text: string;
  }) => {
    if (acceptingRef.current) return;
    if (pendingRejections.includes(target.proposalId)) {
      retryRejection(target.proposalId);
      return;
    }
    const displayed = reads.find(({ proposal }) => proposal?.proposal_id === target.proposalId)?.proposal;
    const projection = projections.find((item) => item.proposalId === target.proposalId);
    const workspace = editorProps.persistWorkspace;
    if (displayed === undefined || workspace === undefined
      || projection?.rejectEligible !== true
      || displayed.revision_id !== target.revisionId
      || displayed.candidate_text !== target.text
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
        if (editorProps.controllerRef.current?.hasIncompleteSemanticIntent()) {
          throw new Error("请先完成当前输入。");
        }
        await editorProps.controllerRef.current?.flush();
        if (workspace.partition.disposition !== "current_writer_open"
          || workspace.pending.save_state !== "saved"
          || workspace.pending.unsettled_intent_count !== 0) {
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
            workspace, proposalId: target.proposalId, operationId: target.operationId,
            proposalRevisionId: target.revisionId, targetRevisionId: authoritativeRevisionId });
        if (response.project_scope.owner_user_id !== scope.owner_user_id
          || response.project_scope.project_id !== scope.project_id
          || response.receipt.proposal_id !== target.proposalId
          || response.receipt.proposal_revision_id !== target.revisionId
          || JSON.stringify(response.receipt.selected_pending_operation_ids)
            !== JSON.stringify([target.operationId])) {
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
        acceptingRef.current = false;
        setAccepting(undefined);
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
        acceptingRef.current = false;
        setAccepting(undefined);
      }
    })();
  };

  const writerReady = () => editorProps.persistWorkspace?.partition.disposition === "current_writer_open"
    && editorProps.persistWorkspace.session.writer.kind === "current_writer";

  const replanDisplayed = (target: {
    proposalId: string; operationId: string; revisionId: string; text: string;
  }) => {
    if (acceptingRef.current) return;
    if (pendingReplans.includes(target.proposalId)) {
      retryReplan(target.proposalId);
      return;
    }
    const displayed = reads.find(({ proposal }) => proposal?.proposal_id === target.proposalId)?.proposal;
    const projection = projections.find((item) => item.proposalId === target.proposalId);
    const workspace = editorProps.persistWorkspace;
    const condition = displayed === undefined ? undefined : displayed.source_condition;
    if (displayed === undefined || workspace === undefined || projection?.replanEligible !== true
      || !writerReady() || displayed.revision_id !== target.revisionId
      || condition === undefined || condition.kind === "absent") {
      setDecisionMessages((current) => ({ ...current,
        [target.proposalId]: "候选文字已变化。请检查当前版本。" }));
      setSettlementRefresh((value) => value + 1);
      return;
    }
    acceptingRef.current = true;
    setAccepting(target.proposalId);
    void (async () => {
      try {
        const current = await getProposal({ baseUrl: editorProps.baseUrl,
          fetchImpl: editorProps.fetchImpl, projectId: scope.project_id,
          proposalId: target.proposalId });
        if (current.project_scope.owner_user_id !== scope.owner_user_id
          || current.proposal.revision_id !== displayed.revision_id
          || current.proposal.source_condition.kind !== condition.kind) {
          throw new Error("候选文字的身份已变化。");
        }
        const response = await replanDisplayedBlockProposal({
          baseUrl: editorProps.baseUrl, fetchImpl: editorProps.fetchImpl,
          cryptoImpl: editorProps.cryptoImpl, workspace, proposalId: target.proposalId,
          operationId: target.operationId, proposalRevisionId: target.revisionId,
          targetRevisionId: authoritativeRevisionId, sourceCondition: condition,
        });
        if (response.receipt.proposal_id !== target.proposalId) {
          throw new Error("重新规划结果的身份不匹配。");
        }
        setDecisionMessages((current) => ({ ...current, [target.proposalId]:
          response.effect.kind === "resolved"
            ? "已按当前正文重新规划。接受需要当前版本通过验证。"
            : response.effect.kind === "conflicted"
              ? "正文已变化，重新规划未完成。"
              : "重新规划未生效，请检查当前候选文字。" }));
        setSettlementRefresh((value) => value + 1);
        await onAccepted();
      } catch (error) {
        setDecisionMessages((current) => ({ ...current, [target.proposalId]:
          error instanceof Error && error.message.startsWith("请先")
            ? error.message : "重新规划结果尚未确认。请重试同一操作。" }));
        setSettlementRefresh((value) => value + 1);
      } finally {
        acceptingRef.current = false;
        setAccepting(undefined);
      }
    })();
  };

  const retryReplan = (proposalId: string) => {
    const workspace = editorProps.persistWorkspace;
    if (acceptingRef.current || workspace === undefined) return;
    acceptingRef.current = true;
    setAccepting(proposalId);
    void (async () => {
      try {
        const response = await retryPendingDisplayedReplan({
          baseUrl: editorProps.baseUrl, fetchImpl: editorProps.fetchImpl,
          cryptoImpl: editorProps.cryptoImpl, workspace, proposalId,
        });
        setDecisionMessages((current) => ({ ...current, [proposalId]:
          response.effect.kind === "resolved"
            ? "已按当前正文重新规划。接受需要当前版本通过验证。"
            : "重新规划结果尚未确认。请重试同一操作。" }));
        setSettlementRefresh((value) => value + 1);
        await onAccepted();
      } catch {
        setDecisionMessages((current) => ({ ...current,
          [proposalId]: "重新规划结果尚未确认。请重试同一操作。" }));
        setSettlementRefresh((value) => value + 1);
      } finally {
        acceptingRef.current = false;
        setAccepting(undefined);
      }
    })();
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
          && (acceptanceChecked || (effectiveLocators.length === 0
            && editorProps.persistWorkspace?.pending.unsettled_intent_count === 0))
          && accepting === undefined
          && pendingAcceptances.length === 0}
        proposals={projections}
        onCandidateSettled={() => setSettlementRefresh((value) => value + 1)}
        onAcceptProposal={acceptDisplayed} onRejectProposal={rejectDisplayed}
        onReplanProposal={replanDisplayed} onCopyProposal={copyDisplayed} />
      <RefusedEditDraftDisplay workspace={editorProps.persistWorkspace} scope={scope}
        baseUrl={editorProps.baseUrl} fetchImpl={editorProps.fetchImpl}
        refreshKey={`${refreshKey}:${settlementRefresh}`} onHoldChange={setDiscardHold} onProjection={editorProps.onProjection}
        onResult={() => setSettlementRefresh((value) => value + 1)} />
      {reads.flatMap(({ proposal }) => proposal?.source.kind === "refused_edit_draft" ? [
        <section key={proposal.proposal_id} data-proposal-id={proposal.proposal_id} aria-label="Draft Proposal">
          <p>Source Draft: {proposal.source.draft_id}. Revision: {proposal.source.draft_revision_id}. Status: {proposal.validation}, {proposal.closure}.</p>
          {proposal.candidate_blocks?.map((block, index) => block.block_kind === "heading"
            ? <h3 key={index} data-proposal-structured-block style={{ whiteSpace: "pre-wrap" }}>{block.text}</h3>
            : <p key={index} data-proposal-structured-block style={{ whiteSpace: "pre-wrap" }}>{block.text}</p>)}
        </section>] : [])}
      {recoveryUnavailable ? <p role="alert">接受记录暂不可读取，请检查本地数据。</p> : null}
      {reads.map(({ locator, proposal }) => {
        const problem = knownProblems[locator.proposalId];
        const rejectionResult = settledRejections[locator.proposalId];
        const writerOpen = editorProps.persistWorkspace?.partition.disposition === "current_writer_open"
          && editorProps.persistWorkspace.session.writer.kind === "current_writer";
        const posture: SessionPosture = !writerOpen ? "closed"
          : sessionBlockedIds.includes(locator.proposalId) ? "blocked" : "current";
        const message = pendingRejections.includes(locator.proposalId)
            ? "拒绝结果尚未确认。请重试同一操作。"
          : proposal?.operation_resolution === "rejected"
            ? "已拒绝，正文保持不变。"
          : rejectionResult !== undefined
            ? {
              resolved: "已拒绝，正文保持不变。",
              conflicted: "正文已变化，拒绝结果请检查。",
              refused: "此次拒绝未生效，请检查当前候选文字。",
            }[rejectionResult]
          : pendingReplans.includes(locator.proposalId)
            ? "重新规划结果尚未确认。请重试同一操作。"
          : problem !== undefined && problem.code !== "acceptance_session_ineligible"
            ? `Acceptance ${problem.code} (HTTP ${problem.status}): ${problem.message}`
          : pendingAcceptances.includes(locator.proposalId)
            ? proposal?.operation_resolution === "applied"
              ? "正文已变化；此次接受结果尚未确认。请重试同一操作。"
              : decisionMessages[locator.proposalId]
                ?? "接受结果尚未确认。请重试同一操作。"
          : proposal?.operation_resolution === "applied"
          ? !acceptanceChecked ? "正在同步正文。"
            : decisionMessages[locator.proposalId]?.includes("请刷新")
            ? decisionMessages[locator.proposalId] : "已接受，正文已更新。"
          : proposal === undefined ? decisionMessages[locator.proposalId]
          : (recoveryStatus(proposal, posture) ?? decisionMessages[locator.proposalId]);
        return message === undefined ? null : (
          <p data-proposal-decision={locator.proposalId} role="status" key={locator.proposalId}>
            {message}
            {pendingAcceptances.includes(locator.proposalId)
              && problem === undefined ? (
                <button type="button" disabled={accepting !== undefined}
                  onClick={() => retryPending(locator.proposalId)}>重试接受</button>
              ) : null}
            {pendingRejections.includes(locator.proposalId) ? (
              <button type="button" disabled={accepting !== undefined}
                onClick={() => retryRejection(locator.proposalId)}>重试拒绝</button>
            ) : null}
            {pendingReplans.includes(locator.proposalId) ? (
              <button type="button" data-proposal-replan={locator.proposalId}
                disabled={accepting !== undefined}
                onClick={() => retryReplan(locator.proposalId)}>重试重新规划</button>
            ) : null}
            {decisionMessages[locator.proposalId] === "已复制候选文字。"
              && message !== "已复制候选文字。"
              ? <span>已复制候选文字。</span> : null}
          </p>
        );
      })}
      {unavailable.map(({ locator, proposal }) => (
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
