SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.conversation_memory_settings
  DROP CONSTRAINT conversation_memory_settings_owner_user_id_project_id_conve_key,
  ADD COLUMN is_current boolean DEFAULT TRUE;

-- A retained single-revision row without the marker is current.
CREATE UNIQUE INDEX conversation_memory_settings_one_current
  ON storyos.conversation_memory_settings (owner_user_id, project_id, conversation_id)
  WHERE COALESCE(is_current, TRUE);
