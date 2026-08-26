SET LOCAL ROLE storyos_owner;

ALTER TABLE storyos.projects
  ADD COLUMN tree_revision bigint NOT NULL DEFAULT 1
    CHECK (tree_revision >= 1);
