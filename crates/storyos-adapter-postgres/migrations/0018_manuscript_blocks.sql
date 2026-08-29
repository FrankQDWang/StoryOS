SET LOCAL ROLE storyos_owner;

CREATE TABLE storyos.manuscript_blocks (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  manuscript_block_id uuid NOT NULL,
  manuscript_object_id uuid NOT NULL,
  block_kind text NOT NULL CHECK (block_kind = 'paragraph'),
  PRIMARY KEY (owner_user_id, project_id, manuscript_block_id),
  FOREIGN KEY (owner_user_id, project_id, manuscript_object_id)
    REFERENCES storyos.manuscript_objects
      (owner_user_id, project_id, manuscript_object_id) MATCH FULL
);
CREATE INDEX manuscript_blocks_chapter_fk_idx
  ON storyos.manuscript_blocks (owner_user_id, project_id, manuscript_object_id);

CREATE TABLE storyos.manuscript_revision_members (
  owner_user_id uuid NOT NULL,
  project_id uuid NOT NULL,
  manuscript_object_id uuid NOT NULL,
  revision_id uuid NOT NULL,
  manuscript_block_id uuid NOT NULL,
  block_order numeric(20, 0) NOT NULL
    CHECK (block_order BETWEEN 1 AND 18446744073709551615),
  PRIMARY KEY (
    owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id
  ),
  UNIQUE (
    owner_user_id, project_id, manuscript_object_id, revision_id, block_order
  ),
  FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, revision_id)
    REFERENCES storyos.authoritative_revisions
      (owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL,
  FOREIGN KEY (owner_user_id, project_id, manuscript_block_id)
    REFERENCES storyos.manuscript_blocks
      (owner_user_id, project_id, manuscript_block_id) MATCH FULL
);
CREATE INDEX manuscript_revision_members_block_fk_idx
  ON storyos.manuscript_revision_members
    (owner_user_id, project_id, manuscript_block_id);

ALTER TABLE storyos.manuscript_blocks ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.manuscript_blocks FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.manuscript_revision_members ENABLE ROW LEVEL SECURITY;
ALTER TABLE storyos.manuscript_revision_members FORCE ROW LEVEL SECURITY;

CREATE POLICY manuscript_blocks_exact_scope ON storyos.manuscript_blocks USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);

CREATE POLICY manuscript_revision_members_exact_scope ON storyos.manuscript_revision_members USING (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
) WITH CHECK (
  owner_user_id = NULLIF(current_setting('storyos.owner_user_id', true), '')::uuid
  AND project_id = NULLIF(current_setting('storyos.project_id', true), '')::uuid
);

GRANT SELECT, INSERT ON storyos.manuscript_blocks, storyos.manuscript_revision_members
  TO storyos_runtime;
