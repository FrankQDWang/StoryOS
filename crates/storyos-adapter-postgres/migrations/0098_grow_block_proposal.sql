SET LOCAL ROLE storyos_owner;

-- A Proposal Growth makes the bundle of the grown Proposal atomic (ADR 0071).
GRANT UPDATE (bundle_policy) ON storyos.proposals TO storyos_runtime;
