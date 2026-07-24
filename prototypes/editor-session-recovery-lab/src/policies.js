import {
  HARD_BOUNDARY_KINDS,
  bindingFingerprint,
} from "../shared/contract.mjs";

const COALESCIBLE = new Set([
  "typing",
  "delete_backward",
  "delete_forward",
  "selection_replace",
]);

function groupFrom(intent) {
  return {
    intent_ids: [intent.intent_id],
    intents: [intent],
    before_text: intent.before_text,
    result_text: intent.after_text,
    started_at_ms: intent.at_ms,
    ended_at_ms: intent.at_ms,
    binding_fingerprint: bindingFingerprint(intent.binding),
  };
}

function append(group, intent) {
  group.intent_ids.push(intent.intent_id);
  group.intents.push(intent);
  group.result_text = intent.after_text;
  group.ended_at_ms = intent.at_ms;
}

function crossesHardBoundary(left, right) {
  return (
    HARD_BOUNDARY_KINDS.has(left.kind) ||
    HARD_BOUNDARY_KINDS.has(right.kind) ||
    bindingFingerprint(left.binding) !== bindingFingerprint(right.binding)
  );
}

export function partitionIntents(
  intents,
  strategy,
  { idle_ms = 250, fixed_window_ms = 500 } = {},
) {
  const groups = [];
  const violations = [];

  if (strategy === "transaction") {
    for (const intent of intents) {
      const transactionCount = Math.max(1, intent.transaction_count ?? 1);
      for (let index = 0; index < transactionCount; index += 1) {
        groups.push({
          ...groupFrom(intent),
          transaction_index: index,
          transaction_count: transactionCount,
        });
      }
      if (intent.kind === "composition" && transactionCount > 1) {
        violations.push({
          code: "composition_fragmented",
          intent_id: intent.intent_id,
          groups: transactionCount,
        });
      }
      if (intent.history_only_transactions > 0) {
        violations.push({
          code: "non_semantic_transaction_committed",
          intent_id: intent.intent_id,
          count: intent.history_only_transactions,
        });
      }
    }
    return { strategy, groups, violations };
  }

  if (strategy === "semantic-intent") {
    return {
      strategy,
      groups: intents.map(groupFrom),
      violations,
    };
  }

  if (strategy === "bounded-idle") {
    for (const intent of intents) {
      const current = groups.at(-1);
      const previous = current?.intents.at(-1);
      const canMerge =
        current &&
        previous &&
        COALESCIBLE.has(previous.kind) &&
        COALESCIBLE.has(intent.kind) &&
        intent.at_ms - current.ended_at_ms <= idle_ms &&
        !crossesHardBoundary(previous, intent);
      if (canMerge) append(current, intent);
      else groups.push(groupFrom(intent));
    }
    return { strategy, groups, violations };
  }

  if (strategy === "fixed-window") {
    let windowStart = null;
    for (const intent of intents) {
      if (
        windowStart === null ||
        intent.at_ms - windowStart >= fixed_window_ms
      ) {
        groups.push(groupFrom(intent));
        windowStart = intent.at_ms;
        continue;
      }
      const current = groups.at(-1);
      const previous = current.intents.at(-1);
      if (crossesHardBoundary(previous, intent)) {
        violations.push({
          code: "fixed_window_crossed_contract_boundary",
          left_intent_id: previous.intent_id,
          right_intent_id: intent.intent_id,
          left_kind: previous.kind,
          right_kind: intent.kind,
        });
      }
      append(current, intent);
    }
    return { strategy, groups, violations };
  }

  throw new Error(`unknown_strategy:${strategy}`);
}
