SET LOCAL ROLE storyos_owner;

-- ADR 0052: the request bounds for a long Chapter. A binding with the bounds of ADR 0049 keeps
-- them.
ALTER TABLE storyos.project_external_use_binding_revisions
  DROP CONSTRAINT project_external_use_binding_revisions_budget_bounds_check,
  ADD CONSTRAINT project_external_use_binding_revisions_budget_bounds_check
    CHECK (budget_bounds IN (
      'not_required', 'unqualified', 'agent_plan_request_v1', 'agent_plan_request_v2'
    ));
