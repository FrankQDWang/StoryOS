SET LOCAL ROLE storyos_owner;

-- The owner reads and binds the rows of all Projects, so RLS stops for this transaction only.
ALTER TABLE storyos.proposal_anchors NO FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_revisions NO FORCE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_anchors ADD COLUMN proposal_revision_id uuid;
UPDATE storyos.proposal_anchors AS anchor
   SET proposal_revision_id = opening.revision_id
  FROM storyos.proposal_revisions AS opening
 WHERE (opening.owner_user_id, opening.project_id, opening.proposal_id) =
       (anchor.owner_user_id, anchor.project_id, anchor.proposal_id)
   AND opening.parent_revision_id IS NULL;
ALTER TABLE storyos.proposal_anchors ALTER COLUMN proposal_revision_id SET NOT NULL;
ALTER TABLE storyos.proposal_anchors DROP CONSTRAINT proposal_anchors_pkey;
ALTER TABLE storyos.proposal_anchors
  ADD PRIMARY KEY
    (owner_user_id, project_id, proposal_id, proposal_revision_id, operation_id, anchor_order);
ALTER TABLE storyos.proposal_anchors
  ADD FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id)
    REFERENCES storyos.proposal_revisions
      (owner_user_id, project_id, proposal_id, revision_id) MATCH FULL;

ALTER TABLE storyos.proposal_revisions FORCE ROW LEVEL SECURITY;
ALTER TABLE storyos.proposal_anchors FORCE ROW LEVEL SECURITY;

-- The Anchor set of a Proposal Revision is the set of the nearest Revision in its parent chain
-- that wrote one. A set on a different base Authoritative Revision fails closed.
CREATE FUNCTION storyos.proposal_anchor_set_revision(
  scope_owner_user_id uuid,
  scope_project_id uuid,
  read_proposal_id uuid,
  read_revision_id uuid
)
RETURNS uuid
LANGUAGE plpgsql
STABLE
SET search_path = pg_catalog
AS $function$
DECLARE
  selected_revision_id uuid;
BEGIN
  WITH RECURSIVE lineage AS (
    SELECT revision.revision_id, revision.parent_revision_id, 0 AS depth
      FROM storyos.proposal_revisions AS revision
     WHERE (revision.owner_user_id, revision.project_id, revision.proposal_id,
            revision.revision_id) =
           (scope_owner_user_id, scope_project_id, read_proposal_id, read_revision_id)
    UNION ALL
    SELECT parent.revision_id, parent.parent_revision_id, lineage.depth + 1
      FROM lineage
      JOIN storyos.proposal_revisions AS parent
        ON (parent.owner_user_id, parent.project_id, parent.proposal_id, parent.revision_id) =
           (scope_owner_user_id, scope_project_id, read_proposal_id, lineage.parent_revision_id)
  )
  SELECT lineage.revision_id INTO selected_revision_id
    FROM lineage
   WHERE EXISTS (
     SELECT 1 FROM storyos.proposal_anchors AS anchor
      WHERE (anchor.owner_user_id, anchor.project_id, anchor.proposal_id,
             anchor.proposal_revision_id) =
            (scope_owner_user_id, scope_project_id, read_proposal_id, lineage.revision_id)
   )
   ORDER BY lineage.depth
   LIMIT 1;
  IF EXISTS (
    SELECT 1
      FROM storyos.proposal_anchors AS anchor
      JOIN storyos.proposal_revisions AS revision
        ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
            revision.revision_id) =
           (scope_owner_user_id, scope_project_id, read_proposal_id, read_revision_id)
     WHERE (anchor.owner_user_id, anchor.project_id, anchor.proposal_id,
            anchor.proposal_revision_id) =
           (scope_owner_user_id, scope_project_id, read_proposal_id, selected_revision_id)
       AND anchor.base_authoritative_revision_id <> revision.base_authoritative_revision_id
  ) THEN
    RAISE EXCEPTION 'the Anchor set base is not the Proposal Revision base'
      USING ERRCODE = 'data_corrupted';
  END IF;
  RETURN selected_revision_id;
END
$function$;

GRANT EXECUTE ON FUNCTION storyos.proposal_anchor_set_revision(uuid, uuid, uuid, uuid)
  TO storyos_runtime;
