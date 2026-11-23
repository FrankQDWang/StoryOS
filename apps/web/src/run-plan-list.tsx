import type {
  GetAgentRunResponse, PlanStepStatusInspect,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

/** The accessible name of the mark of each state of a step. */
const MARK_NAMES: Record<PlanStepStatusInspect, string> = {
  completed: "已完成",
  in_progress: "进行中",
  pending: "未开始",
};

/**
 * The plan of a Run as the newest Run read gave it (ADR 0103 section 2): the complete list in the order of the steps,
 * with one mark for the state of each step. The shape of each mark comes from the style sheet, so the states differ
 * without colour alone, and the panel shows no other new text. The list has no control. A Run with no plan shows no
 * list.
 */
export function RunPlanList({ run }: { run: GetAgentRunResponse }) {
  const steps = run.plan ?? [];
  // The style sheet removes the list markers, so Safari needs the list role to keep the list for VoiceOver.
  return steps.length === 0 ? null
    : <ol className="assistant-plan" role="list" data-assistant-plan="">
      {steps.map((step, index) => <li key={index} data-plan-step-status={step.status}>
        <span className="assistant-plan-mark" role="img" aria-label={MARK_NAMES[step.status]} />
        <span>{step.text}</span>
      </li>)}
    </ol>;
}
