import type { GetProposalResponse, ProjectScope } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { getProposal } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { openControlledProject } from "./boot.ts";
import { completeJournalOrRefuse, openSelectedChapter } from "./chapter-navigation.ts";
import { rebuildPendingProjection } from "./local-edit-journal.ts";
import { setOwnedCurrentChapter } from "./set-current-chapter.ts";
import type { ProjectReadyState, ControlledProjectState } from "./editor-types.ts";
import type { ManualInputController } from "./manual-input.ts";
import type { ProposalLocator } from "./block-proposal-display.tsx";

export type ProposalFocus = { proposalId: string; operationId: string; revisionId: string; blockId: string };
export type ProposalDestination = { chapterId: string; focus?: ProposalFocus };
export type ProposalNavigation = { sequence: number; queue: Promise<void> };

function matchesDestination(response: GetProposalResponse, scope: ProjectScope,
  destination: ProposalDestination): boolean {
  const proposal = response.proposal;
  const focus = destination.focus!;
  return response.project_scope.owner_user_id === scope.owner_user_id
    && response.project_scope.project_id === scope.project_id
    && proposal.proposal_id === focus.proposalId && proposal.chapter_id === destination.chapterId
    && proposal.revision_id === focus.revisionId && proposal.closure === "open"
    && proposal.operations.some((operation) => operation.operation_id === focus.operationId
      && operation.manuscript_block_id === focus.blockId && operation.resolution === "pending"
      && operation.reservation_state === "unresolved");
}

export function navigateProposal(options: {
  state: ProjectReadyState;
  destination: ProposalDestination;
  navigation: ProposalNavigation;
  controller: { current: ManualInputController | null };
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onOpened: (state: ControlledProjectState, focus?: ProposalFocus) => void;
  onLocator: (locator: ProposalLocator) => void;
  onFailure: (message: string) => void;
}): void {
  const { navigation, destination, state, baseUrl, fetchImpl, cryptoImpl } = options;
  const sequence = ++navigation.sequence;
  const latest = () => navigation.sequence === sequence;
  navigation.queue = navigation.queue.then(async () => {
    if (!latest()) return;
    try {
      await options.controller.current?.flush();
      const gate = await completeJournalOrRefuse({
        incompleteSemanticIntent: options.controller.current?.hasIncompleteSemanticIntent() ?? false,
        whenIdle: () => options.controller.current?.whenIdle() ?? Promise.resolve(),
      });
      if (!latest()) return;
      if (gate.kind === "refused" || state.editor.kind !== "editor-ready"
        || (await rebuildPendingProjection(state.editor)).unsettled_intent_count !== 0) {
        throw new Error("请先完成当前输入。");
      }
      const scope = state.project.project_scope;
      const focus = destination.focus;
      if (focus !== undefined) {
        const response = await getProposal({ baseUrl, fetchImpl, projectId: scope.project_id,
          proposalId: focus.proposalId });
        if (!latest()) return;
        const proposal = response.proposal;
        if (!matchesDestination(response, scope, destination)) throw new Error("候选位置已变化，请检查本次结果。");
        if (proposal.source.kind === "agent_run_decision") options.onLocator({
          proposalId: proposal.proposal_id, runId: proposal.source.run_id, decisionId: proposal.source.decision_id,
        });
      }
      const opened = await openSelectedChapter({ baseUrl, fetchImpl, projectId: scope.project_id,
        chapterId: destination.chapterId, expectedScope: scope });
      if (!latest()) return;
      if (opened.kind !== "opened" || focus !== undefined
        && !opened.chapter.chapter.current_revision.blocks.some((block) => block.manuscript_block_id === focus.blockId)) {
        throw new Error("无法打开候选位置。");
      }
      const current = await openControlledProject({ baseUrl, fetchImpl, cryptoImpl, projectId: scope.project_id });
      if (!latest()) return;
      if (current.kind !== "project-ready" || current.editor.kind !== "editor-ready"
        || current.project.project.open.kind !== "current_chapter") throw new Error("无法打开写作会话。");
      const switched = await setOwnedCurrentChapter({ baseUrl, fetchImpl, cryptoImpl, projectId: scope.project_id,
        chapterId: destination.chapterId, expectedCurrentChapterId: current.project.project.open.current_chapter_id,
        expectedTargetRevisionId: opened.chapter.chapter.current_revision.revision_id,
        editorSessionId: current.editor.session.editor_session.editor_session_id });
      if (!latest()) return;
      if (switched.effect.kind !== "authoritative_applied" && switched.effect.kind !== "no_effect") {
        throw new Error("无法打开写作会话。");
      }
      const next = await openControlledProject({ baseUrl, fetchImpl, cryptoImpl, projectId: scope.project_id });
      if (!latest()) return;
      if (next.kind !== "project-ready" || next.editor.kind !== "editor-ready"
        || next.editor.session.writer.kind !== "current_writer"
        || next.editor.partition.disposition !== "current_writer_open"
        || next.chapter.chapter.chapter_id !== destination.chapterId
        || next.project.project_scope.owner_user_id !== scope.owner_user_id
        || next.project.project_scope.project_id !== scope.project_id
        || focus !== undefined && !next.chapter.chapter.current_revision.blocks.some((block) =>
          block.manuscript_block_id === focus.blockId)) throw new Error("无法打开写作会话。");
      if (focus !== undefined) {
        const response = await getProposal({ baseUrl, fetchImpl, projectId: scope.project_id,
          proposalId: focus.proposalId });
        if (!latest()) return;
        if (!matchesDestination(response, scope, destination)) throw new Error("候选位置已变化，请检查本次结果。");
      }
      options.onOpened(next, focus);
    } catch (error) {
      if (latest()) options.onFailure(error instanceof Error ? error.message : "无法打开候选位置。");
    }
  });
}
