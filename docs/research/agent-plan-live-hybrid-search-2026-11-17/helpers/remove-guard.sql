DROP TRIGGER live_guard_model ON storyos.model_attempts;
DROP TRIGGER live_guard_exa_insert ON storyos.tool_calls;
DROP TRIGGER live_guard_exa_update ON storyos.tool_calls;
DROP TRIGGER live_guard_embedding ON storyos.embedding_attempts;
DROP SCHEMA live_guard CASCADE;
