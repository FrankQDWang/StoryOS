SET LOCAL ROLE storyos_owner;

-- A qualified Model Registration revision becomes the head of its adapter (S4-02c). An AgentRun
-- that pinned an earlier head then refuses with Registration drift.
GRANT UPDATE (model_registration_revision) ON storyos.model_registration_heads TO storyos_runtime;
