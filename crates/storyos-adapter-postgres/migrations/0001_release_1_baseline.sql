-- The Release 1 baseline (ADR 0079): one definition of each object of the squashed migration chain.
-- Until the first database that holds data exists, a change can edit this file. The next migration is 0002.

CREATE SCHEMA storyos AUTHORIZATION storyos_owner;
SET LOCAL ROLE storyos_owner;

GRANT USAGE ON SCHEMA storyos TO storyos_runtime;
GRANT USAGE ON SCHEMA storyos TO storyos_restore;
GRANT USAGE ON SCHEMA storyos TO storyos_backup;

-- Closed lists (ADR 0079). The command kinds and the event kinds are those of the generated Route Catalog.

CREATE DOMAIN storyos.command_kind AS text CHECK (VALUE IN (
    'acceptProposal',
    'applyAuthorEdit',
    'cancelAgentRun',
    'completeReadyPartialProposal',
    'continueProposalGeneration',
    'createAgentRun',
    'createChapter',
    'createEditorSession',
    'createProject',
    'createVolume',
    'deleteChapter',
    'deleteVolume',
    'exportHumanReadableManuscript',
    'exportProjectArchive',
    'pauseAgentRun',
    'rejectProposalOperations',
    'reopenRejectedOperations',
    'reopenWithdrawnProposal',
    'replanProposal',
    'setCurrentChapter',
    'setProjectToolEnablement',
    'steerAgentRun',
    'takeOverProjectWriter',
    'undoLatestAuthorAction',
    'updateChapter',
    'updateProject',
    'updateProjectAssistance',
    'updateVolume',
    'withdrawProposal'
));

CREATE DOMAIN storyos.event_kind AS text CHECK (VALUE IN (
    'agent_run_cancelled',
    'agent_run_created',
    'agent_run_paused',
    'agent_run_steering_retained',
    'authoritative_author_edit_applied',
    'chapter_created',
    'chapter_deleted',
    'chapter_updated',
    'current_chapter_set',
    'human_readable_manuscript_export_settled',
    'project_assistance_updated',
    'project_created',
    'project_export_settled',
    'project_tool_enablement_set',
    'project_updated',
    'volume_created',
    'volume_deleted',
    'volume_updated',
    'writer_takeover_applied',
    'writer_takeover_compare_failed'
));

CREATE DOMAIN storyos.result_kind AS text CHECK (VALUE IN (
    'authoritative_applied',
    'conflicted',
    'invalid',
    'no_effect',
    'proposal_closure_changed',
    'proposal_generation_completed',
    'proposal_generation_started',
    'proposal_operations_resolved',
    'proposal_revised',
    'refused'
));

CREATE DOMAIN storyos.receipt_result_kind AS text CHECK (VALUE IN (
    'authoritative_applied',
    'proposal_closure_changed',
    'proposal_generation_completed',
    'proposal_generation_started',
    'proposal_operations_resolved',
    'proposal_revised'
));

CREATE DOMAIN storyos.agent_run_status AS text CHECK (VALUE IN (
    'cancelled',
    'claimed',
    'completed',
    'paused',
    'queued',
    'refused',
    'waiting'
));

CREATE DOMAIN storyos.attempt_role AS text CHECK (VALUE IN (
    'abort',
    'compaction',
    'decision',
    'late_retrieval',
    'later_request',
    'rejected_create',
    'retrieval',
    'successor',
    'successor_abort'
));

CREATE DOMAIN storyos.action_class AS text CHECK (VALUE IN (
    'agent_run_control',
    'agent_run_start',
    'direct_editor_action',
    'explicit_editor_command',
    'explicit_project_command'
));

-- Users, Projects, manuscript objects, and Authoritative Revisions

CREATE TABLE storyos.users (
    user_id uuid NOT NULL,
    CONSTRAINT users_pkey PRIMARY KEY (user_id)
);

ALTER TABLE storyos.users ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.users FORCE ROW LEVEL SECURITY;

CREATE POLICY users_exact_identity ON storyos.users USING ((user_id = (NULLIF(current_setting('storyos.user_id'::text, true), ''::text))::uuid)) WITH CHECK ((user_id = (NULLIF(current_setting('storyos.user_id'::text, true), ''::text))::uuid));

GRANT SELECT ON TABLE storyos.users TO storyos_runtime;

CREATE FUNCTION storyos.uuid_setting(setting_name text) RETURNS uuid
    LANGUAGE sql STABLE PARALLEL SAFE
    SET search_path TO 'pg_catalog'
    AS $_$
  SELECT CASE
    WHEN current_setting(setting_name, true)
      ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
    THEN current_setting(setting_name, true)::uuid
  END
$_$;

GRANT ALL ON FUNCTION storyos.uuid_setting(setting_name text) TO storyos_runtime;

CREATE TABLE storyos.projects (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    title text NOT NULL,
    current_chapter_id uuid,
    lifecycle_state text DEFAULT 'active'::text NOT NULL,
    revision bigint DEFAULT 1 NOT NULL,
    tree_revision bigint DEFAULT 1 NOT NULL,
    CONSTRAINT projects_lifecycle_state_check CHECK ((lifecycle_state = 'active'::text)),
    CONSTRAINT projects_revision_check CHECK ((revision >= 1)),
    CONSTRAINT projects_title_check CHECK (((octet_length(title) >= 1) AND (octet_length(title) <= 1024))),
    CONSTRAINT projects_tree_revision_check CHECK ((tree_revision >= 1)),
    CONSTRAINT projects_pkey PRIMARY KEY (owner_user_id, project_id),
    CONSTRAINT projects_owner_user_id_fkey FOREIGN KEY (owner_user_id) REFERENCES storyos.users(user_id)
);

CREATE TABLE storyos.manuscript_objects (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    manuscript_object_id uuid NOT NULL,
    object_kind text NOT NULL,
    title text NOT NULL,
    tree_order bigint DEFAULT 1 NOT NULL,
    parent_volume_id uuid,
    CONSTRAINT manuscript_objects_object_kind_check CHECK ((object_kind = ANY (ARRAY['chapter'::text, 'volume'::text]))),
    CONSTRAINT manuscript_objects_parent_volume_shape CHECK ((((object_kind = 'volume'::text) AND (parent_volume_id IS NULL)) OR (object_kind = 'chapter'::text))),
    CONSTRAINT manuscript_objects_title_check CHECK (((octet_length(title) >= 1) AND (octet_length(title) <= 1024))),
    CONSTRAINT manuscript_objects_tree_order_check CHECK ((tree_order >= 1)),
    CONSTRAINT manuscript_objects_pkey PRIMARY KEY (owner_user_id, project_id, manuscript_object_id),
    CONSTRAINT manuscript_objects_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT manuscript_objects_parent_volume_fk FOREIGN KEY (owner_user_id, project_id, parent_volume_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id)
);

ALTER TABLE storyos.projects
    ADD CONSTRAINT projects_current_chapter_fk FOREIGN KEY (owner_user_id, project_id, current_chapter_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id) DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE storyos.projects ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.projects FORCE ROW LEVEL SECURITY;

CREATE POLICY projects_exact_scope ON storyos.projects USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

CREATE POLICY projects_user_owned_list ON storyos.projects FOR SELECT USING (((current_setting('storyos.scope_mode'::text, true) = 'user'::text) AND (owner_user_id = storyos.uuid_setting('storyos.user_id'::text)) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.projects TO storyos_runtime;

CREATE UNIQUE INDEX manuscript_objects_chapter_volume_tree_order ON storyos.manuscript_objects USING btree (owner_user_id, project_id, parent_volume_id, tree_order) WHERE ((object_kind = 'chapter'::text) AND (parent_volume_id IS NOT NULL));

CREATE UNIQUE INDEX manuscript_objects_volume_tree_order ON storyos.manuscript_objects USING btree (owner_user_id, project_id, tree_order) WHERE (object_kind = 'volume'::text);

ALTER TABLE storyos.manuscript_objects ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.manuscript_objects FORCE ROW LEVEL SECURITY;

CREATE POLICY manuscript_objects_exact_scope ON storyos.manuscript_objects USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.manuscript_objects TO storyos_runtime;

CREATE FUNCTION storyos.require_chapter_parent_volume() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
DECLARE
  parent_kind text;
BEGIN
  IF NEW.object_kind = 'volume' THEN
    IF NEW.parent_volume_id IS NOT NULL THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'A Volume cannot join another Volume';
    END IF;
    RETURN NEW;
  END IF;
  IF NEW.parent_volume_id IS NULL THEN
    RETURN NEW;
  END IF;
  SELECT object.object_kind
    INTO parent_kind
    FROM storyos.manuscript_objects AS object
   WHERE object.owner_user_id = NEW.owner_user_id
     AND object.project_id = NEW.project_id
     AND object.manuscript_object_id = NEW.parent_volume_id;
  IF parent_kind IS DISTINCT FROM 'volume' THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'A Chapter parent must be a Volume in exact Project Scope';
  END IF;
  RETURN NEW;
END
$$;

CREATE TRIGGER manuscript_objects_parent_volume BEFORE INSERT OR UPDATE ON storyos.manuscript_objects FOR EACH ROW EXECUTE FUNCTION storyos.require_chapter_parent_volume();

CREATE TABLE storyos.authoritative_payloads (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    payload_id uuid NOT NULL,
    canonical_bytes bytea NOT NULL,
    CONSTRAINT authoritative_payloads_canonical_bytes_check CHECK ((octet_length(canonical_bytes) <= 1048576)),
    CONSTRAINT authoritative_payloads_pkey PRIMARY KEY (owner_user_id, project_id, payload_id),
    CONSTRAINT authoritative_payloads_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

ALTER TABLE storyos.authoritative_payloads ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.authoritative_payloads FORCE ROW LEVEL SECURITY;

CREATE POLICY authoritative_payloads_exact_scope ON storyos.authoritative_payloads USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.authoritative_payloads TO storyos_runtime;

CREATE TABLE storyos.authoritative_revisions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    manuscript_object_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    payload_id uuid NOT NULL,
    CONSTRAINT authoritative_revisions_owner_user_id_project_id_manuscript_key UNIQUE (owner_user_id, project_id, manuscript_object_id, revision_id),
    CONSTRAINT authoritative_revisions_pkey PRIMARY KEY (owner_user_id, project_id, revision_id),
    CONSTRAINT authoritative_revisions_owner_user_id_project_id_manuscrip_fkey FOREIGN KEY (owner_user_id, project_id, manuscript_object_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id) MATCH FULL,
    CONSTRAINT authoritative_revisions_owner_user_id_project_id_payload_i_fkey FOREIGN KEY (owner_user_id, project_id, payload_id) REFERENCES storyos.authoritative_payloads(owner_user_id, project_id, payload_id) MATCH FULL
);

ALTER TABLE storyos.authoritative_revisions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.authoritative_revisions FORCE ROW LEVEL SECURITY;

CREATE POLICY authoritative_revisions_exact_scope ON storyos.authoritative_revisions USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.authoritative_revisions TO storyos_runtime;

CREATE TABLE storyos.authoritative_heads (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    manuscript_object_id uuid NOT NULL,
    current_revision_id uuid NOT NULL,
    CONSTRAINT authoritative_heads_pkey PRIMARY KEY (owner_user_id, project_id, manuscript_object_id),
    CONSTRAINT authoritative_heads_owner_user_id_project_id_manuscript_ob_fkey FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, current_revision_id) REFERENCES storyos.authoritative_revisions(owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL
);

ALTER TABLE storyos.authoritative_heads ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.authoritative_heads FORCE ROW LEVEL SECURITY;

CREATE POLICY authoritative_heads_exact_scope ON storyos.authoritative_heads USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.authoritative_heads TO storyos_runtime;

-- Project Command Challenges and command idempotency

CREATE TABLE storyos.command_idempotency (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    command_kind text NOT NULL,
    idempotency_key uuid NOT NULL,
    canonical_command_digest text NOT NULL,
    outcome_kind text DEFAULT 'pending'::text NOT NULL,
    result_reference text,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    acknowledgement_format text,
    response_project jsonb,
    response_assistance jsonb,
    response_editor_session jsonb,
    CONSTRAINT command_idempotency_acknowledgement_evidence CHECK (((((acknowledgement_format IS NULL) AND (response_project IS NULL)) OR ((acknowledgement_format IS NOT NULL) AND (response_project IS NOT NULL))) IS TRUE)),
    CONSTRAINT command_idempotency_outcome_kind_check CHECK ((outcome_kind = ANY (ARRAY['pending'::text, 'in_progress'::text, 'settled'::text]))),
    CONSTRAINT command_idempotency_response_assistance_evidence CHECK (((NOT (acknowledgement_format IS DISTINCT FROM 'command_response_project_assistance.v1'::text)) = (response_assistance IS NOT NULL))),
    CONSTRAINT command_idempotency_response_editor_session_evidence CHECK (((response_editor_session IS NULL) OR ((command_kind = 'createEditorSession'::text) AND (outcome_kind = 'settled'::text)))),
    CONSTRAINT command_idempotency_pkey PRIMARY KEY (owner_user_id, project_id, command_kind, idempotency_key),
    CONSTRAINT command_idempotency_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id)
);

ALTER TABLE storyos.command_idempotency ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.command_idempotency FORCE ROW LEVEL SECURITY;

CREATE POLICY command_idempotency_exact_scope ON storyos.command_idempotency USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.command_idempotency TO storyos_runtime;

CREATE TABLE storyos.project_command_challenge_rate_guards (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    client_session_generation numeric(20,0) NOT NULL,
    policy_revision text NOT NULL,
    CONSTRAINT project_command_challenge_rate__client_session_generation_check CHECK (((client_session_generation >= (0)::numeric) AND (client_session_generation <= '18446744073709551615'::numeric))),
    CONSTRAINT project_command_challenge_rate_guards_pkey PRIMARY KEY (owner_user_id, project_id, client_session_generation, policy_revision),
    CONSTRAINT project_command_challenge_rate_gu_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id)
);

ALTER TABLE storyos.project_command_challenge_rate_guards ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_command_challenge_rate_guards FORCE ROW LEVEL SECURITY;

CREATE POLICY project_command_challenge_rate_guards_exact_scope ON storyos.project_command_challenge_rate_guards USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.project_command_challenge_rate_guards TO storyos_runtime;

CREATE TABLE storyos.project_command_challenge_rate_windows (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    client_session_generation numeric(20,0) NOT NULL,
    policy_revision text NOT NULL,
    window_started_at timestamp with time zone NOT NULL,
    issued_count smallint DEFAULT 0 NOT NULL,
    CONSTRAINT project_command_challenge_rate_client_session_generation_check1 CHECK (((client_session_generation >= (0)::numeric) AND (client_session_generation <= '18446744073709551615'::numeric))),
    CONSTRAINT project_command_challenge_rate_windows_issued_count_check CHECK ((((policy_revision = 'storyos.project-command-challenge-rate.fixed-window.v1'::text) AND ((issued_count >= 0) AND (issued_count <= 10))) OR ((policy_revision = 'storyos.project-command-challenge-rate.author-edit.fixed-window.v1'::text) AND ((issued_count >= 0) AND (issued_count <= 120))))),
    CONSTRAINT project_command_challenge_rate_windows_pkey PRIMARY KEY (owner_user_id, project_id, client_session_generation, policy_revision, window_started_at),
    CONSTRAINT project_command_challenge_rat_owner_user_id_project_id_cli_fkey FOREIGN KEY (owner_user_id, project_id, client_session_generation, policy_revision) REFERENCES storyos.project_command_challenge_rate_guards(owner_user_id, project_id, client_session_generation, policy_revision)
);

ALTER TABLE storyos.project_command_challenge_rate_windows ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_command_challenge_rate_windows FORCE ROW LEVEL SECURITY;

CREATE POLICY project_command_challenge_rate_windows_exact_scope ON storyos.project_command_challenge_rate_windows USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.project_command_challenge_rate_windows TO storyos_runtime;

CREATE TABLE storyos.project_command_challenges (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    command_kind text NOT NULL,
    idempotency_key uuid NOT NULL,
    client_session_binding_digest text NOT NULL,
    client_session_generation numeric(20,0) NOT NULL,
    client_contract_revision text NOT NULL,
    security_policy_revision text NOT NULL,
    limit_profile_revision text NOT NULL,
    challenge_rate_policy_revision text NOT NULL,
    challenge_rate_window_started_at timestamp with time zone NOT NULL,
    method text NOT NULL,
    route_template text NOT NULL,
    command_schema text NOT NULL,
    canonical_command_digest text NOT NULL,
    nonce_digest text NOT NULL,
    issued_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    expires_at timestamp with time zone DEFAULT (clock_timestamp() + '00:05:00'::interval) NOT NULL,
    consumed_at timestamp with time zone,
    CONSTRAINT project_command_challenges_client_session_generation_check CHECK (((client_session_generation >= (0)::numeric) AND (client_session_generation <= '18446744073709551615'::numeric))),
    CONSTRAINT project_command_challenges_consumption_key UNIQUE (owner_user_id, project_id, command_kind, idempotency_key, consumed_at, expires_at),
    CONSTRAINT project_command_challenges_nonce_digest_key UNIQUE (nonce_digest),
    CONSTRAINT project_command_challenges_pkey PRIMARY KEY (owner_user_id, project_id, command_kind, idempotency_key),
    CONSTRAINT project_command_challenges_owner_user_id_project_id_client_fkey FOREIGN KEY (owner_user_id, project_id, client_session_generation, challenge_rate_policy_revision, challenge_rate_window_started_at) REFERENCES storyos.project_command_challenge_rate_windows(owner_user_id, project_id, client_session_generation, policy_revision, window_started_at),
    CONSTRAINT project_command_challenges_owner_user_id_project_id_comman_fkey FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key) REFERENCES storyos.command_idempotency(owner_user_id, project_id, command_kind, idempotency_key)
);

ALTER TABLE storyos.project_command_challenges ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_command_challenges FORCE ROW LEVEL SECURITY;

CREATE POLICY project_command_challenges_exact_scope ON storyos.project_command_challenges USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.project_command_challenges TO storyos_runtime;

-- Editor Sessions and writer generations

CREATE TABLE storyos.editor_sessions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    editor_session_id uuid NOT NULL,
    client_session_binding_ref text NOT NULL,
    client_session_generation numeric(20,0) NOT NULL,
    client_contract_revision text NOT NULL,
    security_policy_revision text NOT NULL,
    opened_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    disposition text DEFAULT 'open'::text NOT NULL,
    CONSTRAINT editor_sessions_client_session_generation_check CHECK (((client_session_generation >= (0)::numeric) AND (client_session_generation <= '18446744073709551615'::numeric))),
    CONSTRAINT editor_sessions_disposition_check CHECK ((disposition = 'open'::text)),
    CONSTRAINT editor_sessions_pkey PRIMARY KEY (owner_user_id, project_id, editor_session_id),
    CONSTRAINT editor_sessions_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

ALTER TABLE storyos.editor_sessions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.editor_sessions FORCE ROW LEVEL SECURITY;

CREATE POLICY editor_sessions_exact_scope ON storyos.editor_sessions USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.editor_sessions TO storyos_runtime;

CREATE TABLE storyos.project_writer_generations (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    writer_generation numeric(20,0) NOT NULL,
    current_editor_session_id uuid NOT NULL,
    CONSTRAINT project_writer_generations_writer_generation_check CHECK (((writer_generation >= (1)::numeric) AND (writer_generation <= '18446744073709551615'::numeric))),
    CONSTRAINT project_writer_generations_pkey PRIMARY KEY (owner_user_id, project_id, writer_generation),
    CONSTRAINT project_writer_generations_session_generation_key UNIQUE (owner_user_id, project_id, current_editor_session_id, writer_generation),
    CONSTRAINT project_writer_generations_owner_user_id_project_id_curren_fkey FOREIGN KEY (owner_user_id, project_id, current_editor_session_id) REFERENCES storyos.editor_sessions(owner_user_id, project_id, editor_session_id) MATCH FULL
);

ALTER TABLE storyos.project_writer_generations ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_writer_generations FORCE ROW LEVEL SECURITY;

CREATE POLICY project_writer_generations_exact_scope ON storyos.project_writer_generations USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_writer_generations TO storyos_runtime;

CREATE TABLE storyos.editor_session_base_snapshots (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    snapshot_id uuid NOT NULL,
    editor_session_id uuid NOT NULL,
    chapter_object_id uuid NOT NULL,
    authoritative_revision_id uuid NOT NULL,
    project_activity_position numeric(20,0) DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT editor_session_base_snapshots_project_activity_position_check CHECK (((project_activity_position >= (0)::numeric) AND (project_activity_position <= '18446744073709551615'::numeric))),
    CONSTRAINT editor_session_base_snapshots_owner_user_id_project_id_edit_key UNIQUE (owner_user_id, project_id, editor_session_id),
    CONSTRAINT editor_session_base_snapshots_pkey PRIMARY KEY (owner_user_id, project_id, snapshot_id),
    CONSTRAINT editor_session_base_snapshots_owner_user_id_project_id_cha_fkey FOREIGN KEY (owner_user_id, project_id, chapter_object_id, authoritative_revision_id) REFERENCES storyos.authoritative_revisions(owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL,
    CONSTRAINT editor_session_base_snapshots_owner_user_id_project_id_edi_fkey FOREIGN KEY (owner_user_id, project_id, editor_session_id) REFERENCES storyos.editor_sessions(owner_user_id, project_id, editor_session_id) MATCH FULL
);

CREATE INDEX editor_session_base_snapshots_revision_fk_idx ON storyos.editor_session_base_snapshots USING btree (owner_user_id, project_id, chapter_object_id, authoritative_revision_id);

ALTER TABLE storyos.editor_session_base_snapshots ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.editor_session_base_snapshots FORCE ROW LEVEL SECURITY;

CREATE POLICY editor_session_base_snapshots_exact_scope ON storyos.editor_session_base_snapshots USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.editor_session_base_snapshots TO storyos_runtime;

-- Author Command Admissions, Receipts, Author Actions, and Project Activity

CREATE TABLE storyos.author_command_admissions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    author_command_admission_id uuid NOT NULL,
    command_id uuid NOT NULL,
    editor_session_id uuid,
    writer_generation numeric(20,0),
    client_session_binding_ref text NOT NULL,
    client_session_generation numeric(20,0) NOT NULL,
    client_contract_revision text NOT NULL,
    security_policy_revision text NOT NULL,
    action_class storyos.action_class NOT NULL,
    method text NOT NULL,
    route_template text NOT NULL,
    command_schema text NOT NULL,
    command_kind storyos.command_kind NOT NULL,
    canonical_command_digest text NOT NULL,
    idempotency_key uuid NOT NULL,
    challenge_consumed_at timestamp with time zone,
    challenge_expires_at timestamp with time zone,
    correlation_id uuid NOT NULL,
    chapter_object_id uuid,
    expected_authoritative_revision_id uuid,
    expected_proposal_head_revision_ids uuid[] NOT NULL,
    target_refs text[] NOT NULL,
    observed_ownership_partition text,
    editor_contract_revision text NOT NULL,
    undo_group_id uuid,
    completed_intent_record_id uuid,
    local_intent_sequence numeric(20,0),
    command_payload jsonb NOT NULL,
    issued_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT author_command_admissions_client_session_generation_check CHECK (((client_session_generation >= (0)::numeric) AND (client_session_generation <= '18446744073709551615'::numeric))),
    CONSTRAINT author_command_admissions_command_payload_check CHECK ((jsonb_typeof(command_payload) = 'object'::text)),
    CONSTRAINT author_command_admissions_no_draft_retry_source CHECK ((NOT (command_payload ? 'retry_source'::text))),
    CONSTRAINT author_command_admissions_command_shape CHECK ((
        (command_kind = 'applyAuthorEdit'::text
            AND action_class = 'direct_editor_action'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND chapter_object_id IS NOT NULL
            AND expected_authoritative_revision_id IS NOT NULL
            AND observed_ownership_partition = ANY (ARRAY['authoritative'::text, 'mixed'::text])
            AND (((observed_ownership_partition = 'authoritative'::text) AND (expected_proposal_head_revision_ids = '{}'::uuid[])) OR ((observed_ownership_partition = 'mixed'::text) AND (cardinality(expected_proposal_head_revision_ids) >= 1)))
            AND undo_group_id IS NOT NULL
            AND completed_intent_record_id IS NOT NULL
            AND local_intent_sequence IS NOT NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'takeOverProjectWriter'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'createProject'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NULL
            AND challenge_expires_at IS NULL)
        OR (command_kind = 'updateProject'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'createVolume'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'createChapter'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'updateVolume'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'updateChapter'::text
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = ANY (ARRAY['deleteChapter'::text, 'deleteVolume'::text, 'exportHumanReadableManuscript'::text, 'exportProjectArchive'::text, 'updateProjectAssistance'::text, 'setProjectToolEnablement'::text])
            AND action_class = 'explicit_project_command'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'setCurrentChapter'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND (((chapter_object_id IS NOT NULL) AND (expected_authoritative_revision_id IS NOT NULL)) OR ((chapter_object_id IS NULL) AND (expected_authoritative_revision_id IS NULL)))
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'undoLatestAuthorAction'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND (((chapter_object_id IS NOT NULL) AND (expected_authoritative_revision_id IS NOT NULL)) OR ((chapter_object_id IS NULL) AND (expected_authoritative_revision_id IS NULL)))
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'createAgentRun'::text
            AND action_class = 'agent_run_start'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = ANY (ARRAY['pauseAgentRun'::text, 'cancelAgentRun'::text])
            AND action_class = 'agent_run_control'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'acceptProposal'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND chapter_object_id IS NOT NULL
            AND expected_authoritative_revision_id IS NOT NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'rejectProposalOperations'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND chapter_object_id IS NOT NULL
            AND expected_authoritative_revision_id IS NOT NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'reopenRejectedOperations'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND chapter_object_id IS NOT NULL
            AND expected_authoritative_revision_id IS NOT NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = ANY (ARRAY['completeReadyPartialProposal'::text, 'continueProposalGeneration'::text])
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND chapter_object_id IS NOT NULL
            AND expected_authoritative_revision_id IS NOT NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'replanProposal'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND chapter_object_id IS NOT NULL
            AND expected_authoritative_revision_id IS NOT NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'withdrawProposal'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND chapter_object_id IS NOT NULL
            AND expected_authoritative_revision_id IS NOT NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'reopenWithdrawnProposal'::text
            AND action_class = 'explicit_editor_command'::text
            AND editor_session_id IS NOT NULL
            AND writer_generation IS NOT NULL
            AND chapter_object_id IS NOT NULL
            AND expected_authoritative_revision_id IS NOT NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
        OR (command_kind = 'steerAgentRun'::text
            AND action_class = 'agent_run_control'::text
            AND editor_session_id IS NULL
            AND writer_generation IS NULL
            AND chapter_object_id IS NULL
            AND expected_authoritative_revision_id IS NULL
            AND expected_proposal_head_revision_ids = '{}'::uuid[]
            AND target_refs = '{}'::text[]
            AND observed_ownership_partition IS NULL
            AND undo_group_id IS NULL
            AND completed_intent_record_id IS NULL
            AND local_intent_sequence IS NULL
            AND challenge_consumed_at IS NOT NULL
            AND challenge_expires_at IS NOT NULL)
    ) IS TRUE),
    CONSTRAINT author_command_admissions_local_intent_sequence_check CHECK (((local_intent_sequence >= (1)::numeric) AND (local_intent_sequence <= '18446744073709551615'::numeric))),
    CONSTRAINT author_command_admissions_observed_ownership_partition_check CHECK (((observed_ownership_partition IS NULL) OR (observed_ownership_partition = ANY (ARRAY['authoritative'::text, 'mixed'::text])))),
    CONSTRAINT author_command_admissions_writer_generation_check CHECK (((writer_generation >= (1)::numeric) AND (writer_generation <= '18446744073709551615'::numeric))),
    CONSTRAINT author_command_admissions_owner_user_id_project_id_author_c_key UNIQUE (owner_user_id, project_id, author_command_admission_id, command_id, command_kind, canonical_command_digest, idempotency_key),
    CONSTRAINT author_command_admissions_owner_user_id_project_id_command__key UNIQUE (owner_user_id, project_id, command_id),
    CONSTRAINT author_command_admissions_owner_user_id_project_id_command_key1 UNIQUE (owner_user_id, project_id, command_kind, idempotency_key),
    CONSTRAINT author_command_admissions_pkey PRIMARY KEY (owner_user_id, project_id, author_command_admission_id),
    CONSTRAINT author_command_admissions_editor_session_fkey FOREIGN KEY (owner_user_id, project_id, editor_session_id) REFERENCES storyos.editor_sessions(owner_user_id, project_id, editor_session_id),
    CONSTRAINT author_command_admissions_owner_user_id_project_id_command_fkey FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key) REFERENCES storyos.command_idempotency(owner_user_id, project_id, command_kind, idempotency_key) MATCH FULL,
    CONSTRAINT author_command_admissions_project_command_challenge_fkey FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key, challenge_consumed_at, challenge_expires_at) REFERENCES storyos.project_command_challenges(owner_user_id, project_id, command_kind, idempotency_key, consumed_at, expires_at),
    CONSTRAINT author_command_admissions_writer_generation_fkey FOREIGN KEY (owner_user_id, project_id, writer_generation) REFERENCES storyos.project_writer_generations(owner_user_id, project_id, writer_generation)
);

CREATE INDEX author_command_admissions_editor_session_fk_idx ON storyos.author_command_admissions USING btree (owner_user_id, project_id, editor_session_id);

CREATE INDEX author_command_admissions_revision_fk_idx ON storyos.author_command_admissions USING btree (owner_user_id, project_id, chapter_object_id, expected_authoritative_revision_id);

CREATE INDEX author_command_admissions_writer_generation_fk_idx ON storyos.author_command_admissions USING btree (owner_user_id, project_id, editor_session_id, writer_generation);

CREATE INDEX author_command_admissions_writer_generation_history_fk_idx ON storyos.author_command_admissions USING btree (owner_user_id, project_id, writer_generation);

ALTER TABLE storyos.author_command_admissions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.author_command_admissions FORCE ROW LEVEL SECURITY;

CREATE POLICY author_command_admissions_exact_scope ON storyos.author_command_admissions USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.author_command_admissions TO storyos_runtime;

CREATE FUNCTION storyos.require_writer_generation_admission() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
DECLARE
  current_generation numeric(20, 0);
  current_session uuid;
BEGIN
  SELECT writer.writer_generation, writer.current_editor_session_id
    INTO current_generation, current_session
    FROM storyos.project_writer_generations AS writer
   WHERE writer.owner_user_id = NEW.owner_user_id
     AND writer.project_id = NEW.project_id
   ORDER BY writer.writer_generation DESC
   LIMIT 1;
  IF NEW.command_kind = 'applyAuthorEdit' THEN
    IF NEW.writer_generation IS DISTINCT FROM current_generation
       OR NEW.editor_session_id IS DISTINCT FROM current_session THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'ApplyAuthorEdit Admission requires the current writer session and generation';
    END IF;
    IF NOT EXISTS (
      SELECT 1
        FROM storyos.authoritative_revisions AS revision
       WHERE revision.owner_user_id = NEW.owner_user_id
         AND revision.project_id = NEW.project_id
         AND revision.manuscript_object_id = NEW.chapter_object_id
         AND revision.revision_id = NEW.expected_authoritative_revision_id
    ) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23503',
        MESSAGE = 'ApplyAuthorEdit Admission requires an exact authoritative revision';
    END IF;
  ELSIF NEW.command_kind = 'takeOverProjectWriter' THEN
    IF NEW.writer_generation IS DISTINCT FROM current_generation
       OR NEW.editor_session_id IS NOT DISTINCT FROM current_session THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'takeOverProjectWriter Admission requires an observer of the current generation';
    END IF;
  END IF;
  RETURN NEW;
END
$$;

CREATE TRIGGER author_command_admissions_writer_generation BEFORE INSERT ON storyos.author_command_admissions FOR EACH ROW WHEN (((new.action_class <> 'agent_run_control'::text) AND (new.command_kind <> ALL (ARRAY['createProject'::text, 'updateProject'::text, 'createVolume'::text, 'createChapter'::text, 'updateVolume'::text, 'updateChapter'::text, 'deleteChapter'::text, 'deleteVolume'::text, 'exportHumanReadableManuscript'::text, 'exportProjectArchive'::text, 'updateProjectAssistance'::text, 'setProjectToolEnablement'::text, 'createAgentRun'::text])))) EXECUTE FUNCTION storyos.require_writer_generation_admission();

CREATE TABLE storyos.scope_counters (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    author_action_sequence numeric(20,0) DEFAULT 0 NOT NULL,
    authoritative_commit_sequence numeric(20,0) DEFAULT 0 NOT NULL,
    project_activity_position numeric(20,0) DEFAULT 0 NOT NULL,
    CONSTRAINT scope_counters_author_action_sequence_check CHECK (((author_action_sequence >= (0)::numeric) AND (author_action_sequence <= '18446744073709551614'::numeric))),
    CONSTRAINT scope_counters_authoritative_commit_sequence_check CHECK (((authoritative_commit_sequence >= (0)::numeric) AND (authoritative_commit_sequence <= '18446744073709551614'::numeric))),
    CONSTRAINT scope_counters_project_activity_position_check CHECK (((project_activity_position >= (0)::numeric) AND (project_activity_position <= '18446744073709551614'::numeric))),
    CONSTRAINT scope_counters_pkey PRIMARY KEY (owner_user_id, project_id),
    CONSTRAINT scope_counters_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

ALTER TABLE storyos.scope_counters ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.scope_counters FORCE ROW LEVEL SECURITY;

CREATE POLICY scope_counters_exact_scope ON storyos.scope_counters USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.scope_counters TO storyos_runtime;

CREATE TABLE storyos.authoritative_revision_envelopes (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    manuscript_object_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    parent_revision_id uuid NOT NULL,
    schema_revision text NOT NULL,
    creator_kind text NOT NULL,
    creator_ref uuid NOT NULL,
    receipt_id uuid NOT NULL,
    receipt_result_kind text NOT NULL,
    cause_kind text NOT NULL,
    payload_digest text NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT authoritative_revision_envelopes_cause_kind_check CHECK ((cause_kind = 'direct_author_action'::text)),
    CONSTRAINT authoritative_revision_envelopes_creator_kind_check CHECK ((creator_kind = 'author_command_admission'::text)),
    CONSTRAINT authoritative_revision_envelopes_receipt_result_kind_check CHECK ((receipt_result_kind = 'authoritative_applied'::text)),
    CONSTRAINT authoritative_revision_envelo_owner_user_id_project_id_crea_key UNIQUE (owner_user_id, project_id, creator_ref),
    CONSTRAINT authoritative_revision_envelo_owner_user_id_project_id_rec_key1 UNIQUE (owner_user_id, project_id, receipt_id, receipt_result_kind, manuscript_object_id, parent_revision_id, revision_id),
    CONSTRAINT authoritative_revision_envelo_owner_user_id_project_id_rece_key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT authoritative_revision_envelopes_pkey PRIMARY KEY (owner_user_id, project_id, manuscript_object_id, revision_id),
    CONSTRAINT authoritative_revision_envel_owner_user_id_project_id_man_fkey1 FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, parent_revision_id) REFERENCES storyos.authoritative_revisions(owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL,
    CONSTRAINT authoritative_revision_envelo_owner_user_id_project_id_cre_fkey FOREIGN KEY (owner_user_id, project_id, creator_ref) REFERENCES storyos.author_command_admissions(owner_user_id, project_id, author_command_admission_id) MATCH FULL,
    CONSTRAINT authoritative_revision_envelo_owner_user_id_project_id_man_fkey FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, revision_id) REFERENCES storyos.authoritative_revisions(owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL
);

CREATE TABLE storyos.authoritative_commits (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    authoritative_commit_id uuid NOT NULL,
    authoritative_commit_sequence numeric(20,0) NOT NULL,
    manuscript_object_id uuid,
    prior_revision_id uuid,
    resulting_revision_id uuid,
    author_command_admission_id uuid NOT NULL,
    receipt_id uuid NOT NULL,
    receipt_result_kind text NOT NULL,
    committed_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    prior_manuscript_tree_revision numeric(20,0),
    resulting_manuscript_tree_revision numeric(20,0),
    affected_volume_id uuid,
    affected_chapter_id uuid,
    CONSTRAINT authoritative_commits_authoritative_commit_sequence_check CHECK (((authoritative_commit_sequence >= (1)::numeric) AND (authoritative_commit_sequence <= '18446744073709551615'::numeric))),
    CONSTRAINT authoritative_commits_binding_shape CHECK (((((manuscript_object_id IS NOT NULL) AND (prior_revision_id IS NOT NULL) AND (resulting_revision_id IS NOT NULL) AND (prior_manuscript_tree_revision IS NULL) AND (resulting_manuscript_tree_revision IS NULL) AND (affected_volume_id IS NULL) AND (affected_chapter_id IS NULL)) OR ((prior_manuscript_tree_revision IS NOT NULL) AND (resulting_manuscript_tree_revision IS NOT NULL) AND ((resulting_manuscript_tree_revision = (prior_manuscript_tree_revision + (1)::numeric)) OR (resulting_manuscript_tree_revision = (prior_manuscript_tree_revision - (1)::numeric))) AND ((affected_volume_id IS NOT NULL) <> (affected_chapter_id IS NOT NULL)) AND (((manuscript_object_id IS NULL) AND (prior_revision_id IS NULL) AND (resulting_revision_id IS NULL)) OR ((manuscript_object_id IS NOT NULL) AND (prior_revision_id IS NOT NULL) AND (resulting_revision_id IS NOT NULL)) OR ((manuscript_object_id IS NOT NULL) AND (prior_revision_id IS NULL) AND (resulting_revision_id IS NOT NULL) AND (affected_chapter_id IS NOT NULL) AND (manuscript_object_id = affected_chapter_id))))) IS TRUE)),
    CONSTRAINT authoritative_commits_prior_manuscript_tree_revision_check CHECK (((prior_manuscript_tree_revision IS NULL) OR ((prior_manuscript_tree_revision >= (0)::numeric) AND (prior_manuscript_tree_revision <= '18446744073709551615'::numeric)))),
    CONSTRAINT authoritative_commits_receipt_result_kind_check CHECK ((receipt_result_kind = 'authoritative_applied'::text)),
    CONSTRAINT authoritative_commits_resulting_manuscript_tree_revision_check CHECK (((resulting_manuscript_tree_revision IS NULL) OR ((resulting_manuscript_tree_revision >= (1)::numeric) AND (resulting_manuscript_tree_revision <= '18446744073709551615'::numeric)))),
    CONSTRAINT authoritative_commits_owner_user_id_project_id_author_comma_key UNIQUE (owner_user_id, project_id, author_command_admission_id),
    CONSTRAINT authoritative_commits_owner_user_id_project_id_authoritati_key1 UNIQUE (owner_user_id, project_id, authoritative_commit_id, resulting_revision_id),
    CONSTRAINT authoritative_commits_owner_user_id_project_id_authoritativ_key UNIQUE (owner_user_id, project_id, authoritative_commit_sequence),
    CONSTRAINT authoritative_commits_owner_user_id_project_id_receipt_id__key1 UNIQUE (owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id, resulting_revision_id),
    CONSTRAINT authoritative_commits_owner_user_id_project_id_receipt_id_key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT authoritative_commits_owner_user_id_project_id_receipt_id_r_key UNIQUE (owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id),
    CONSTRAINT authoritative_commits_pkey PRIMARY KEY (owner_user_id, project_id, authoritative_commit_id),
    CONSTRAINT authoritative_commits_affected_chapter_fk FOREIGN KEY (owner_user_id, project_id, affected_chapter_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id),
    CONSTRAINT authoritative_commits_affected_volume_fk FOREIGN KEY (owner_user_id, project_id, affected_volume_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id),
    CONSTRAINT authoritative_commits_envelope_fk FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind, manuscript_object_id, prior_revision_id, resulting_revision_id) REFERENCES storyos.authoritative_revision_envelopes(owner_user_id, project_id, receipt_id, receipt_result_kind, manuscript_object_id, parent_revision_id, revision_id) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT authoritative_commits_owner_user_id_project_id_author_comm_fkey FOREIGN KEY (owner_user_id, project_id, author_command_admission_id) REFERENCES storyos.author_command_admissions(owner_user_id, project_id, author_command_admission_id) MATCH FULL,
    CONSTRAINT authoritative_commits_prior_revision_fk FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, prior_revision_id) REFERENCES storyos.authoritative_revisions(owner_user_id, project_id, manuscript_object_id, revision_id),
    CONSTRAINT authoritative_commits_resulting_revision_fk FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, resulting_revision_id) REFERENCES storyos.authoritative_revisions(owner_user_id, project_id, manuscript_object_id, revision_id)
);

CREATE TABLE storyos.domain_receipts (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    receipt_id uuid NOT NULL,
    author_command_admission_id uuid,
    command_id uuid NOT NULL,
    command_kind storyos.command_kind NOT NULL,
    command_digest text NOT NULL,
    idempotency_key uuid NOT NULL,
    producer_cause text NOT NULL,
    expected_heads uuid[] NOT NULL,
    prior_heads uuid[] NOT NULL,
    resulting_heads uuid[] NOT NULL,
    authoritative_revision_ids uuid[] NOT NULL,
    proposal_revision_ids uuid[] NOT NULL,
    authoritative_commit_ids uuid[] NOT NULL,
    draft_artifact_refs text[] NOT NULL,
    artifact_lifecycle_event_refs text[] NOT NULL,
    condition_refs text[] NOT NULL,
    result_kind storyos.result_kind NOT NULL,
    result_payload jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT domain_receipts_common_shape CHECK ((
        ((((result_kind <> 'proposal_revised'::text) AND (cardinality(proposal_revision_ids) = 0)) OR ((result_kind = 'proposal_revised'::text) AND (cardinality(proposal_revision_ids) = 1) AND (array_dims(proposal_revision_ids) = '[1:1]'::text)))
            AND cardinality(draft_artifact_refs) = 0
            AND cardinality(artifact_lifecycle_event_refs) = 0
            AND ((cardinality(condition_refs) = 0) OR ((command_kind = 'acceptProposal'::text) AND (result_kind = 'conflicted'::text) AND (cardinality(condition_refs) = 1)))
            AND array_position(expected_heads, NULL::uuid) IS NULL
            AND array_position(prior_heads, NULL::uuid) IS NULL
            AND array_position(resulting_heads, NULL::uuid) IS NULL
            AND array_position(authoritative_revision_ids, NULL::uuid) IS NULL
            AND array_position(proposal_revision_ids, NULL::uuid) IS NULL
            AND array_position(authoritative_commit_ids, NULL::uuid) IS NULL
            AND array_position(draft_artifact_refs, NULL::text) IS NULL
            AND array_position(artifact_lifecycle_event_refs, NULL::text) IS NULL
            AND array_position(condition_refs, NULL::text) IS NULL
            AND (((command_kind <> ALL (ARRAY['createProject'::text, 'updateProject'::text, 'createVolume'::text, 'createChapter'::text, 'updateVolume'::text, 'updateChapter'::text, 'deleteChapter'::text, 'deleteVolume'::text, 'exportHumanReadableManuscript'::text, 'exportProjectArchive'::text, 'updateProjectAssistance'::text, 'setProjectToolEnablement'::text, 'createAgentRun'::text, 'pauseAgentRun'::text, 'cancelAgentRun'::text])) AND (cardinality(expected_heads) = 1) AND (cardinality(prior_heads) = 1) AND (cardinality(resulting_heads) = 1) AND (array_dims(expected_heads) = '[1:1]'::text) AND (array_dims(prior_heads) = '[1:1]'::text) AND (array_dims(resulting_heads) = '[1:1]'::text)) OR ((command_kind = ANY (ARRAY['createProject'::text, 'updateProject'::text, 'createVolume'::text, 'createChapter'::text, 'updateVolume'::text, 'updateChapter'::text, 'deleteChapter'::text, 'deleteVolume'::text, 'exportHumanReadableManuscript'::text, 'exportProjectArchive'::text, 'updateProjectAssistance'::text, 'setProjectToolEnablement'::text, 'createAgentRun'::text, 'pauseAgentRun'::text, 'cancelAgentRun'::text])) AND (cardinality(expected_heads) = 0) AND (cardinality(prior_heads) = 0) AND (cardinality(resulting_heads) = 0))))
        OR (command_kind = 'replanProposal'::text
            AND cardinality(expected_heads) = 1
            AND cardinality(prior_heads) <= 1
            AND resulting_heads = prior_heads
            AND authoritative_revision_ids = '{}'::uuid[]
            AND authoritative_commit_ids = '{}'::uuid[]
            AND condition_refs = '{}'::text[]
            AND draft_artifact_refs = '{}'::text[]
            AND artifact_lifecycle_event_refs = '{}'::text[]
            AND (((result_kind = 'proposal_revised'::text) AND (expected_heads = prior_heads) AND (cardinality(proposal_revision_ids) = 1) AND (array_position(proposal_revision_ids, NULL::uuid) IS NULL)) OR ((result_kind = ANY (ARRAY['refused'::text, 'conflicted'::text])) AND (proposal_revision_ids = '{}'::uuid[]))))
        OR (command_kind = 'withdrawProposal'::text
            AND cardinality(expected_heads) = 1
            AND cardinality(prior_heads) <= 1
            AND resulting_heads = prior_heads
            AND authoritative_revision_ids = '{}'::uuid[]
            AND authoritative_commit_ids = '{}'::uuid[]
            AND condition_refs = '{}'::text[]
            AND draft_artifact_refs = '{}'::text[]
            AND artifact_lifecycle_event_refs = '{}'::text[]
            AND proposal_revision_ids = '{}'::uuid[]
            AND (((result_kind = 'proposal_closure_changed'::text) AND (expected_heads = prior_heads)) OR (result_kind = ANY (ARRAY['refused'::text, 'conflicted'::text, 'no_effect'::text]))))
        OR (command_kind = 'reopenWithdrawnProposal'::text
            AND cardinality(expected_heads) = 1
            AND cardinality(prior_heads) <= 1
            AND resulting_heads = prior_heads
            AND authoritative_revision_ids = '{}'::uuid[]
            AND authoritative_commit_ids = '{}'::uuid[]
            AND condition_refs = '{}'::text[]
            AND draft_artifact_refs = '{}'::text[]
            AND artifact_lifecycle_event_refs = '{}'::text[]
            AND (((result_kind = 'proposal_revised'::text) AND (expected_heads = prior_heads) AND (cardinality(proposal_revision_ids) = 1) AND (array_position(proposal_revision_ids, NULL::uuid) IS NULL)) OR ((result_kind = ANY (ARRAY['refused'::text, 'conflicted'::text, 'no_effect'::text])) AND (proposal_revision_ids = '{}'::uuid[]))))
        OR (command_kind = 'steerAgentRun'::text
            AND expected_heads = '{}'::uuid[]
            AND prior_heads = '{}'::uuid[]
            AND resulting_heads = '{}'::uuid[]
            AND authoritative_revision_ids = '{}'::uuid[]
            AND proposal_revision_ids = '{}'::uuid[]
            AND authoritative_commit_ids = '{}'::uuid[]
            AND draft_artifact_refs = '{}'::text[]
            AND artifact_lifecycle_event_refs = '{}'::text[]
            AND condition_refs = '{}'::text[])
    ) IS TRUE),
    CONSTRAINT domain_receipts_producer_admission_pair CHECK (((producer_cause = 'author_command_admission'::text) = (author_command_admission_id IS NOT NULL))),
    CONSTRAINT domain_receipts_producer_cause_check CHECK ((producer_cause = ANY (ARRAY['author_command_admission'::text, 'agent_run_decision'::text]))),
    CONSTRAINT domain_receipts_result_payload_check CHECK ((jsonb_typeof(result_payload) = 'object'::text)),
    CONSTRAINT domain_receipts_result_shape CHECK ((
        (command_kind = 'applyAuthorEdit'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 1
            AND cardinality(authoritative_commit_ids) = 1
            AND array_dims(authoritative_revision_ids) = '[1:1]'::text
            AND array_dims(authoritative_commit_ids) = '[1:1]'::text
            AND resulting_heads = authoritative_revision_ids)
        OR (command_kind = 'applyAuthorEdit'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'content_unchanged'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'takeOverProjectWriter'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['writer_takeover_applied'::text, 'writer_takeover_compare_failed'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'createProject'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateProject'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateProject'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'title_unchanged'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateProject'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'stale_project_revision'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'createVolume'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = ANY (ARRAY[0, 1])
            AND ((cardinality(authoritative_commit_ids) = 0) OR (array_dims(authoritative_commit_ids) = '[1:1]'::text)))
        OR (command_kind = 'createVolume'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload ? 'order'::text
            AND (result_payload - 'order'::text) = '{}'::jsonb
            AND jsonb_typeof((result_payload -> 'order'::text)) = 'string'::text
            AND (result_payload ->> 'order'::text) ~ '^[1-9][0-9]*$'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = ANY (ARRAY[0, 1])
            AND ((cardinality(authoritative_commit_ids) = 0) OR (array_dims(authoritative_commit_ids) = '[1:1]'::text)))
        OR (command_kind = 'createVolume'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'stale_tree_revision'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'createVolume'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['invalid_title'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'createChapter'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = ANY (ARRAY[0, 1])
            AND ((cardinality(authoritative_commit_ids) = 0) OR (array_dims(authoritative_commit_ids) = '[1:1]'::text)))
        OR (command_kind = 'createChapter'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload ? 'order'::text
            AND (result_payload - 'order'::text) = '{}'::jsonb
            AND jsonb_typeof((result_payload -> 'order'::text)) = 'string'::text
            AND (result_payload ->> 'order'::text) ~ '^[1-9][0-9]*$'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = ANY (ARRAY[0, 1])
            AND ((cardinality(authoritative_commit_ids) = 0) OR (array_dims(authoritative_commit_ids) = '[1:1]'::text)))
        OR (command_kind = 'createChapter'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'stale_tree_revision'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'createChapter'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['invalid_title'::text, 'invalid_volume_join'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateVolume'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = ANY (ARRAY[0, 1])
            AND ((cardinality(authoritative_commit_ids) = 0) OR (array_dims(authoritative_commit_ids) = '[1:1]'::text)))
        OR (command_kind = 'updateVolume'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'unchanged'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateVolume'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'stale_tree_revision'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateVolume'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['invalid_title'::text, 'invalid_order'::text, 'invalid_volume_join'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateChapter'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = ANY (ARRAY[0, 1])
            AND ((cardinality(authoritative_commit_ids) = 0) OR (array_dims(authoritative_commit_ids) = '[1:1]'::text)))
        OR (command_kind = 'updateChapter'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'unchanged'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateChapter'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'stale_tree_revision'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateChapter'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['invalid_title'::text, 'invalid_order'::text, 'invalid_chapter_join'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'deleteChapter'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = ANY (ARRAY[0, 1])
            AND ((cardinality(authoritative_commit_ids) = 0) OR (array_dims(authoritative_commit_ids) = '[1:1]'::text)))
        OR (command_kind = 'deleteChapter'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'already_removed'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'deleteChapter'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'stale_tree_revision'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'deleteChapter'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['invalid_chapter_join'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'deleteVolume'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = ANY (ARRAY[0, 1])
            AND ((cardinality(authoritative_commit_ids) = 0) OR (array_dims(authoritative_commit_ids) = '[1:1]'::text)))
        OR (command_kind = 'deleteVolume'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'already_removed'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'deleteVolume'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'stale_tree_revision'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'deleteVolume'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['invalid_volume_join'::text, 'nonempty_volume'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'setCurrentChapter'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads)
        OR (command_kind = 'setCurrentChapter'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'already_current'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'setCurrentChapter'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['stale_current_chapter'::text, 'wrong_target_head'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads)
        OR (command_kind = 'setCurrentChapter'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['invalid_chapter_join'::text, 'empty_project'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads)
        OR (command_kind = 'applyAuthorEdit'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND result_payload ? 'current_authoritative_revision_id'::text
            AND ((result_payload - 'reason'::text) - 'current_authoritative_revision_id'::text) = '{}'::jsonb
            AND jsonb_typeof((result_payload -> 'reason'::text)) = 'string'::text
            AND jsonb_typeof((result_payload -> 'current_authoritative_revision_id'::text)) = 'string'::text
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['stale_authoritative_head'::text, 'proposal_head_present'::text, 'ownership_changed'::text])
            AND (result_payload ->> 'current_authoritative_revision_id'::text) = (resulting_heads[1])::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND (((result_payload ->> 'reason'::text) <> 'stale_authoritative_head'::text) OR (expected_heads <> resulting_heads)))
        OR (command_kind = 'applyAuthorEdit'::text
            AND result_kind = 'proposal_revised'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND cardinality(proposal_revision_ids) = 1
            AND array_dims(proposal_revision_ids) = '[1:1]'::text
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'applyAuthorEdit'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND jsonb_typeof((result_payload -> 'reason'::text)) = 'string'::text
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['unsupported_intent_shape'::text, 'invalid_selection'::text, 'target_mismatch'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'exportHumanReadableManuscript'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'exportHumanReadableManuscript'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['pinned_export_source_unavailable'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'exportProjectArchive'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'exportProjectArchive'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['pinned_export_source_unavailable'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'undoLatestAuthorAction'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_commit_ids) = ANY (ARRAY[0, 1])
            AND (((cardinality(authoritative_commit_ids) = 1) AND (array_dims(authoritative_commit_ids) = '[1:1]'::text) AND (((cardinality(authoritative_revision_ids) = 1) AND (array_dims(authoritative_revision_ids) = '[1:1]'::text) AND (resulting_heads = authoritative_revision_ids)) OR ((cardinality(authoritative_revision_ids) = 0) AND (prior_heads = resulting_heads)))) OR ((cardinality(authoritative_commit_ids) = 0) AND (cardinality(authoritative_revision_ids) = 0) AND (prior_heads = resulting_heads))))
        OR (command_kind = 'undoLatestAuthorAction'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['frontier_mismatch'::text, 'wrong_target_head'::text, 'source_binding_changed'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads)
        OR (command_kind = 'undoLatestAuthorAction'::text
            AND result_kind = 'refused'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = ANY (ARRAY['no_frontier'::text, 'barrier'::text, 'source_unavailable'::text])
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads)
        OR (command_kind = 'updateProjectAssistance'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateProjectAssistance'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'availability_unchanged'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'setProjectToolEnablement'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'setProjectToolEnablement'::text
            AND ((result_kind = 'no_effect'::text
                  AND (result_payload ->> 'reason'::text) = 'enablement_unchanged'::text)
                 OR (result_kind = 'refused'::text
                     AND (result_payload ->> 'reason'::text) = ANY (ARRAY['registration_not_offered'::text, 'registration_unavailable'::text])))
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'updateProjectAssistance'::text
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'stale_assistance_revision'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'createAgentRun'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = ANY (ARRAY['pauseAgentRun'::text, 'cancelAgentRun'::text])
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'pauseAgentRun'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'already_paused'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'cancelAgentRun'::text
            AND result_kind = 'no_effect'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'already_cancelled'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = ANY (ARRAY['pauseAgentRun'::text, 'cancelAgentRun'::text])
            AND result_kind = 'conflicted'::text
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'reason'::text) = 'terminal_run'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
        OR (command_kind = 'acceptProposal'::text
            AND result_kind = 'authoritative_applied'::text
            AND result_payload = '{}'::jsonb
            AND cardinality(authoritative_revision_ids) = 1
            AND cardinality(authoritative_commit_ids) = 1
            AND array_dims(authoritative_revision_ids) = '[1:1]'::text
            AND array_dims(authoritative_commit_ids) = '[1:1]'::text
            AND resulting_heads = authoritative_revision_ids)
        OR (command_kind = 'acceptProposal'::text
            AND result_kind = ANY (ARRAY['invalid'::text, 'conflicted'::text, 'refused'::text])
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND jsonb_typeof((result_payload -> 'reason'::text)) = 'string'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'rejectProposalOperations'::text
            AND result_kind = 'proposal_operations_resolved'::text
            AND result_payload ? 'rejection_reason'::text
            AND (result_payload - 'rejection_reason'::text) = '{}'::jsonb
            AND (result_payload ->> 'rejection_reason'::text) = 'author_declined'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'rejectProposalOperations'::text
            AND result_kind = ANY (ARRAY['conflicted'::text, 'refused'::text])
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND jsonb_typeof((result_payload -> 'reason'::text)) = 'string'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'reopenRejectedOperations'::text
            AND result_kind = 'proposal_revised'::text
            AND result_payload = '{"transition": "reopen_rejected"}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND cardinality(proposal_revision_ids) = 1
            AND array_dims(proposal_revision_ids) = '[1:1]'::text
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'reopenRejectedOperations'::text
            AND result_kind = ANY (ARRAY['conflicted'::text, 'refused'::text])
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND jsonb_typeof((result_payload -> 'reason'::text)) = 'string'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'completeReadyPartialProposal'::text
            AND result_kind = 'proposal_generation_completed'::text
            AND result_payload = '{"transition": "generation_completed"}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'completeReadyPartialProposal'::text
            AND result_kind = ANY (ARRAY['conflicted'::text, 'refused'::text])
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND jsonb_typeof((result_payload -> 'reason'::text)) = 'string'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'continueProposalGeneration'::text
            AND result_kind = 'proposal_generation_started'::text
            AND result_payload = '{"transition": "generation_started"}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'continueProposalGeneration'::text
            AND result_kind = ANY (ARRAY['conflicted'::text, 'refused'::text])
            AND result_payload ? 'reason'::text
            AND (result_payload - 'reason'::text) = '{}'::jsonb
            AND jsonb_typeof((result_payload -> 'reason'::text)) = 'string'::text
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0
            AND prior_heads = resulting_heads
            AND expected_heads = resulting_heads)
        OR (command_kind = 'undoLatestAuthorAction'::text
            AND result_kind = 'authoritative_applied'::text
            AND jsonb_typeof(result_payload) = 'object'::text
            AND result_payload ?& ARRAY['proposal_revision_id'::text, 'source_proposal_revision_id'::text, 'project_activity_position'::text]
            AND (result_payload - ARRAY['proposal_revision_id'::text, 'source_proposal_revision_id'::text, 'project_activity_position'::text]) = '{}'::jsonb
            AND (result_payload ->> 'proposal_revision_id'::text) ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'::text
            AND (result_payload ->> 'source_proposal_revision_id'::text) ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'::text
            AND CASE
    WHEN ((jsonb_typeof((result_payload -> 'project_activity_position'::text)) = 'string'::text) AND ((result_payload ->> 'project_activity_position'::text) ~ '^(0|[1-9][0-9]{0,19})$'::text)) THEN (((result_payload ->> 'project_activity_position'::text))::numeric <= '18446744073709551615'::numeric)
    ELSE false
END
            AND authoritative_revision_ids = '{}'::uuid[]
            AND authoritative_commit_ids = '{}'::uuid[]
            AND proposal_revision_ids = '{}'::uuid[]
            AND prior_heads = expected_heads
            AND resulting_heads = expected_heads)
        OR (command_kind = 'replanProposal'::text
            AND jsonb_typeof(result_payload) = 'object'::text
            AND (((result_kind = 'proposal_revised'::text) AND ((result_payload ->> 'transition'::text) = 'replan'::text) AND ((result_payload - ARRAY['transition'::text]) = '{}'::jsonb)) OR ((result_kind = 'conflicted'::text) AND ((result_payload ->> 'reason'::text) = 'changed_head'::text) AND ((result_payload - ARRAY['reason'::text]) = '{}'::jsonb)) OR ((result_kind = 'refused'::text) AND ((result_payload ->> 'reason'::text) = ANY (ARRAY['wrong_scope'::text, 'wrong_admission'::text, 'stale_proposal_revision'::text, 'not_eligible'::text, 'unavailable_proof'::text])) AND ((result_payload - ARRAY['reason'::text]) = '{}'::jsonb))))
        OR (command_kind = 'withdrawProposal'::text
            AND jsonb_typeof(result_payload) = 'object'::text
            AND (((result_kind = 'proposal_closure_changed'::text) AND ((result_payload ->> 'transition'::text) = 'withdraw'::text) AND ((result_payload - ARRAY['transition'::text]) = '{}'::jsonb)) OR ((result_kind = 'conflicted'::text) AND ((result_payload ->> 'reason'::text) = 'changed_head'::text) AND ((result_payload - ARRAY['reason'::text]) = '{}'::jsonb)) OR ((result_kind = 'refused'::text) AND ((result_payload ->> 'reason'::text) = ANY (ARRAY['wrong_scope'::text, 'wrong_admission'::text, 'stale_proposal_revision'::text])) AND ((result_payload - ARRAY['reason'::text]) = '{}'::jsonb)) OR ((result_kind = 'no_effect'::text) AND ((result_payload ->> 'reason'::text) = ANY (ARRAY['unsupported_cause'::text, 'terminal_supersession'::text, 'closure_not_open'::text])) AND ((result_payload - ARRAY['reason'::text]) = '{}'::jsonb))))
        OR (command_kind = 'reopenWithdrawnProposal'::text
            AND jsonb_typeof(result_payload) = 'object'::text
            AND (((result_kind = 'proposal_revised'::text) AND ((result_payload ->> 'transition'::text) = 'reopen_withdrawn'::text) AND ((result_payload - ARRAY['transition'::text]) = '{}'::jsonb)) OR ((result_kind = 'conflicted'::text) AND ((result_payload ->> 'reason'::text) = 'changed_head'::text) AND ((result_payload - ARRAY['reason'::text]) = '{}'::jsonb)) OR ((result_kind = 'refused'::text) AND ((result_payload ->> 'reason'::text) = ANY (ARRAY['wrong_scope'::text, 'wrong_admission'::text, 'stale_proposal_revision'::text])) AND ((result_payload - ARRAY['reason'::text]) = '{}'::jsonb)) OR ((result_kind = 'no_effect'::text) AND ((result_payload ->> 'reason'::text) = ANY (ARRAY['terminal_supersession'::text, 'closure_not_withdrawn'::text, 'withdrawal_event_mismatch'::text])) AND ((result_payload - ARRAY['reason'::text]) = '{}'::jsonb))))
        OR (command_kind = 'steerAgentRun'::text
            AND jsonb_typeof(result_payload) = 'object'::text
            AND (result_payload - ARRAY['reason'::text]) = '{}'::jsonb
            AND (((result_kind = 'no_effect'::text) AND ((result_payload ->> 'reason'::text) = 'steering_retained'::text)) OR ((result_kind = 'conflicted'::text) AND ((result_payload ->> 'reason'::text) = 'terminal_run'::text))))
        OR (command_kind = 'createChapter'::text
            AND result_kind = 'refused'::text
            AND result_payload = '{"reason": "invalid_placement"}'::jsonb
            AND cardinality(authoritative_revision_ids) = 0
            AND cardinality(authoritative_commit_ids) = 0)
    ) IS TRUE),
    CONSTRAINT domain_receipts_owner_user_id_project_id_author_command_ad_key1 UNIQUE (owner_user_id, project_id, author_command_admission_id, receipt_id),
    CONSTRAINT domain_receipts_owner_user_id_project_id_author_command_ad_key2 UNIQUE (owner_user_id, project_id, author_command_admission_id, receipt_id, result_kind),
    CONSTRAINT domain_receipts_owner_user_id_project_id_author_command_adm_key UNIQUE (owner_user_id, project_id, author_command_admission_id),
    CONSTRAINT domain_receipts_owner_user_id_project_id_receipt_id_result__key UNIQUE (owner_user_id, project_id, receipt_id, result_kind),
    CONSTRAINT domain_receipts_pkey PRIMARY KEY (owner_user_id, project_id, receipt_id),
    CONSTRAINT domain_receipts_author_admission_fk FOREIGN KEY (owner_user_id, project_id, author_command_admission_id, command_id, command_kind, command_digest, idempotency_key) REFERENCES storyos.author_command_admissions(owner_user_id, project_id, author_command_admission_id, command_id, command_kind, canonical_command_digest, idempotency_key)
);

ALTER TABLE storyos.authoritative_revision_envelopes
    ADD CONSTRAINT authoritative_revision_envelopes_applied_receipt_fk FOREIGN KEY (owner_user_id, project_id, creator_ref, receipt_id, receipt_result_kind) REFERENCES storyos.domain_receipts(owner_user_id, project_id, author_command_admission_id, receipt_id, result_kind) MATCH FULL DEFERRABLE INITIALLY DEFERRED;

ALTER TABLE storyos.authoritative_commits
    ADD CONSTRAINT authoritative_commits_applied_receipt_fk FOREIGN KEY (owner_user_id, project_id, author_command_admission_id, receipt_id, receipt_result_kind) REFERENCES storyos.domain_receipts(owner_user_id, project_id, author_command_admission_id, receipt_id, result_kind) MATCH FULL DEFERRABLE INITIALLY DEFERRED;

CREATE INDEX authoritative_revision_envelopes_admission_fk_idx ON storyos.authoritative_revision_envelopes USING btree (owner_user_id, project_id, creator_ref);

CREATE INDEX authoritative_revision_envelopes_parent_revision_fk_idx ON storyos.authoritative_revision_envelopes USING btree (owner_user_id, project_id, manuscript_object_id, parent_revision_id);

ALTER TABLE storyos.authoritative_revision_envelopes ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.authoritative_revision_envelopes FORCE ROW LEVEL SECURITY;

CREATE POLICY authoritative_revision_envelopes_exact_scope ON storyos.authoritative_revision_envelopes USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.authoritative_revision_envelopes TO storyos_runtime;

CREATE INDEX authoritative_commits_admission_fk_idx ON storyos.authoritative_commits USING btree (owner_user_id, project_id, author_command_admission_id);

CREATE INDEX authoritative_commits_affected_chapter_fk_idx ON storyos.authoritative_commits USING btree (owner_user_id, project_id, affected_chapter_id);

CREATE INDEX authoritative_commits_affected_volume_fk_idx ON storyos.authoritative_commits USING btree (owner_user_id, project_id, affected_volume_id);

CREATE INDEX authoritative_commits_prior_revision_fk_idx ON storyos.authoritative_commits USING btree (owner_user_id, project_id, manuscript_object_id, prior_revision_id);

CREATE INDEX authoritative_commits_resulting_revision_fk_idx ON storyos.authoritative_commits USING btree (owner_user_id, project_id, manuscript_object_id, resulting_revision_id);

ALTER TABLE storyos.authoritative_commits ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.authoritative_commits FORCE ROW LEVEL SECURITY;

CREATE POLICY authoritative_commits_exact_scope ON storyos.authoritative_commits USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.authoritative_commits TO storyos_runtime;

CREATE INDEX domain_receipts_admission_fk_idx ON storyos.domain_receipts USING btree (owner_user_id, project_id, author_command_admission_id, command_id, command_kind, command_digest, idempotency_key);

CREATE UNIQUE INDEX domain_receipts_agent_run_decision_idempotency ON storyos.domain_receipts USING btree (owner_user_id, project_id, command_kind, idempotency_key) WHERE (producer_cause = 'agent_run_decision'::text);

ALTER TABLE storyos.domain_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.domain_receipts FORCE ROW LEVEL SECURITY;

CREATE POLICY domain_receipts_exact_scope ON storyos.domain_receipts USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.domain_receipts TO storyos_runtime;

CREATE TABLE storyos.author_command_admission_settlements (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    author_command_admission_id uuid NOT NULL,
    settlement_kind text NOT NULL,
    receipt_id uuid NOT NULL,
    settled_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT author_command_admission_settlements_settlement_kind_check CHECK ((settlement_kind = 'receipt_settled'::text)),
    CONSTRAINT author_command_admission_sett_owner_user_id_project_id_rece_key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT author_command_admission_settlements_pkey PRIMARY KEY (owner_user_id, project_id, author_command_admission_id),
    CONSTRAINT author_command_admission_sett_owner_user_id_project_id_aut_fkey FOREIGN KEY (owner_user_id, project_id, author_command_admission_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, author_command_admission_id, receipt_id) MATCH FULL
);

CREATE INDEX author_command_admission_settlements_receipt_fk_idx ON storyos.author_command_admission_settlements USING btree (owner_user_id, project_id, author_command_admission_id, receipt_id);

ALTER TABLE storyos.author_command_admission_settlements ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.author_command_admission_settlements FORCE ROW LEVEL SECURITY;

CREATE POLICY author_command_admission_settlements_exact_scope ON storyos.author_command_admission_settlements USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.author_command_admission_settlements TO storyos_runtime;

CREATE TABLE storyos.author_command_admission_outcome_unknown_observations (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    observation_id uuid NOT NULL,
    observation_sequence numeric(20,0) NOT NULL,
    author_command_admission_id uuid NOT NULL,
    command_id uuid NOT NULL,
    command_kind storyos.command_kind NOT NULL,
    canonical_command_digest text NOT NULL,
    idempotency_key uuid NOT NULL,
    last_provable_boundary text NOT NULL,
    reason text NOT NULL,
    reconciliation_required boolean DEFAULT true NOT NULL,
    observed_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT author_command_admission_outcome__reconciliation_required_check CHECK (reconciliation_required),
    CONSTRAINT author_command_admission_outcome_u_last_provable_boundary_check CHECK ((last_provable_boundary = 'admission_committed'::text)),
    CONSTRAINT author_command_admission_outcome_unk_observation_sequence_check CHECK (((observation_sequence >= (1)::numeric) AND (observation_sequence <= '18446744073709551615'::numeric))),
    CONSTRAINT author_command_admission_outcome_unknown_observati_reason_check CHECK ((reason = 'acknowledgement_missing'::text)),
    CONSTRAINT author_command_admission_outc_owner_user_id_project_id_auth_key UNIQUE (owner_user_id, project_id, author_command_admission_id, observation_sequence),
    CONSTRAINT author_command_admission_outcome_unknown_observations_pkey PRIMARY KEY (owner_user_id, project_id, observation_id),
    CONSTRAINT author_command_admission_outc_owner_user_id_project_id_aut_fkey FOREIGN KEY (owner_user_id, project_id, author_command_admission_id, command_id, command_kind, canonical_command_digest, idempotency_key) REFERENCES storyos.author_command_admissions(owner_user_id, project_id, author_command_admission_id, command_id, command_kind, canonical_command_digest, idempotency_key) MATCH FULL
);

CREATE INDEX author_command_outcome_unknown_admission_fk_idx ON storyos.author_command_admission_outcome_unknown_observations USING btree (owner_user_id, project_id, author_command_admission_id, command_id, command_kind, canonical_command_digest, idempotency_key);

ALTER TABLE storyos.author_command_admission_outcome_unknown_observations ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.author_command_admission_outcome_unknown_observations FORCE ROW LEVEL SECURITY;

CREATE POLICY author_command_outcome_unknown_exact_scope ON storyos.author_command_admission_outcome_unknown_observations USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.author_command_admission_outcome_unknown_observations TO storyos_runtime;

CREATE FUNCTION storyos.prepare_author_command_outcome_unknown_observation() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
DECLARE
  admission record;
  arbiter_outcome_kind text;
  next_sequence numeric(20, 0);
BEGIN
  SELECT command_id, command_kind, canonical_command_digest, idempotency_key
    INTO admission
    FROM storyos.author_command_admissions
   WHERE owner_user_id = NEW.owner_user_id
     AND project_id = NEW.project_id
     AND author_command_admission_id = NEW.author_command_admission_id;
  IF NOT FOUND THEN
    RAISE EXCEPTION USING
      ERRCODE = '23503',
      MESSAGE = 'outcome_unknown requires one exact Author Command Admission';
  END IF;

  NEW.command_id := admission.command_id;
  NEW.command_kind := admission.command_kind;
  NEW.canonical_command_digest := admission.canonical_command_digest;
  NEW.idempotency_key := admission.idempotency_key;

  SELECT outcome_kind
    INTO arbiter_outcome_kind
    FROM storyos.command_idempotency
   WHERE owner_user_id = NEW.owner_user_id
     AND project_id = NEW.project_id
     AND command_kind = admission.command_kind
     AND idempotency_key = admission.idempotency_key
   FOR UPDATE;
  IF NOT FOUND OR arbiter_outcome_kind <> 'in_progress' THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'outcome_unknown requires an in-progress idempotency arbiter';
  END IF;
  IF EXISTS (
    SELECT 1
      FROM storyos.author_command_admission_settlements
     WHERE owner_user_id = NEW.owner_user_id
       AND project_id = NEW.project_id
       AND author_command_admission_id = NEW.author_command_admission_id
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'a terminal Admission cannot append outcome_unknown';
  END IF;

  SELECT COALESCE(max(observation_sequence), 0) + 1
    INTO next_sequence
    FROM storyos.author_command_admission_outcome_unknown_observations
   WHERE owner_user_id = NEW.owner_user_id
     AND project_id = NEW.project_id
     AND author_command_admission_id = NEW.author_command_admission_id;
  IF next_sequence > 18446744073709551615 THEN
    RAISE EXCEPTION USING
      ERRCODE = '22003',
      MESSAGE = 'outcome_unknown observation sequence is exhausted';
  END IF;

  NEW.observation_sequence := next_sequence;
  NEW.observed_at := clock_timestamp();
  RETURN NEW;
END
$$;

CREATE TRIGGER author_command_outcome_unknown_prepare BEFORE INSERT ON storyos.author_command_admission_outcome_unknown_observations FOR EACH ROW EXECUTE FUNCTION storyos.prepare_author_command_outcome_unknown_observation();

CREATE TABLE storyos.author_action_entries (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    author_action_sequence numeric(20,0) NOT NULL,
    disposition text NOT NULL,
    authoritative_commit_id uuid,
    receipt_id uuid NOT NULL,
    receipt_result_kind storyos.receipt_result_kind NOT NULL,
    compensated_source_sequence numeric(20,0),
    CONSTRAINT author_action_entries_compensated_source_sequence_check CHECK (((compensated_source_sequence IS NULL) OR ((compensated_source_sequence >= (1)::numeric) AND (compensated_source_sequence <= '18446744073709551615'::numeric)))),
    CONSTRAINT author_action_entries_disposition_check CHECK ((((disposition = 'forward'::text) AND (compensated_source_sequence IS NULL)) OR ((disposition = 'compensation'::text) AND (compensated_source_sequence IS NOT NULL)))),
    CONSTRAINT author_action_entries_owner_user_id_project_id_receipt_id_key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT author_action_entries_owner_user_id_project_id_receipt_id_r_key UNIQUE (owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id, author_action_sequence),
    CONSTRAINT author_action_entries_pkey PRIMARY KEY (owner_user_id, project_id, author_action_sequence),
    CONSTRAINT author_action_entries_commit_fk FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id) REFERENCES storyos.authoritative_commits(owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id),
    CONSTRAINT author_action_entries_owner_user_id_project_id_receipt_id__fkey FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id, result_kind) MATCH FULL
);

CREATE UNIQUE INDEX author_action_entries_one_compensation_per_source ON storyos.author_action_entries USING btree (owner_user_id, project_id, compensated_source_sequence) WHERE (disposition = 'compensation'::text);

CREATE INDEX author_action_entries_receipt_commit_fk_idx ON storyos.author_action_entries USING btree (owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id, author_action_sequence);

ALTER TABLE storyos.author_action_entries ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.author_action_entries FORCE ROW LEVEL SECURITY;

CREATE POLICY author_action_entries_exact_scope ON storyos.author_action_entries USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.author_action_entries TO storyos_runtime;

CREATE TABLE storyos.project_activity_events (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    project_activity_position numeric(20,0) NOT NULL,
    project_activity_event_id uuid NOT NULL,
    event_kind text NOT NULL,
    receipt_id uuid NOT NULL,
    receipt_result_kind text NOT NULL,
    authoritative_commit_id uuid NOT NULL,
    resulting_revision_id uuid NOT NULL,
    author_action_sequence numeric(20,0) NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT project_activity_events_author_action_sequence_check CHECK (((author_action_sequence >= (1)::numeric) AND (author_action_sequence <= '18446744073709551615'::numeric))),
    CONSTRAINT project_activity_events_event_kind_check CHECK ((event_kind = 'authoritative_author_edit_applied'::text)),
    CONSTRAINT project_activity_events_receipt_result_kind_check CHECK ((receipt_result_kind = 'authoritative_applied'::text)),
    CONSTRAINT project_activity_events_owner_user_id_project_id_project_ac_key UNIQUE (owner_user_id, project_id, project_activity_event_id),
    CONSTRAINT project_activity_events_owner_user_id_project_id_receipt_id_key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT project_activity_events_pkey PRIMARY KEY (owner_user_id, project_id, project_activity_position),
    CONSTRAINT project_activity_events_owner_user_id_project_id_receipt__fkey1 FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id, resulting_revision_id) REFERENCES storyos.authoritative_commits(owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id, resulting_revision_id) MATCH FULL,
    CONSTRAINT project_activity_events_owner_user_id_project_id_receipt__fkey2 FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id, author_action_sequence) REFERENCES storyos.author_action_entries(owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id, author_action_sequence) MATCH FULL,
    CONSTRAINT project_activity_events_owner_user_id_project_id_receipt_i_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id, result_kind) MATCH FULL
);

CREATE INDEX project_activity_events_receipt_result_fk_idx ON storyos.project_activity_events USING btree (owner_user_id, project_id, receipt_id, receipt_result_kind, authoritative_commit_id, author_action_sequence);

ALTER TABLE storyos.project_activity_events ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_activity_events FORCE ROW LEVEL SECURITY;

CREATE POLICY project_activity_events_exact_scope ON storyos.project_activity_events USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_activity_events TO storyos_runtime;

-- Project Snapshots and replay

CREATE TABLE storyos.replay_generations (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    replay_generation numeric(20,0) NOT NULL,
    opened_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    closing_project_activity_position numeric(20,0),
    CONSTRAINT replay_generations_closing_project_activity_position_check CHECK (((closing_project_activity_position IS NULL) OR ((closing_project_activity_position >= (0)::numeric) AND (closing_project_activity_position <= '18446744073709551615'::numeric)))),
    CONSTRAINT replay_generations_replay_generation_check CHECK (((replay_generation >= (1)::numeric) AND (replay_generation <= '18446744073709551615'::numeric))),
    CONSTRAINT replay_generations_pkey PRIMARY KEY (owner_user_id, project_id, replay_generation),
    CONSTRAINT replay_generations_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

ALTER TABLE storyos.replay_generations ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.replay_generations FORCE ROW LEVEL SECURITY;

CREATE POLICY replay_generations_exact_scope ON storyos.replay_generations USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.replay_generations TO storyos_runtime;

CREATE TABLE storyos.replay_floors (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    replay_generation numeric(20,0) NOT NULL,
    floor_position numeric(20,0) NOT NULL,
    CONSTRAINT replay_floors_floor_position_check CHECK (((floor_position >= (0)::numeric) AND (floor_position <= '18446744073709551615'::numeric))),
    CONSTRAINT replay_floors_pkey PRIMARY KEY (owner_user_id, project_id, replay_generation),
    CONSTRAINT replay_floors_owner_user_id_project_id_replay_generation_fkey FOREIGN KEY (owner_user_id, project_id, replay_generation) REFERENCES storyos.replay_generations(owner_user_id, project_id, replay_generation) MATCH FULL
);

ALTER TABLE storyos.replay_floors ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.replay_floors FORCE ROW LEVEL SECURITY;

CREATE POLICY replay_floors_exact_scope ON storyos.replay_floors USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.replay_floors TO storyos_runtime;

CREATE TABLE storyos.project_snapshots (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    snapshot_id uuid NOT NULL,
    replay_generation numeric(20,0) NOT NULL,
    project_activity_position numeric(20,0) NOT NULL,
    snapshot_kind text NOT NULL,
    redaction_profile text NOT NULL,
    schema_profile text NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    expires_at timestamp with time zone,
    CONSTRAINT project_snapshots_project_activity_position_check CHECK (((project_activity_position >= (0)::numeric) AND (project_activity_position <= '18446744073709551615'::numeric))),
    CONSTRAINT project_snapshots_redaction_profile_check CHECK ((redaction_profile = 'storyos.author.v1'::text)),
    CONSTRAINT project_snapshots_schema_profile_check CHECK ((schema_profile = 'storyos.public.release.1'::text)),
    CONSTRAINT project_snapshots_snapshot_kind_check CHECK ((snapshot_kind = 'canonical'::text)),
    CONSTRAINT project_snapshots_pkey PRIMARY KEY (owner_user_id, project_id, snapshot_id),
    CONSTRAINT project_snapshots_owner_user_id_project_id_replay_generati_fkey FOREIGN KEY (owner_user_id, project_id, replay_generation) REFERENCES storyos.replay_generations(owner_user_id, project_id, replay_generation) MATCH FULL
);

CREATE INDEX project_snapshots_replay_generation_fk_idx ON storyos.project_snapshots USING btree (owner_user_id, project_id, replay_generation);

ALTER TABLE storyos.project_snapshots ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_snapshots FORCE ROW LEVEL SECURITY;

CREATE POLICY project_snapshots_exact_scope ON storyos.project_snapshots USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_snapshots TO storyos_runtime;

-- Project Activity Event payloads

CREATE TABLE storyos.project_activity_event_payloads (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    project_activity_position numeric(20,0) NOT NULL,
    project_activity_event_id uuid NOT NULL,
    event_kind storyos.event_kind NOT NULL,
    receipt_id uuid NOT NULL,
    receipt_result_kind text NOT NULL,
    payload jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT project_activity_event_payloads_project_activity_position_check CHECK (((project_activity_position >= (1)::numeric) AND (project_activity_position <= '18446744073709551615'::numeric))),
    CONSTRAINT project_activity_event_payloads_receipt_result_kind_check CHECK ((receipt_result_kind = ANY (ARRAY['no_effect'::text, 'authoritative_applied'::text]))),
    CONSTRAINT project_activity_event_payloads_shape CHECK ((
        (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'writer_takeover_applied'::text
            AND receipt_result_kind = 'no_effect'::text
            AND (payload ->> 'kind'::text) = 'takeover_applied'::text
            AND jsonb_typeof((payload -> 'prior_editor_session_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'prior_writer_generation'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'resulting_editor_session_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'resulting_writer_generation'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'resulting_snapshot_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'resulting_snapshot_activity_position'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'resulting_heads'::text)) = 'array'::text
            AND ((((((((payload - 'kind'::text) - 'prior_editor_session_id'::text) - 'prior_writer_generation'::text) - 'resulting_editor_session_id'::text) - 'resulting_writer_generation'::text) - 'resulting_snapshot_id'::text) - 'resulting_snapshot_activity_position'::text) - 'resulting_heads'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'writer_takeover_compare_failed'::text
            AND receipt_result_kind = 'no_effect'::text
            AND (payload ->> 'kind'::text) = 'takeover_compare_failed'::text
            AND jsonb_typeof((payload -> 'observed_writer_generation'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'current_writer_generation'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'current_writer_projection'::text)) = 'object'::text
            AND jsonb_typeof((payload -> 'current_snapshot_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'current_snapshot_activity_position'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'current_heads'::text)) = 'array'::text
            AND jsonb_typeof((payload -> 'reason'::text)) = 'string'::text
            AND (payload ->> 'reason'::text) = ANY (ARRAY['writer_generation_advanced_after_admission'::text, 'requester_became_current_after_admission'::text])
            AND ((((((((payload - 'kind'::text) - 'observed_writer_generation'::text) - 'current_writer_generation'::text) - 'current_writer_projection'::text) - 'current_snapshot_id'::text) - 'current_snapshot_activity_position'::text) - 'current_heads'::text) - 'reason'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'project_created'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'project_created'::text
            AND (payload ->> 'open_kind'::text) = 'empty'::text
            AND jsonb_typeof((payload -> 'title'::text)) = 'string'::text
            AND (((payload - 'kind'::text) - 'open_kind'::text) - 'title'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'project_updated'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'project_updated'::text
            AND jsonb_typeof((payload -> 'title'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'revision'::text)) = 'string'::text
            AND (((payload - 'kind'::text) - 'title'::text) - 'revision'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'volume_created'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'volume_created'::text
            AND jsonb_typeof((payload -> 'volume_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'title'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'tree_revision'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'order'::text)) = 'string'::text
            AND (((((payload - 'kind'::text) - 'volume_id'::text) - 'title'::text) - 'tree_revision'::text) - 'order'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'volume_updated'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'volume_updated'::text
            AND jsonb_typeof((payload -> 'volume_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'title'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'tree_revision'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'order'::text)) = 'string'::text
            AND (((((((payload - 'kind'::text) - 'volume_id'::text) - 'title'::text) - 'tree_revision'::text) - 'order'::text) - 'prior_title'::text) - 'prior_order'::text) = '{}'::jsonb
            AND (((NOT (payload ? 'prior_title'::text)) AND (NOT (payload ? 'prior_order'::text))) OR ((payload ? 'prior_title'::text) AND (payload ? 'prior_order'::text) AND (jsonb_typeof((payload -> 'prior_title'::text)) = 'string'::text) AND (jsonb_typeof((payload -> 'prior_order'::text)) = 'string'::text) AND ((payload ->> 'prior_order'::text) ~ '^[1-9][0-9]*$'::text))))
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'chapter_updated'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'chapter_updated'::text
            AND jsonb_typeof((payload -> 'chapter_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'title'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'tree_revision'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'order'::text)) = 'string'::text
            AND (((((((payload - 'kind'::text) - 'chapter_id'::text) - 'title'::text) - 'tree_revision'::text) - 'order'::text) - 'prior_title'::text) - 'prior_order'::text) = '{}'::jsonb
            AND (((NOT (payload ? 'prior_title'::text)) AND (NOT (payload ? 'prior_order'::text))) OR ((payload ? 'prior_title'::text) AND (payload ? 'prior_order'::text) AND (jsonb_typeof((payload -> 'prior_title'::text)) = 'string'::text) AND (jsonb_typeof((payload -> 'prior_order'::text)) = 'string'::text) AND ((payload ->> 'prior_order'::text) ~ '^[1-9][0-9]*$'::text))))
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'chapter_created'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'chapter_created'::text
            AND jsonb_typeof((payload -> 'volume_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'chapter_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'title'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'tree_revision'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'order'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'current_chapter_id'::text)) = 'string'::text
            AND (((((((payload - 'kind'::text) - 'volume_id'::text) - 'chapter_id'::text) - 'title'::text) - 'tree_revision'::text) - 'order'::text) - 'current_chapter_id'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'current_chapter_set'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'current_chapter_set'::text
            AND jsonb_typeof((payload -> 'prior_chapter_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'current_chapter_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'base_snapshot_id'::text)) = 'string'::text
            AND ((((payload - 'kind'::text) - 'prior_chapter_id'::text) - 'current_chapter_id'::text) - 'base_snapshot_id'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'chapter_deleted'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'chapter_deleted'::text
            AND jsonb_typeof((payload -> 'chapter_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'volume_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'tree_revision'::text)) = 'string'::text
            AND payload ? 'current_chapter_id'::text
            AND jsonb_typeof((payload -> 'current_chapter_id'::text)) = ANY (ARRAY['string'::text, 'null'::text])
            AND ((((((payload - 'kind'::text) - 'chapter_id'::text) - 'volume_id'::text) - 'tree_revision'::text) - 'current_chapter_id'::text) - 'prior_current_chapter_id'::text) = '{}'::jsonb
            AND ((NOT (payload ? 'prior_current_chapter_id'::text)) OR (jsonb_typeof((payload -> 'prior_current_chapter_id'::text)) = ANY (ARRAY['string'::text, 'null'::text]))))
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'volume_deleted'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'volume_deleted'::text
            AND jsonb_typeof((payload -> 'volume_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'tree_revision'::text)) = 'string'::text
            AND (((payload - 'kind'::text) - 'volume_id'::text) - 'tree_revision'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'human_readable_manuscript_export_settled'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'human_readable_manuscript_export_settled'::text
            AND jsonb_typeof((payload -> 'export_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'content_sha256'::text)) = 'string'::text
            AND (((payload - 'kind'::text) - 'export_id'::text) - 'content_sha256'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'project_export_settled'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'project_export_settled'::text
            AND jsonb_typeof((payload -> 'export_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'archive_profile'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'archive_path_profile'::text)) = 'string'::text
            AND ((((payload - 'kind'::text) - 'export_id'::text) - 'archive_profile'::text) - 'archive_path_profile'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'project_assistance_updated'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'project_assistance_updated'::text
            AND (payload ->> 'availability'::text) = ANY (ARRAY['available'::text, 'unavailable'::text])
            AND jsonb_typeof((payload -> 'revision'::text)) = 'string'::text
            AND (((payload - 'kind'::text) - 'availability'::text) - 'revision'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'project_tool_enablement_set'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'project_tool_enablement_set'::text
            AND (payload ->> 'enablement'::text) = ANY (ARRAY['enabled'::text, 'disabled'::text])
            AND jsonb_typeof((payload -> 'revision'::text)) = 'string'::text
            AND (((payload - 'kind'::text) - 'enablement'::text) - 'revision'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'agent_run_created'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'agent_run_created'::text
            AND jsonb_typeof((payload -> 'project_agent_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'conversation_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'memory_settings_revision'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'run_id'::text)) = 'string'::text
            AND (((((payload - 'kind'::text) - 'project_agent_id'::text) - 'conversation_id'::text) - 'memory_settings_revision'::text) - 'run_id'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'agent_run_paused'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'agent_run_paused'::text
            AND jsonb_typeof((payload -> 'run_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'fence_generation'::text)) = 'string'::text
            AND (((payload - 'kind'::text) - 'run_id'::text) - 'fence_generation'::text) = '{}'::jsonb)
        OR (jsonb_typeof(payload) = 'object'::text
            AND event_kind = 'agent_run_cancelled'::text
            AND receipt_result_kind = 'authoritative_applied'::text
            AND (payload ->> 'kind'::text) = 'agent_run_cancelled'::text
            AND jsonb_typeof((payload -> 'run_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'fence_generation'::text)) = 'string'::text
            AND (((payload - 'kind'::text) - 'run_id'::text) - 'fence_generation'::text) = '{}'::jsonb)
        OR (event_kind = 'agent_run_steering_retained'::text
            AND receipt_result_kind = 'no_effect'::text
            AND (payload ->> 'kind'::text) = event_kind
            AND jsonb_typeof((payload -> 'run_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'conversation_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'steering_input_id'::text)) = 'string'::text
            AND (payload ->> 'input_position'::text) ~ '^[1-9][0-9]*$'::text
            AND ((payload ->> 'input_position'::text))::numeric <= '18446744073709551615'::numeric
            AND char_length((payload ->> 'author_message'::text)) >= 1
            AND char_length((payload ->> 'author_message'::text)) <= 8000
            AND (payload - ARRAY['kind'::text, 'run_id'::text, 'conversation_id'::text, 'steering_input_id'::text, 'input_position'::text, 'author_message'::text]) = '{}'::jsonb)
        OR (event_kind = 'agent_run_steering_retained'::text
            AND receipt_result_kind = 'no_effect'::text
            AND (payload ->> 'kind'::text) = event_kind
            AND jsonb_typeof((payload -> 'run_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'conversation_id'::text)) = 'string'::text
            AND jsonb_typeof((payload -> 'steering_input_id'::text)) = 'string'::text
            AND (payload ->> 'input_position'::text) ~ '^[1-9][0-9]*$'::text
            AND ((payload ->> 'input_position'::text))::numeric <= '18446744073709551615'::numeric
            AND char_length((payload ->> 'author_message'::text)) >= 1
            AND char_length((payload ->> 'author_message'::text)) <= 8000
            AND jsonb_typeof((payload -> 'selection_references'::text)) = 'array'::text
            AND jsonb_array_length((payload -> 'selection_references'::text)) >= 1
            AND jsonb_array_length((payload -> 'selection_references'::text)) <= 8
            AND (payload - ARRAY['kind'::text, 'run_id'::text, 'conversation_id'::text, 'steering_input_id'::text, 'input_position'::text, 'author_message'::text, 'selection_references'::text]) = '{}'::jsonb)
    ) IS TRUE),
    CONSTRAINT project_activity_event_payloa_owner_user_id_project_id_proj_key UNIQUE (owner_user_id, project_id, project_activity_event_id),
    CONSTRAINT project_activity_event_payloa_owner_user_id_project_id_rece_key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT project_activity_event_payloads_pkey PRIMARY KEY (owner_user_id, project_id, project_activity_position),
    CONSTRAINT project_activity_event_payloa_owner_user_id_project_id_rec_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id, receipt_result_kind) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id, result_kind) MATCH FULL
);

ALTER TABLE storyos.project_activity_event_payloads ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_activity_event_payloads FORCE ROW LEVEL SECURITY;

CREATE POLICY project_activity_event_payloads_exact_scope ON storyos.project_activity_event_payloads USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_activity_event_payloads TO storyos_runtime;

CREATE FUNCTION storyos.require_activity_position_exclusive() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
DECLARE
  scoped_owner_user_id uuid := COALESCE(NEW.owner_user_id, OLD.owner_user_id);
  scoped_project_id uuid := COALESCE(NEW.project_id, OLD.project_id);
  scoped_position numeric(20, 0) := COALESCE(
    NEW.project_activity_position, OLD.project_activity_position
  );
  overlap_count bigint;
BEGIN
  SELECT count(*) INTO overlap_count
    FROM storyos.project_activity_events AS activity
    JOIN storyos.project_activity_event_payloads AS payload
      ON (payload.owner_user_id, payload.project_id, payload.project_activity_position) =
         (activity.owner_user_id, activity.project_id, activity.project_activity_position)
   WHERE activity.owner_user_id = scoped_owner_user_id
     AND activity.project_id = scoped_project_id
     AND activity.project_activity_position = scoped_position;
  IF overlap_count <> 0 THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'Author Edit Activity and takeover Activity cannot share a position';
  END IF;
  RETURN NULL;
END
$$;

CREATE CONSTRAINT TRIGGER project_activity_events_position_exclusive AFTER INSERT OR DELETE OR UPDATE ON storyos.project_activity_events DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_activity_position_exclusive();

CREATE CONSTRAINT TRIGGER project_activity_event_payloads_position_exclusive AFTER INSERT OR DELETE OR UPDATE ON storyos.project_activity_event_payloads DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_activity_position_exclusive();

-- Create Project challenges

CREATE TABLE storyos.create_project_idempotency (
    user_id uuid NOT NULL,
    idempotency_key uuid NOT NULL,
    prospective_project_id uuid NOT NULL,
    create_input_digest text NOT NULL,
    canonical_command_digest text NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    outcome_kind text DEFAULT 'pending'::text NOT NULL,
    result_reference text,
    CONSTRAINT create_project_idempotency_outcome_kind_check CHECK ((outcome_kind = ANY (ARRAY['pending'::text, 'in_progress'::text, 'settled'::text]))),
    CONSTRAINT create_project_idempotency_pkey PRIMARY KEY (user_id, idempotency_key),
    CONSTRAINT create_project_idempotency_prospective_project_id_key UNIQUE (prospective_project_id),
    CONSTRAINT create_project_idempotency_user_id_fkey FOREIGN KEY (user_id) REFERENCES storyos.users(user_id)
);

ALTER TABLE storyos.create_project_idempotency ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.create_project_idempotency FORCE ROW LEVEL SECURITY;

CREATE POLICY create_project_idempotency_exact_user ON storyos.create_project_idempotency USING ((user_id = (NULLIF(current_setting('storyos.user_id'::text, true), ''::text))::uuid)) WITH CHECK ((user_id = (NULLIF(current_setting('storyos.user_id'::text, true), ''::text))::uuid));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.create_project_idempotency TO storyos_runtime;

CREATE TABLE storyos.create_project_challenges (
    user_id uuid NOT NULL,
    idempotency_key uuid NOT NULL,
    prospective_project_id uuid NOT NULL,
    client_session_binding_digest text NOT NULL,
    client_session_generation numeric(20,0) NOT NULL,
    client_contract_revision text NOT NULL,
    security_policy_revision text NOT NULL,
    limit_profile_revision text NOT NULL,
    method text NOT NULL,
    route_template text NOT NULL,
    command_schema text NOT NULL,
    command_kind text NOT NULL,
    canonical_command_digest text NOT NULL,
    nonce_digest text NOT NULL,
    issued_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    expires_at timestamp with time zone DEFAULT (clock_timestamp() + '00:05:00'::interval) NOT NULL,
    consumed_at timestamp with time zone,
    CONSTRAINT create_project_challenges_client_session_generation_check CHECK (((client_session_generation >= (0)::numeric) AND (client_session_generation <= '18446744073709551615'::numeric))),
    CONSTRAINT create_project_challenges_command_kind_check CHECK ((command_kind = 'createProject'::text)),
    CONSTRAINT create_project_challenges_nonce_digest_key UNIQUE (nonce_digest),
    CONSTRAINT create_project_challenges_pkey PRIMARY KEY (user_id, idempotency_key),
    CONSTRAINT create_project_challenges_user_id_idempotency_key_fkey FOREIGN KEY (user_id, idempotency_key) REFERENCES storyos.create_project_idempotency(user_id, idempotency_key)
);

ALTER TABLE storyos.create_project_challenges ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.create_project_challenges FORCE ROW LEVEL SECURITY;

CREATE POLICY create_project_challenges_exact_user ON storyos.create_project_challenges USING ((user_id = (NULLIF(current_setting('storyos.user_id'::text, true), ''::text))::uuid)) WITH CHECK ((user_id = (NULLIF(current_setting('storyos.user_id'::text, true), ''::text))::uuid));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.create_project_challenges TO storyos_runtime;

CREATE FUNCTION storyos.require_create_project_challenge_admission() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
BEGIN
  IF NEW.command_kind <> 'createProject' THEN
    RETURN NEW;
  END IF;
  IF NOT EXISTS (
    SELECT 1
      FROM storyos.create_project_challenges AS challenge
     WHERE challenge.user_id = NEW.owner_user_id
       AND challenge.idempotency_key = NEW.idempotency_key
       AND challenge.prospective_project_id = NEW.project_id
       AND challenge.canonical_command_digest = NEW.canonical_command_digest
       AND challenge.consumed_at IS NOT NULL
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'createProject Admission requires a consumed User-level challenge';
  END IF;
  RETURN NEW;
END
$$;

CREATE TRIGGER author_command_admissions_create_project_challenge BEFORE INSERT ON storyos.author_command_admissions FOR EACH ROW EXECUTE FUNCTION storyos.require_create_project_challenge_admission();

-- Receipt relations of the Author Command kinds

CREATE FUNCTION storyos.require_author_edit_receipt_relation() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
DECLARE
  scoped_owner_user_id uuid := COALESCE(NEW.owner_user_id, OLD.owner_user_id);
  scoped_project_id uuid := COALESCE(NEW.project_id, OLD.project_id);
  scoped_receipt_id uuid := COALESCE(NEW.receipt_id, OLD.receipt_id);
  scoped_result_kind text;
  scoped_command_kind text;
  scoped_reason text;
  scoped_producer_cause text;
  activity_count bigint;
  payload_count bigint;
  action_count bigint;
  commit_count bigint;
  revision_envelope_count bigint;
  payload_event_kind text;
BEGIN
  SELECT receipt.result_kind, receipt.command_kind, receipt.result_payload->>'reason',
         receipt.producer_cause
    INTO scoped_result_kind, scoped_command_kind, scoped_reason, scoped_producer_cause
    FROM storyos.domain_receipts AS receipt
   WHERE receipt.owner_user_id = scoped_owner_user_id
     AND receipt.project_id = scoped_project_id
     AND receipt.receipt_id = scoped_receipt_id;
  IF NOT FOUND THEN
    RETURN NULL;
  END IF;

  SELECT count(*) INTO activity_count
    FROM storyos.project_activity_events AS activity
   WHERE activity.owner_user_id = scoped_owner_user_id
     AND activity.project_id = scoped_project_id
     AND activity.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO payload_count
    FROM storyos.project_activity_event_payloads AS payload
   WHERE payload.owner_user_id = scoped_owner_user_id
     AND payload.project_id = scoped_project_id
     AND payload.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO action_count
    FROM storyos.author_action_entries AS action
   WHERE action.owner_user_id = scoped_owner_user_id
     AND action.project_id = scoped_project_id
     AND action.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO commit_count
    FROM storyos.authoritative_commits AS authoritative_commit
   WHERE authoritative_commit.owner_user_id = scoped_owner_user_id
     AND authoritative_commit.project_id = scoped_project_id
     AND authoritative_commit.receipt_id = scoped_receipt_id;
  SELECT count(*) INTO revision_envelope_count
    FROM storyos.authoritative_revision_envelopes AS envelope
   WHERE envelope.owner_user_id = scoped_owner_user_id
     AND envelope.project_id = scoped_project_id
     AND envelope.receipt_id = scoped_receipt_id;

  IF scoped_command_kind = 'takeOverProjectWriter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind <> 'no_effect'
       OR (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
          <> (0, 0, 0, 0, 1)
       OR payload_event_kind IS DISTINCT FROM scoped_reason THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'takeOverProjectWriter requires one takeover Activity and zero manuscript authority';
    END IF;
  ELSIF scoped_command_kind = 'createProject' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind <> 'authoritative_applied'
       OR (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
          <> (0, 0, 0, 0, 1)
       OR payload_event_kind IS DISTINCT FROM 'project_created' THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createProject requires one project-created Activity and zero manuscript authority';
    END IF;
  ELSIF scoped_command_kind = 'updateProject' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count)
           <> (0, 0, 0, 0, 1)
         OR payload_event_kind IS DISTINCT FROM 'project_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateProject applied requires one project-updated Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateProject with zero title effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'createVolume' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'volume_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createVolume applied requires one volume-created Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createVolume with zero tree effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'updateVolume' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'volume_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateVolume applied requires one volume-updated Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateVolume with zero tree effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'updateChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'chapter_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateChapter applied requires one chapter-updated Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateChapter with zero tree effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'setCurrentChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR commit_count <> 0
         OR revision_envelope_count <> 0
         OR action_count NOT IN (0, 1)
         OR payload_event_kind IS DISTINCT FROM 'current_chapter_set' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'setCurrentChapter applied requires one current-chapter-set Activity, no Commit, and at most one Author Action';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'setCurrentChapter with zero current-chapter effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'createChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'chapter_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createChapter applied requires one chapter-created Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createChapter with zero tree effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'deleteChapter' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'chapter_deleted' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'deleteChapter applied requires one chapter-deleted Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'deleteChapter with zero tree effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'deleteVolume' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF activity_count <> 0
         OR payload_count <> 1
         OR NOT (
           (action_count, commit_count, revision_envelope_count) = (0, 0, 0)
           OR (action_count = 1
             AND commit_count = 1
             AND revision_envelope_count IN (0, 1))
         )
         OR payload_event_kind IS DISTINCT FROM 'volume_deleted' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'deleteVolume applied requires one volume-deleted Activity and at most one structure Commit';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'deleteVolume with zero tree effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'exportHumanReadableManuscript' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count)
           <> (0, 0, 0, 0, 1)
         OR payload_event_kind IS DISTINCT FROM 'human_readable_manuscript_export_settled' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'exportHumanReadableManuscript applied requires one export-settled Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'exportHumanReadableManuscript with zero export effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'exportProjectArchive' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count)
           <> (0, 0, 0, 0, 1)
         OR payload_event_kind IS DISTINCT FROM 'project_export_settled' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'exportProjectArchive applied requires one export-settled Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'exportProjectArchive with zero export effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'undoLatestAuthorAction' THEN
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF NOT (
        (activity_count, action_count, commit_count, revision_envelope_count,
         payload_count) = (1, 1, 1, 1, 0)
        OR (activity_count, action_count, commit_count, revision_envelope_count,
            payload_count) = (0, 1, 1, 0, 0)
        OR (activity_count, action_count, commit_count, revision_envelope_count,
            payload_count) = (0, 1, 0, 0, 0)
      ) THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'undoLatestAuthorAction applied requires prose, structure, or Current Chapter compensation authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'undoLatestAuthorAction with zero authority cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'updateProjectAssistance' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count)
           <> (0, 0, 0, 0, 1)
         OR payload_event_kind IS DISTINCT FROM 'project_assistance_updated' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'updateProjectAssistance applied requires one assistance Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'updateProjectAssistance with zero availability effect cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'setProjectToolEnablement' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF (activity_count, action_count, commit_count, revision_envelope_count, payload_count)
         <> (0, 0, 0, 0, CASE WHEN scoped_result_kind = 'authoritative_applied' THEN 1 ELSE 0 END)
       OR (scoped_result_kind = 'authoritative_applied'
           AND payload_event_kind IS DISTINCT FROM 'project_tool_enablement_set') THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'setProjectToolEnablement requires one enablement Activity only when applied';
    END IF;

  ELSIF scoped_command_kind = 'createAgentRun' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count)
           <> (0, 0, 0, 0, 1)
         OR payload_event_kind IS DISTINCT FROM 'agent_run_created' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'createAgentRun applied requires one AgentRun Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'createAgentRun with zero admission effect cannot have an authority or Activity relation';
    END IF;


  ELSIF scoped_command_kind = 'steerAgentRun' THEN
    SELECT payload.event_kind INTO payload_event_kind FROM storyos.project_activity_event_payloads AS payload
      WHERE payload.owner_user_id=scoped_owner_user_id AND payload.project_id=scoped_project_id AND payload.receipt_id=scoped_receipt_id;
    IF (activity_count,action_count,commit_count,revision_envelope_count) <> (0,0,0,0)
      OR (scoped_result_kind='no_effect' AND (payload_count<>1 OR payload_event_kind IS DISTINCT FROM 'agent_run_steering_retained'))
      OR (scoped_result_kind='conflicted' AND payload_count<>0) THEN
      RAISE EXCEPTION 'Steering retention requires exact Activity and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'pauseAgentRun' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count)
           <> (0, 0, 0, 0, 1)
         OR payload_event_kind IS DISTINCT FROM 'agent_run_paused' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'pauseAgentRun applied requires one paused Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'pauseAgentRun with zero transition cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'cancelAgentRun' THEN
    SELECT payload.event_kind
      INTO payload_event_kind
      FROM storyos.project_activity_event_payloads AS payload
     WHERE payload.owner_user_id = scoped_owner_user_id
       AND payload.project_id = scoped_project_id
       AND payload.receipt_id = scoped_receipt_id;
    IF scoped_result_kind = 'authoritative_applied' THEN
      IF (activity_count, action_count, commit_count, revision_envelope_count,
          payload_count)
           <> (0, 0, 0, 0, 1)
         OR payload_event_kind IS DISTINCT FROM 'agent_run_cancelled' THEN
        RAISE EXCEPTION USING
          ERRCODE = '23514',
          MESSAGE = 'cancelAgentRun applied requires one cancelled Activity and zero manuscript authority';
      END IF;
    ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
           payload_count)
            <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'cancelAgentRun with zero transition cannot have an authority or Activity relation';
    END IF;

  ELSIF scoped_command_kind = 'rejectProposalOperations'
        AND scoped_result_kind = 'proposal_operations_resolved' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 1, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'rejectProposalOperations resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'rejectProposalOperations'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'rejectProposalOperations with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'reopenRejectedOperations'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 1, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenRejectedOperations resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'reopenRejectedOperations'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenRejectedOperations with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'replanProposal'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 1, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'replanProposal resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'replanProposal'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'replanProposal with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'withdrawProposal'
        AND scoped_result_kind = 'proposal_closure_changed'
        AND scoped_producer_cause = 'author_command_admission' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 1, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'withdrawProposal resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'withdrawProposal'
        AND scoped_result_kind = 'proposal_closure_changed'
        AND scoped_producer_cause = 'agent_run_decision' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'withdrawProposal current-producer resolved requires zero Author Actions and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'withdrawProposal'
        AND scoped_result_kind IN ('conflicted', 'refused', 'no_effect') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'withdrawProposal with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'reopenWithdrawnProposal'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 1, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenWithdrawnProposal resolved requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'reopenWithdrawnProposal'
        AND scoped_result_kind IN ('conflicted', 'refused', 'no_effect') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'reopenWithdrawnProposal with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'completeReadyPartialProposal'
        AND scoped_result_kind = 'proposal_generation_completed' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 1, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'completeReadyPartialProposal completed requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'completeReadyPartialProposal'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'completeReadyPartialProposal with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'continueProposalGeneration'
        AND scoped_result_kind = 'proposal_generation_started' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 1, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'continueProposalGeneration started requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_command_kind = 'continueProposalGeneration'
        AND scoped_result_kind IN ('conflicted', 'refused') THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 0, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'continueProposalGeneration with zero effect cannot have an authority or Activity relation';
    END IF;
  ELSIF scoped_command_kind = 'applyAuthorEdit'
        AND scoped_result_kind = 'proposal_revised' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (0, 1, 0, 0, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'applyAuthorEdit ProposalRevised requires one Author Action and zero authority';
    END IF;
  ELSIF scoped_result_kind = 'authoritative_applied' THEN
    IF (activity_count, action_count, commit_count, revision_envelope_count,
        payload_count)
         <> (1, 1, 1, 1, 0) THEN
      RAISE EXCEPTION USING
        ERRCODE = '23514',
        MESSAGE = 'AuthoritativeApplied requires one complete authority and Activity relation';
    END IF;
  ELSIF (activity_count, action_count, commit_count, revision_envelope_count,
         payload_count)
          <> (0, 0, 0, 0, 0) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'A zero-authority Receipt cannot have an authority or Activity relation';
  END IF;
  RETURN NULL;
END
$$;

CREATE CONSTRAINT TRIGGER domain_receipts_author_edit_relation_complete AFTER INSERT OR UPDATE ON storyos.domain_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_author_edit_receipt_relation();

CREATE CONSTRAINT TRIGGER project_activity_author_edit_relation_complete AFTER INSERT OR DELETE OR UPDATE ON storyos.project_activity_events DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_author_edit_receipt_relation();

CREATE CONSTRAINT TRIGGER project_activity_event_payloads_receipt_relation_complete AFTER INSERT OR DELETE OR UPDATE ON storyos.project_activity_event_payloads DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_author_edit_receipt_relation();

-- Manuscript Blocks

CREATE TABLE storyos.manuscript_blocks (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    manuscript_block_id uuid NOT NULL,
    manuscript_object_id uuid NOT NULL,
    block_kind text NOT NULL,
    CONSTRAINT manuscript_blocks_block_kind_check CHECK ((block_kind = 'paragraph'::text)),
    CONSTRAINT manuscript_blocks_pkey PRIMARY KEY (owner_user_id, project_id, manuscript_block_id),
    CONSTRAINT manuscript_blocks_owner_user_id_project_id_manuscript_obje_fkey FOREIGN KEY (owner_user_id, project_id, manuscript_object_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id) MATCH FULL
);

CREATE INDEX manuscript_blocks_chapter_fk_idx ON storyos.manuscript_blocks USING btree (owner_user_id, project_id, manuscript_object_id);

ALTER TABLE storyos.manuscript_blocks ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.manuscript_blocks FORCE ROW LEVEL SECURITY;

CREATE POLICY manuscript_blocks_exact_scope ON storyos.manuscript_blocks USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.manuscript_blocks TO storyos_runtime;

CREATE TABLE storyos.manuscript_revision_members (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    manuscript_object_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    manuscript_block_id uuid NOT NULL,
    block_order numeric(20,0) NOT NULL,
    CONSTRAINT manuscript_revision_members_block_order_check CHECK (((block_order >= (1)::numeric) AND (block_order <= '18446744073709551615'::numeric))),
    CONSTRAINT manuscript_revision_members_owner_user_id_project_id_manusc_key UNIQUE (owner_user_id, project_id, manuscript_object_id, revision_id, block_order),
    CONSTRAINT manuscript_revision_members_pkey PRIMARY KEY (owner_user_id, project_id, manuscript_object_id, revision_id, manuscript_block_id),
    CONSTRAINT manuscript_revision_members_owner_user_id_project_id_manu_fkey1 FOREIGN KEY (owner_user_id, project_id, manuscript_block_id) REFERENCES storyos.manuscript_blocks(owner_user_id, project_id, manuscript_block_id) MATCH FULL,
    CONSTRAINT manuscript_revision_members_owner_user_id_project_id_manus_fkey FOREIGN KEY (owner_user_id, project_id, manuscript_object_id, revision_id) REFERENCES storyos.authoritative_revisions(owner_user_id, project_id, manuscript_object_id, revision_id) MATCH FULL
);

CREATE INDEX manuscript_revision_members_block_fk_idx ON storyos.manuscript_revision_members USING btree (owner_user_id, project_id, manuscript_block_id);

ALTER TABLE storyos.manuscript_revision_members ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.manuscript_revision_members FORCE ROW LEVEL SECURITY;

CREATE POLICY manuscript_revision_members_exact_scope ON storyos.manuscript_revision_members USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.manuscript_revision_members TO storyos_runtime;

-- Chapter removal

CREATE TABLE storyos.chapter_removal_decisions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    chapter_removal_decision_id uuid NOT NULL,
    receipt_id uuid NOT NULL,
    chapter_id uuid NOT NULL,
    volume_id uuid NOT NULL,
    tree_revision bigint NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT chapter_removal_decisions_tree_revision_check CHECK ((tree_revision >= 1)),
    CONSTRAINT chapter_removal_decisions_owner_user_id_project_id_chapter__key UNIQUE (owner_user_id, project_id, chapter_id),
    CONSTRAINT chapter_removal_decisions_owner_user_id_project_id_receipt__key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT chapter_removal_decisions_pkey PRIMARY KEY (owner_user_id, project_id, chapter_removal_decision_id),
    CONSTRAINT chapter_removal_decisions_owner_user_id_project_id_chapter_fkey FOREIGN KEY (owner_user_id, project_id, chapter_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id) MATCH FULL,
    CONSTRAINT chapter_removal_decisions_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT chapter_removal_decisions_owner_user_id_project_id_receipt_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);

ALTER TABLE storyos.chapter_removal_decisions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.chapter_removal_decisions FORCE ROW LEVEL SECURITY;

CREATE POLICY chapter_removal_decisions_exact_scope ON storyos.chapter_removal_decisions USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,DELETE ON TABLE storyos.chapter_removal_decisions TO storyos_runtime;

-- Volume removal

CREATE TABLE storyos.volume_removal_decisions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    volume_removal_decision_id uuid NOT NULL,
    receipt_id uuid NOT NULL,
    volume_id uuid NOT NULL,
    tree_revision bigint NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT volume_removal_decisions_tree_revision_check CHECK ((tree_revision >= 1)),
    CONSTRAINT volume_removal_decisions_owner_user_id_project_id_receipt_i_key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT volume_removal_decisions_owner_user_id_project_id_volume_id_key UNIQUE (owner_user_id, project_id, volume_id),
    CONSTRAINT volume_removal_decisions_pkey PRIMARY KEY (owner_user_id, project_id, volume_removal_decision_id),
    CONSTRAINT volume_removal_decisions_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT volume_removal_decisions_owner_user_id_project_id_receipt__fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
    CONSTRAINT volume_removal_decisions_owner_user_id_project_id_volume_i_fkey FOREIGN KEY (owner_user_id, project_id, volume_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id) MATCH FULL
);

ALTER TABLE storyos.volume_removal_decisions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.volume_removal_decisions FORCE ROW LEVEL SECURITY;

CREATE POLICY volume_removal_decisions_exact_scope ON storyos.volume_removal_decisions USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,DELETE ON TABLE storyos.volume_removal_decisions TO storyos_runtime;

-- Human-readable manuscript exports

CREATE TABLE storyos.human_readable_manuscript_exports (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    export_id uuid NOT NULL,
    receipt_id uuid NOT NULL,
    source_snapshot_id text NOT NULL,
    source_activity_position bigint NOT NULL,
    export_profile text NOT NULL,
    manuscript_utf8 text NOT NULL,
    content_sha256 text NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT human_readable_manuscript_export_source_activity_position_check CHECK ((source_activity_position >= 0)),
    CONSTRAINT human_readable_manuscript_exports_content_sha256_check CHECK ((content_sha256 ~ '^[0-9a-f]{64}$'::text)),
    CONSTRAINT human_readable_manuscript_exports_export_profile_check CHECK ((export_profile = 'storyos.readable-export.utf8-lf.v1'::text)),
    CONSTRAINT human_readable_manuscript_exp_owner_user_id_project_id_rece_key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT human_readable_manuscript_exports_pkey PRIMARY KEY (owner_user_id, project_id, export_id),
    CONSTRAINT human_readable_manuscript_exp_owner_user_id_project_id_rec_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
    CONSTRAINT human_readable_manuscript_exports_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

ALTER TABLE storyos.human_readable_manuscript_exports ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.human_readable_manuscript_exports FORCE ROW LEVEL SECURITY;

CREATE POLICY human_readable_manuscript_exports_exact_scope ON storyos.human_readable_manuscript_exports USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.human_readable_manuscript_exports TO storyos_runtime;

-- Project export manifests

CREATE TABLE storyos.project_export_manifests (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    export_id uuid NOT NULL,
    receipt_id uuid NOT NULL,
    source_snapshot_id text NOT NULL,
    source_activity_position bigint NOT NULL,
    archive_profile text NOT NULL,
    archive_path_profile text NOT NULL,
    immutable_root text,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT project_export_manifests_archive_path_profile_check CHECK ((archive_path_profile = 'storyos.archive-path.utf8-nfc-unicode-16.0.0.v1'::text)),
    CONSTRAINT project_export_manifests_archive_profile_check CHECK ((archive_profile = 'storyos.project-export.v1'::text)),
    CONSTRAINT project_export_manifests_immutable_root_check CHECK (((immutable_root IS NULL) OR (immutable_root ~ '^sha256:[0-9a-f]{64}$'::text))),
    CONSTRAINT project_export_manifests_source_activity_position_check CHECK ((source_activity_position >= 0)),
    CONSTRAINT project_export_manifests_owner_user_id_project_id_receipt_i_key UNIQUE (owner_user_id, project_id, receipt_id),
    CONSTRAINT project_export_manifests_pkey PRIMARY KEY (owner_user_id, project_id, export_id),
    CONSTRAINT project_export_manifests_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT project_export_manifests_owner_user_id_project_id_receipt__fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);

ALTER TABLE storyos.project_export_manifests ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_export_manifests FORCE ROW LEVEL SECURITY;

CREATE POLICY project_export_manifests_exact_scope ON storyos.project_export_manifests USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.project_export_manifests TO storyos_runtime;

-- Project export entries

CREATE TABLE storyos.project_export_entries (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    export_id uuid NOT NULL,
    path text NOT NULL,
    media_type text NOT NULL,
    payload_schema text NOT NULL,
    byte_length bigint NOT NULL,
    digest text NOT NULL,
    payload bytea NOT NULL,
    CONSTRAINT project_export_entries_byte_length_check CHECK ((byte_length >= 0)),
    CONSTRAINT project_export_entries_check CHECK ((octet_length(payload) = byte_length)),
    CONSTRAINT project_export_entries_digest_check CHECK ((digest ~ '^sha256:[0-9a-f]{64}$'::text)),
    CONSTRAINT project_export_entries_path_check CHECK (octet_length(path) BETWEEN 1 AND 512 AND path !~ '[\\]' AND path !~ '//'),
    CONSTRAINT project_export_entries_pkey PRIMARY KEY (owner_user_id, project_id, export_id, path),
    CONSTRAINT project_export_entries_owner_user_id_project_id_export_id_fkey FOREIGN KEY (owner_user_id, project_id, export_id) REFERENCES storyos.project_export_manifests(owner_user_id, project_id, export_id) MATCH FULL
);

ALTER TABLE storyos.project_export_entries ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_export_entries FORCE ROW LEVEL SECURITY;

CREATE POLICY project_export_entries_exact_scope ON storyos.project_export_entries USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_export_entries TO storyos_runtime;

-- Recovery copies and recovery visibility proofs

CREATE TABLE storyos.recovery_copies (
    recovery_copy_id uuid NOT NULL,
    method text NOT NULL,
    chain_sha256 text NOT NULL,
    required_wal_member text NOT NULL,
    recovery_target_lsn text NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT recovery_copies_chain_sha256_check CHECK ((chain_sha256 ~ '^[0-9a-f]{64}$'::text)),
    CONSTRAINT recovery_copies_method_check CHECK ((method = 'pg_basebackup'::text)),
    CONSTRAINT recovery_copies_recovery_target_lsn_check CHECK (((octet_length(recovery_target_lsn) >= 1) AND (octet_length(recovery_target_lsn) <= 32))),
    CONSTRAINT recovery_copies_required_wal_member_check CHECK (((octet_length(required_wal_member) >= 1) AND (octet_length(required_wal_member) <= 128))),
    CONSTRAINT recovery_copies_pkey PRIMARY KEY (recovery_copy_id)
);

GRANT SELECT,INSERT ON TABLE storyos.recovery_copies TO storyos_restore;
GRANT SELECT ON TABLE storyos.recovery_copies TO storyos_backup;

CREATE TABLE storyos.backup_wal_evidence (
    recovery_copy_id uuid NOT NULL,
    path text NOT NULL,
    sha256 text NOT NULL,
    CONSTRAINT backup_wal_evidence_path_check CHECK (((octet_length(path) >= 1) AND (octet_length(path) <= 512))),
    CONSTRAINT backup_wal_evidence_sha256_check CHECK ((sha256 ~ '^[0-9a-f]{64}$'::text)),
    CONSTRAINT backup_wal_evidence_pkey PRIMARY KEY (recovery_copy_id, path),
    CONSTRAINT backup_wal_evidence_recovery_copy_id_fkey FOREIGN KEY (recovery_copy_id) REFERENCES storyos.recovery_copies(recovery_copy_id)
);

GRANT SELECT,INSERT ON TABLE storyos.backup_wal_evidence TO storyos_restore;
GRANT SELECT ON TABLE storyos.backup_wal_evidence TO storyos_backup;

CREATE TABLE storyos.restore_proofs (
    restore_proof_id uuid NOT NULL,
    recovery_copy_id uuid NOT NULL,
    isolated_target_identity text NOT NULL,
    recovery_target_lsn text NOT NULL,
    state text NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT restore_proofs_isolated_target_identity_check CHECK (((octet_length(isolated_target_identity) >= 1) AND (octet_length(isolated_target_identity) <= 256))),
    CONSTRAINT restore_proofs_recovery_target_lsn_check CHECK (((octet_length(recovery_target_lsn) >= 1) AND (octet_length(recovery_target_lsn) <= 32))),
    CONSTRAINT restore_proofs_state_check CHECK ((state = ANY (ARRAY['recovery_hold'::text, 'visible'::text]))),
    CONSTRAINT restore_proofs_pkey PRIMARY KEY (restore_proof_id),
    CONSTRAINT restore_proofs_recovery_copy_id_fkey FOREIGN KEY (recovery_copy_id) REFERENCES storyos.recovery_copies(recovery_copy_id)
);

GRANT SELECT,INSERT ON TABLE storyos.restore_proofs TO storyos_restore;

CREATE TABLE storyos.recovery_visibility_proofs (
    visibility_proof_id uuid NOT NULL,
    restore_proof_id uuid NOT NULL,
    recovery_copy_id uuid NOT NULL,
    isolated_target_identity text NOT NULL,
    catalog_identity text NOT NULL,
    checks jsonb NOT NULL,
    search_rebuild jsonb NOT NULL,
    statistics_rebuild jsonb NOT NULL,
    lifecycle_range jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT recovery_visibility_proofs_catalog_identity_check CHECK ((catalog_identity = 'storyos.persistence.release-1'::text)),
    CONSTRAINT recovery_visibility_proofs_isolated_target_identity_check CHECK (((octet_length(isolated_target_identity) >= 1) AND (octet_length(isolated_target_identity) <= 256))),
    CONSTRAINT recovery_visibility_proofs_pkey PRIMARY KEY (visibility_proof_id),
    CONSTRAINT recovery_visibility_proofs_restore_proof_id_key UNIQUE (restore_proof_id),
    CONSTRAINT recovery_visibility_proofs_recovery_copy_id_fkey FOREIGN KEY (recovery_copy_id) REFERENCES storyos.recovery_copies(recovery_copy_id),
    CONSTRAINT recovery_visibility_proofs_restore_proof_id_fkey FOREIGN KEY (restore_proof_id) REFERENCES storyos.restore_proofs(restore_proof_id)
);

GRANT SELECT,INSERT ON TABLE storyos.recovery_visibility_proofs TO storyos_restore;

-- The superuser owns this SECURITY DEFINER function.
RESET ROLE;

CREATE FUNCTION storyos.pass_recovery_visibility_proof(p_recovery_copy_id uuid, p_restore_proof_id uuid, p_visibility_proof_id uuid, p_isolated_target_identity text, p_chain_sha256 text, p_required_wal_member text, p_recovery_target_lsn text, p_method text, p_catalog_identity text) RETURNS text
    LANGUAGE plpgsql SECURITY DEFINER
    SET search_path TO 'pg_catalog', 'storyos'
    AS $_$
DECLARE
  runtime_login boolean;
  restore_state text;
  missing_rls text;
  head_gaps bigint;
  chapter_gaps bigint;
  replay_gaps bigint;
  inactive_projects bigint;
  live_search jsonb;
  live_stats jsonb;
BEGIN
  IF p_method IS DISTINCT FROM 'pg_basebackup'
     OR p_catalog_identity IS DISTINCT FROM 'storyos.persistence.release-1'
     OR p_chain_sha256 !~ '^[0-9a-f]{64}$'
     OR octet_length(p_isolated_target_identity) = 0 THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: recovery input';
  END IF;

  SELECT rolcanlogin INTO runtime_login
    FROM pg_roles WHERE rolname = 'storyos_runtime';
  IF runtime_login THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: already visible';
  END IF;
  IF NOT EXISTS (
       SELECT 1 FROM pg_roles
        WHERE rolname = 'storyos_runtime'
          AND NOT rolsuper AND NOT rolbypassrls AND NOT rolcanlogin
     )
     OR NOT EXISTS (
       SELECT 1 FROM pg_roles
        WHERE rolname = 'storyos_restore'
          AND NOT rolsuper AND NOT rolbypassrls AND rolcanlogin
     )
     OR NOT EXISTS (
       SELECT 1 FROM pg_roles
        WHERE rolname = 'storyos_backup'
          AND NOT rolsuper AND NOT rolbypassrls AND rolreplication AND rolcanlogin
     ) THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: roles';
  END IF;

  IF NOT EXISTS (
       SELECT 1 FROM storyos.recovery_copies
        WHERE recovery_copy_id = p_recovery_copy_id
          AND method = p_method
          AND chain_sha256 = p_chain_sha256
          AND required_wal_member = p_required_wal_member
          AND recovery_target_lsn = p_recovery_target_lsn
     )
     OR NOT EXISTS (
       SELECT 1 FROM storyos.backup_wal_evidence
        WHERE recovery_copy_id = p_recovery_copy_id
     ) THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: recovery copy';
  END IF;

  SELECT state INTO restore_state
    FROM storyos.restore_proofs
   WHERE restore_proof_id = p_restore_proof_id
     AND recovery_copy_id = p_recovery_copy_id
     AND isolated_target_identity = p_isolated_target_identity
     AND recovery_target_lsn = p_recovery_target_lsn;
  IF restore_state IS DISTINCT FROM 'recovery_hold' THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: restore proof';
  END IF;
  IF EXISTS (
       SELECT 1 FROM storyos.recovery_visibility_proofs
        WHERE restore_proof_id = p_restore_proof_id
           OR visibility_proof_id = p_visibility_proof_id
     ) THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: duplicate proof';
  END IF;

  SELECT string_agg(c.relname, ',' ORDER BY c.relname) INTO missing_rls
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    JOIN pg_attribute a1
      ON a1.attrelid = c.oid AND a1.attname = 'owner_user_id' AND NOT a1.attisdropped
    JOIN pg_attribute a2
      ON a2.attrelid = c.oid AND a2.attname = 'project_id' AND NOT a2.attisdropped
   WHERE n.nspname = 'storyos' AND c.relkind = 'r'
     AND NOT (c.relrowsecurity AND c.relforcerowsecurity);
  IF missing_rls IS NOT NULL THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: forced rls';
  END IF;

  SELECT count(*) INTO head_gaps
    FROM storyos.authoritative_heads AS head
    LEFT JOIN storyos.authoritative_revisions AS revision
      ON (revision.owner_user_id, revision.project_id,
          revision.manuscript_object_id, revision.revision_id) =
         (head.owner_user_id, head.project_id,
          head.manuscript_object_id, head.current_revision_id)
   WHERE revision.revision_id IS NULL;
  -- A lawful empty Project stores current_chapter_id as SQL NULL. That NULL is not a Chapter gap.
  SELECT count(*) INTO chapter_gaps
    FROM storyos.projects AS project
    LEFT JOIN storyos.manuscript_objects AS chapter
      ON (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id) =
         (project.owner_user_id, project.project_id, project.current_chapter_id)
     AND chapter.object_kind = 'chapter'
   WHERE project.current_chapter_id IS NOT NULL
     AND chapter.manuscript_object_id IS NULL;
  SELECT count(*) INTO replay_gaps
    FROM storyos.projects AS project
    LEFT JOIN storyos.replay_generations AS generation
      ON (generation.owner_user_id, generation.project_id) =
         (project.owner_user_id, project.project_id)
    LEFT JOIN storyos.replay_floors AS floor
      ON (floor.owner_user_id, floor.project_id, floor.replay_generation) =
         (generation.owner_user_id, generation.project_id, generation.replay_generation)
   WHERE generation.replay_generation IS NULL OR floor.floor_position IS NULL;
  IF head_gaps <> 0 OR chapter_gaps <> 0 OR replay_gaps <> 0 THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: canonical heads';
  END IF;

  SELECT count(*) INTO inactive_projects
    FROM storyos.projects AS project
   WHERE project.lifecycle_state IS DISTINCT FROM 'active';
  IF inactive_projects <> 0 THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: lifecycle range';
  END IF;

  SELECT coalesce(
           jsonb_agg(to_jsonb(live) ORDER BY live.project_id, live.chapter_id),
           '[]'::jsonb
         )
    INTO live_search
    FROM (
      SELECT project.project_id::text AS project_id,
             chapter.manuscript_object_id::text AS chapter_id,
             convert_from(payload.canonical_bytes, 'UTF8') AS payload_text,
             octet_length(payload.canonical_bytes) AS byte_length
        FROM storyos.projects AS project
        JOIN storyos.manuscript_objects AS chapter
          ON (chapter.owner_user_id, chapter.project_id) =
             (project.owner_user_id, project.project_id)
         AND chapter.object_kind = 'chapter'
        JOIN storyos.manuscript_objects AS volume
          ON (volume.owner_user_id, volume.project_id, volume.manuscript_object_id) =
             (chapter.owner_user_id, chapter.project_id, chapter.parent_volume_id)
         AND volume.object_kind = 'volume'
        JOIN storyos.authoritative_heads AS head
          ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
             (chapter.owner_user_id, chapter.project_id, chapter.manuscript_object_id)
        JOIN storyos.authoritative_revisions AS revision
          ON (revision.owner_user_id, revision.project_id,
              revision.manuscript_object_id, revision.revision_id) =
             (head.owner_user_id, head.project_id,
              head.manuscript_object_id, head.current_revision_id)
        JOIN storyos.authoritative_payloads AS payload
          ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
             (revision.owner_user_id, revision.project_id, revision.payload_id)
       WHERE project.lifecycle_state = 'active'
         AND NOT EXISTS (
           SELECT 1 FROM storyos.chapter_removal_decisions AS removal
            WHERE removal.owner_user_id = chapter.owner_user_id
              AND removal.project_id = chapter.project_id
              AND removal.chapter_id = chapter.manuscript_object_id
         )
         AND NOT EXISTS (
           SELECT 1 FROM storyos.volume_removal_decisions AS removal
            WHERE removal.owner_user_id = volume.owner_user_id
              AND removal.project_id = volume.project_id
              AND removal.volume_id = volume.manuscript_object_id
         )
    ) AS live;

  SELECT jsonb_build_object(
           'counting_profile', 'storyos.statistics.unicode-16.0.0.v1',
           'chapters', coalesce(
             jsonb_agg(jsonb_build_object(
               'project_id', item->>'project_id',
               'chapter_id', item->>'chapter_id',
               'character_count', char_length(item->>'payload_text')
             )),
             '[]'::jsonb
           )
         )
    INTO live_stats
    FROM jsonb_array_elements(live_search) AS item;

  INSERT INTO storyos.recovery_visibility_proofs (
    visibility_proof_id, restore_proof_id, recovery_copy_id, isolated_target_identity,
    catalog_identity, checks, search_rebuild, statistics_rebuild, lifecycle_range
  ) VALUES (
    p_visibility_proof_id,
    p_restore_proof_id,
    p_recovery_copy_id,
    p_isolated_target_identity,
    p_catalog_identity,
    jsonb_build_object(
      'schema_catalog', p_catalog_identity,
      'forced_rls', true,
      'canonical_heads', true,
      'roles_grants', true,
      'recovery_input', p_chain_sha256
    ),
    live_search,
    live_stats,
    jsonb_build_object('inactive_projects', inactive_projects)
  );

  UPDATE storyos.restore_proofs
     SET state = 'visible'
   WHERE restore_proof_id = p_restore_proof_id
     AND state = 'recovery_hold';
  IF NOT FOUND THEN
    RAISE EXCEPTION 'recovery_visibility_incomplete: restore state';
  END IF;

  ALTER ROLE storyos_runtime LOGIN;
  RETURN 'visible';
END
$_$;

REVOKE ALL ON FUNCTION storyos.pass_recovery_visibility_proof(p_recovery_copy_id uuid, p_restore_proof_id uuid, p_visibility_proof_id uuid, p_isolated_target_identity text, p_chain_sha256 text, p_required_wal_member text, p_recovery_target_lsn text, p_method text, p_catalog_identity text) FROM PUBLIC;
GRANT ALL ON FUNCTION storyos.pass_recovery_visibility_proof(p_recovery_copy_id uuid, p_restore_proof_id uuid, p_visibility_proof_id uuid, p_isolated_target_identity text, p_chain_sha256 text, p_required_wal_member text, p_recovery_target_lsn text, p_method text, p_catalog_identity text) TO storyos_restore;

SET LOCAL ROLE storyos_owner;

-- Author Command Admission reconfirmations

CREATE TABLE storyos.author_command_admission_reconfirmations (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    author_command_admission_id uuid NOT NULL,
    command_id uuid NOT NULL,
    reconfirmation_reason text NOT NULL,
    settled_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT author_command_admission_reconfirma_reconfirmation_reason_check CHECK ((reconfirmation_reason = ANY (ARRAY['admission_expired'::text, 'binding_changed'::text, 'direct_edit_intent_unrecoverable'::text]))),
    CONSTRAINT author_command_admission_reco_owner_user_id_project_id_comm_key UNIQUE (owner_user_id, project_id, command_id),
    CONSTRAINT author_command_admission_reconfirmations_pkey PRIMARY KEY (owner_user_id, project_id, author_command_admission_id),
    CONSTRAINT author_command_admission_reco_owner_user_id_project_id_aut_fkey FOREIGN KEY (owner_user_id, project_id, author_command_admission_id) REFERENCES storyos.author_command_admissions(owner_user_id, project_id, author_command_admission_id) MATCH FULL
);

ALTER TABLE storyos.author_command_admission_reconfirmations ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.author_command_admission_reconfirmations FORCE ROW LEVEL SECURITY;

CREATE POLICY author_command_admission_reconfirmation_exact_scope ON storyos.author_command_admission_reconfirmations USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.author_command_admission_reconfirmations TO storyos_runtime;

CREATE FUNCTION storyos.reject_receipt_after_reconfirmation() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
BEGIN
  IF EXISTS (
    SELECT 1
      FROM storyos.author_command_admission_reconfirmations AS reconfirmation
     WHERE reconfirmation.owner_user_id = NEW.owner_user_id
       AND reconfirmation.project_id = NEW.project_id
       AND reconfirmation.author_command_admission_id = NEW.author_command_admission_id
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'a RequiresReconfirmation Admission cannot append a Receipt settlement';
  END IF;
  RETURN NEW;
END
$$;

CREATE TRIGGER author_command_admission_settlement_before_reconfirmation BEFORE INSERT ON storyos.author_command_admission_settlements FOR EACH ROW EXECUTE FUNCTION storyos.reject_receipt_after_reconfirmation();

CREATE FUNCTION storyos.reject_reconfirmation_after_receipt() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
BEGIN
  IF EXISTS (
    SELECT 1
      FROM storyos.author_command_admission_settlements AS settlement
     WHERE settlement.owner_user_id = NEW.owner_user_id
       AND settlement.project_id = NEW.project_id
       AND settlement.author_command_admission_id = NEW.author_command_admission_id
  ) THEN
    RAISE EXCEPTION USING
      ERRCODE = '23514',
      MESSAGE = 'a Receipt-settled Admission cannot append RequiresReconfirmation';
  END IF;
  RETURN NEW;
END
$$;

CREATE TRIGGER author_command_admission_reconfirmation_before_receipt BEFORE INSERT ON storyos.author_command_admission_reconfirmations FOR EACH ROW EXECUTE FUNCTION storyos.reject_reconfirmation_after_receipt();

-- Human-readable manuscript export operations

CREATE TABLE storyos.human_readable_manuscript_export_operations (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    export_id uuid NOT NULL,
    author_command_admission_id uuid NOT NULL,
    command_id uuid NOT NULL,
    command_kind text NOT NULL,
    idempotency_key uuid NOT NULL,
    source_snapshot_id uuid NOT NULL,
    source_activity_position bigint NOT NULL,
    export_profile text NOT NULL,
    wakeup_pending boolean DEFAULT true NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    claim_generation bigint DEFAULT 0 NOT NULL,
    fence_token bigint DEFAULT 0 NOT NULL,
    lease_expires_at timestamp with time zone,
    settled_result text,
    CONSTRAINT human_readable_manuscript_expor_source_activity_position_check1 CHECK ((source_activity_position >= 0)),
    CONSTRAINT human_readable_manuscript_export_operati_claim_generation_check CHECK ((claim_generation >= 0)),
    CONSTRAINT human_readable_manuscript_export_operation_export_profile_check CHECK ((export_profile = 'storyos.readable-export.utf8-lf.v1'::text)),
    CONSTRAINT human_readable_manuscript_export_operation_settled_result_check CHECK (((settled_result IS NULL) OR (settled_result = ANY (ARRAY['failed'::text, 'ready'::text])))),
    CONSTRAINT human_readable_manuscript_export_operations_command_kind_check CHECK ((command_kind = 'exportHumanReadableManuscript'::text)),
    CONSTRAINT human_readable_manuscript_export_operations_fence_token_check CHECK ((fence_token >= 0)),
    CONSTRAINT human_readable_manuscript_exp_owner_user_id_project_id_auth_key UNIQUE (owner_user_id, project_id, author_command_admission_id),
    CONSTRAINT human_readable_manuscript_exp_owner_user_id_project_id_comm_key UNIQUE (owner_user_id, project_id, command_id),
    CONSTRAINT human_readable_manuscript_exp_owner_user_id_project_id_idem_key UNIQUE (owner_user_id, project_id, idempotency_key),
    CONSTRAINT human_readable_manuscript_export_operations_pkey PRIMARY KEY (owner_user_id, project_id, export_id),
    CONSTRAINT human_readable_manuscript_exp_owner_user_id_project_id_aut_fkey FOREIGN KEY (owner_user_id, project_id, author_command_admission_id) REFERENCES storyos.author_command_admissions(owner_user_id, project_id, author_command_admission_id) MATCH FULL,
    CONSTRAINT human_readable_manuscript_exp_owner_user_id_project_id_com_fkey FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key) REFERENCES storyos.command_idempotency(owner_user_id, project_id, command_kind, idempotency_key) MATCH FULL,
    CONSTRAINT human_readable_manuscript_exp_owner_user_id_project_id_sou_fkey FOREIGN KEY (owner_user_id, project_id, source_snapshot_id) REFERENCES storyos.project_snapshots(owner_user_id, project_id, snapshot_id) MATCH FULL,
    CONSTRAINT human_readable_manuscript_export__owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

ALTER TABLE storyos.human_readable_manuscript_export_operations ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.human_readable_manuscript_export_operations FORCE ROW LEVEL SECURITY;

CREATE POLICY human_readable_manuscript_export_operations_exact_scope ON storyos.human_readable_manuscript_export_operations USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

CREATE POLICY human_readable_manuscript_export_operations_worker_select ON storyos.human_readable_manuscript_export_operations FOR SELECT USING (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL)));

CREATE POLICY human_readable_manuscript_export_operations_worker_update ON storyos.human_readable_manuscript_export_operations FOR UPDATE USING (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL))) WITH CHECK (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.human_readable_manuscript_export_operations TO storyos_runtime;

-- Project export operations

CREATE TABLE storyos.project_export_operations (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    export_id uuid NOT NULL,
    author_command_admission_id uuid NOT NULL,
    command_id uuid NOT NULL,
    command_kind text NOT NULL,
    idempotency_key uuid NOT NULL,
    source_snapshot_id uuid NOT NULL,
    source_activity_position bigint NOT NULL,
    archive_profile text NOT NULL,
    archive_path_profile text NOT NULL,
    wakeup_pending boolean DEFAULT true NOT NULL,
    claim_generation bigint DEFAULT 0 NOT NULL,
    fence_token bigint DEFAULT 0 NOT NULL,
    lease_expires_at timestamp with time zone,
    settled_result text,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT project_export_operations_archive_path_profile_check CHECK ((archive_path_profile = 'storyos.archive-path.utf8-nfc-unicode-16.0.0.v1'::text)),
    CONSTRAINT project_export_operations_archive_profile_check CHECK ((archive_profile = 'storyos.project-export.v1'::text)),
    CONSTRAINT project_export_operations_claim_generation_check CHECK ((claim_generation >= 0)),
    CONSTRAINT project_export_operations_command_kind_check CHECK ((command_kind = 'exportProjectArchive'::text)),
    CONSTRAINT project_export_operations_fence_token_check CHECK ((fence_token >= 0)),
    CONSTRAINT project_export_operations_settled_result_check CHECK (((settled_result IS NULL) OR (settled_result = ANY (ARRAY['failed'::text, 'ready'::text])))),
    CONSTRAINT project_export_operations_source_activity_position_check CHECK ((source_activity_position >= 0)),
    CONSTRAINT project_export_operations_owner_user_id_project_id_author_c_key UNIQUE (owner_user_id, project_id, author_command_admission_id),
    CONSTRAINT project_export_operations_owner_user_id_project_id_command__key UNIQUE (owner_user_id, project_id, command_id),
    CONSTRAINT project_export_operations_owner_user_id_project_id_idempote_key UNIQUE (owner_user_id, project_id, idempotency_key),
    CONSTRAINT project_export_operations_pkey PRIMARY KEY (owner_user_id, project_id, export_id),
    CONSTRAINT project_export_operations_owner_user_id_project_id_author__fkey FOREIGN KEY (owner_user_id, project_id, author_command_admission_id) REFERENCES storyos.author_command_admissions(owner_user_id, project_id, author_command_admission_id) MATCH FULL,
    CONSTRAINT project_export_operations_owner_user_id_project_id_command_fkey FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key) REFERENCES storyos.command_idempotency(owner_user_id, project_id, command_kind, idempotency_key) MATCH FULL,
    CONSTRAINT project_export_operations_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT project_export_operations_owner_user_id_project_id_source__fkey FOREIGN KEY (owner_user_id, project_id, source_snapshot_id) REFERENCES storyos.project_snapshots(owner_user_id, project_id, snapshot_id) MATCH FULL
);

ALTER TABLE storyos.project_export_operations ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_export_operations FORCE ROW LEVEL SECURITY;

CREATE POLICY project_export_operations_exact_scope ON storyos.project_export_operations USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

CREATE POLICY project_export_operations_worker_select ON storyos.project_export_operations FOR SELECT USING (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL)));

CREATE POLICY project_export_operations_worker_update ON storyos.project_export_operations FOR UPDATE USING (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL))) WITH CHECK (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.project_export_operations TO storyos_runtime;

-- Pinned export sources

CREATE TABLE storyos.pinned_export_sources (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    export_id uuid NOT NULL,
    source_snapshot_id uuid NOT NULL,
    completeness_profile text NOT NULL,
    facts jsonb NOT NULL,
    facts_sha256 text NOT NULL,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT pinned_export_sources_completeness_profile_check CHECK ((completeness_profile = ANY (ARRAY['human_readable_manuscript'::text, 'project_export_archive'::text]))),
    CONSTRAINT pinned_export_sources_facts_sha256_check CHECK ((facts_sha256 ~ '^[0-9a-f]{64}$'::text)),
    CONSTRAINT pinned_export_sources_pkey PRIMARY KEY (owner_user_id, project_id, export_id),
    CONSTRAINT pinned_export_sources_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT pinned_export_sources_owner_user_id_project_id_source_snap_fkey FOREIGN KEY (owner_user_id, project_id, source_snapshot_id) REFERENCES storyos.project_snapshots(owner_user_id, project_id, snapshot_id) MATCH FULL
);

ALTER TABLE storyos.pinned_export_sources ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.pinned_export_sources FORCE ROW LEVEL SECURITY;

CREATE POLICY pinned_export_sources_exact_scope ON storyos.pinned_export_sources USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

CREATE POLICY pinned_export_sources_worker_select ON storyos.pinned_export_sources FOR SELECT USING (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL)));

CREATE POLICY pinned_export_sources_worker_update ON storyos.pinned_export_sources FOR UPDATE USING (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL))) WITH CHECK (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.pinned_export_sources TO storyos_runtime;

-- Storage activation ledger

CREATE TABLE storyos.schema_migrations (
    schema_version text NOT NULL,
    migration_id text NOT NULL,
    checksum text NOT NULL,
    release_identity text NOT NULL,
    runner_revision text NOT NULL,
    started_at timestamp with time zone NOT NULL,
    finished_at timestamp with time zone,
    status text NOT NULL,
    CONSTRAINT schema_migrations_checksum_check CHECK ((checksum ~ '^sha256:[0-9a-f]{64}$'::text)),
    CONSTRAINT schema_migrations_status_check CHECK ((status = 'applied'::text)),
    CONSTRAINT schema_migrations_pkey PRIMARY KEY (schema_version)
);

CREATE TABLE storyos.migration_phases (
    attempt_id bigint NOT NULL,
    phase_id text NOT NULL,
    class text NOT NULL,
    postcondition text NOT NULL,
    disposition text NOT NULL,
    recorded_at timestamp with time zone NOT NULL,
    CONSTRAINT migration_phases_disposition_check CHECK ((disposition = 'passed'::text)),
    CONSTRAINT migration_phases_phase_id_check CHECK ((phase_id = ANY (ARRAY['preflight'::text, 'initial_schema_bootstrap'::text, 'validate_transaction'::text, 'activate'::text]))),
    CONSTRAINT migration_phases_pkey PRIMARY KEY (attempt_id, phase_id)
);

CREATE TABLE storyos.migration_phase_checksums (
    attempt_id bigint NOT NULL,
    source_path text NOT NULL,
    checksum text NOT NULL,
    CONSTRAINT migration_phase_checksums_checksum_check CHECK ((checksum ~ '^sha256:[0-9a-f]{64}$'::text)),
    CONSTRAINT migration_phase_checksums_pkey PRIMARY KEY (attempt_id, source_path)
);

CREATE TABLE storyos.storage_activation_proofs (
    proof_id text NOT NULL,
    phase text NOT NULL,
    catalog_id text NOT NULL,
    catalog_checksum text NOT NULL,
    migration_chain_id text NOT NULL,
    migration_chain_digest text NOT NULL,
    database_schema_identity text NOT NULL,
    active_schema_version text NOT NULL,
    public_release text NOT NULL,
    route_catalog_id text NOT NULL,
    route_catalog_sha256 text NOT NULL,
    activated_at timestamp with time zone NOT NULL,
    CONSTRAINT storage_activation_proofs_catalog_checksum_check CHECK ((catalog_checksum ~ '^sha256:[0-9a-f]{64}$'::text)),
    CONSTRAINT storage_activation_proofs_migration_chain_digest_check CHECK ((migration_chain_digest ~ '^sha256:[0-9a-f]{64}$'::text)),
    CONSTRAINT storage_activation_proofs_phase_check CHECK ((phase = 'active'::text)),
    CONSTRAINT storage_activation_proofs_proof_id_check CHECK ((proof_id = 'release-1'::text)),
    CONSTRAINT storage_activation_proofs_route_catalog_sha256_check CHECK ((route_catalog_sha256 ~ '^sha256:[0-9a-f]{64}$'::text)),
    CONSTRAINT storage_activation_proofs_pkey PRIMARY KEY (proof_id)
);

GRANT SELECT ON TABLE storyos.storage_activation_proofs TO storyos_runtime;

-- Authority history floors

CREATE TABLE storyos.authority_history_floors (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    floor_activity_position numeric(20,0) NOT NULL,
    snapshot_id uuid NOT NULL,
    CONSTRAINT authority_history_floors_floor_activity_position_check CHECK (((floor_activity_position >= (0)::numeric) AND (floor_activity_position <= '18446744073709551615'::numeric))),
    CONSTRAINT authority_history_floors_pkey PRIMARY KEY (owner_user_id, project_id),
    CONSTRAINT authority_history_floors_project_fk FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT authority_history_floors_snapshot_fk FOREIGN KEY (owner_user_id, project_id, snapshot_id) REFERENCES storyos.project_snapshots(owner_user_id, project_id, snapshot_id) MATCH FULL
);

CREATE INDEX authority_history_floors_snapshot_fk_idx ON storyos.authority_history_floors USING btree (owner_user_id, project_id, snapshot_id);

ALTER TABLE storyos.authority_history_floors ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.authority_history_floors FORCE ROW LEVEL SECURITY;

CREATE POLICY authority_history_floors_exact_scope ON storyos.authority_history_floors USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.authority_history_floors TO storyos_runtime;

CREATE FUNCTION storyos.ensure_authority_history_floor() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
BEGIN
  INSERT INTO storyos.authority_history_floors (
    owner_user_id, project_id, floor_activity_position, snapshot_id
  )
  VALUES (
    NEW.owner_user_id, NEW.project_id, NEW.project_activity_position, NEW.snapshot_id
  )
  ON CONFLICT DO NOTHING;
  RETURN NULL;
END
$$;

GRANT ALL ON FUNCTION storyos.ensure_authority_history_floor() TO storyos_runtime;

CREATE TRIGGER project_snapshots_authority_history_floor AFTER INSERT ON storyos.project_snapshots FOR EACH ROW EXECUTE FUNCTION storyos.ensure_authority_history_floor();

-- Project policy, destinations, and model registrations

CREATE TABLE storyos.model_capability_profiles (
    capability_profile_revision text NOT NULL,
    profile jsonb NOT NULL,
    CONSTRAINT model_capability_profiles_capability_profile_revision_check CHECK (((char_length(capability_profile_revision) >= 1) AND (char_length(capability_profile_revision) <= 200))),
    CONSTRAINT model_capability_profiles_profile_check CHECK ((jsonb_typeof(profile) = 'object'::text)),
    CONSTRAINT model_capability_profiles_pkey PRIMARY KEY (capability_profile_revision)
);

GRANT SELECT,INSERT ON TABLE storyos.model_capability_profiles TO storyos_runtime;

CREATE TABLE storyos.model_registration_revisions (
    model_registration_revision uuid NOT NULL,
    model_kind text NOT NULL,
    api_surface text NOT NULL,
    provider_model_id text NOT NULL,
    capability_profile_revision text NOT NULL,
    CONSTRAINT model_registration_revisions_api_surface_check CHECK (((char_length(api_surface) >= 1) AND (char_length(api_surface) <= 200))),
    CONSTRAINT model_registration_revisions_model_kind_check CHECK ((model_kind = ANY (ARRAY['host_fake'::text, 'volcengine_agent_plan_responses'::text]))),
    CONSTRAINT model_registration_revisions_provider_model_id_check CHECK (char_length(provider_model_id) BETWEEN 1 AND 200 AND provider_model_id <> 'latest'),
    CONSTRAINT model_registration_revisions_pkey PRIMARY KEY (model_registration_revision),
    CONSTRAINT model_registration_revisions_capability_profile_revision_fkey FOREIGN KEY (capability_profile_revision) REFERENCES storyos.model_capability_profiles(capability_profile_revision)
);

CREATE INDEX model_registration_revisions_profile_idx ON storyos.model_registration_revisions USING btree (capability_profile_revision);

GRANT SELECT,INSERT ON TABLE storyos.model_registration_revisions TO storyos_runtime;

CREATE TABLE storyos.model_registration_heads (
    model_kind text NOT NULL,
    model_registration_revision uuid NOT NULL,
    CONSTRAINT model_registration_heads_model_kind_check CHECK ((model_kind = ANY (ARRAY['host_fake'::text, 'volcengine_agent_plan_responses'::text]))),
    CONSTRAINT model_registration_heads_pkey PRIMARY KEY (model_kind),
    CONSTRAINT model_registration_heads_model_registration_revision_fkey FOREIGN KEY (model_registration_revision) REFERENCES storyos.model_registration_revisions(model_registration_revision)
);

GRANT SELECT,INSERT ON TABLE storyos.model_registration_heads TO storyos_runtime;

GRANT UPDATE(model_registration_revision) ON TABLE storyos.model_registration_heads TO storyos_runtime;

-- Global Tool Registrations (ADR 0083). The rows hold no Project data, Credential Reference,
-- grant, or use state, and they are not Project Export data. Each runtime role reads each row.
-- Only an active revision changes, and only to quarantined or retired. The quarantine columns are
-- the Activity record of the quarantine. A tool row joins only the transaction that inserts its
-- revision, so the tool set of a stored revision does not change.

CREATE TABLE storyos.tool_registration_revisions (
    tool_registration_revision uuid NOT NULL,
    registration_kind text NOT NULL,
    server_url text NOT NULL,
    server_name text NOT NULL,
    protocol_revision text NOT NULL,
    adapter_revision text NOT NULL,
    lifecycle text DEFAULT 'active'::text NOT NULL,
    quarantine_differences jsonb,
    quarantined_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT tool_registration_revisions_pkey PRIMARY KEY (tool_registration_revision),
    CONSTRAINT tool_registration_revisions_kind_check CHECK ((registration_kind = 'mcp_remote'::text)),
    CONSTRAINT tool_registration_revisions_identity_check CHECK (char_length(server_url) BETWEEN 1 AND 2048 AND char_length(server_name) BETWEEN 1 AND 200 AND char_length(protocol_revision) BETWEEN 1 AND 64 AND char_length(adapter_revision) BETWEEN 1 AND 200),
    CONSTRAINT tool_registration_revisions_lifecycle_check CHECK ((lifecycle = ANY (ARRAY['active'::text, 'quarantined'::text, 'retired'::text]))),
    CONSTRAINT tool_registration_revisions_quarantine_check CHECK (
        ((quarantine_differences IS NULL) = (quarantined_at IS NULL))
        AND (lifecycle <> 'active'::text OR quarantine_differences IS NULL)
        AND (lifecycle <> 'quarantined'::text OR quarantine_differences IS NOT NULL)
        AND (quarantine_differences IS NULL
             OR (jsonb_typeof(quarantine_differences) = 'array'::text AND jsonb_array_length(quarantine_differences) >= 1)))
);

ALTER TABLE storyos.tool_registration_revisions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.tool_registration_revisions FORCE ROW LEVEL SECURITY;

CREATE POLICY tool_registration_revisions_global_read ON storyos.tool_registration_revisions FOR SELECT USING (true);

CREATE POLICY tool_registration_revisions_active_insert ON storyos.tool_registration_revisions FOR INSERT WITH CHECK ((lifecycle = 'active'::text));

CREATE POLICY tool_registration_revisions_lifecycle_update ON storyos.tool_registration_revisions FOR UPDATE USING ((lifecycle = 'active'::text)) WITH CHECK ((lifecycle <> 'active'::text));

GRANT SELECT,INSERT ON TABLE storyos.tool_registration_revisions TO storyos_runtime;

GRANT UPDATE(lifecycle, quarantine_differences, quarantined_at) ON TABLE storyos.tool_registration_revisions TO storyos_runtime;

CREATE TABLE storyos.tool_registration_tools (
    tool_registration_revision uuid NOT NULL,
    tool_name text NOT NULL,
    tool_spec_revision text NOT NULL,
    input_schema_digest text NOT NULL,
    input_contract_digest text NOT NULL,
    output_schema_digest text,
    provenance_digest text NOT NULL,
    CONSTRAINT tool_registration_tools_pkey PRIMARY KEY (tool_registration_revision, tool_name),
    CONSTRAINT tool_registration_tools_name_check CHECK (char_length(tool_name) BETWEEN 1 AND 128 AND char_length(tool_spec_revision) BETWEEN 1 AND 200),
    CONSTRAINT tool_registration_tools_digest_check CHECK (input_schema_digest ~ '^sha256:[0-9a-f]{64}$'::text AND input_contract_digest ~ '^sha256:[0-9a-f]{64}$'::text AND provenance_digest ~ '^sha256:[0-9a-f]{64}$'::text AND (output_schema_digest IS NULL OR output_schema_digest ~ '^sha256:[0-9a-f]{64}$'::text)),
    CONSTRAINT tool_registration_tools_revision_fkey FOREIGN KEY (tool_registration_revision) REFERENCES storyos.tool_registration_revisions(tool_registration_revision)
);

ALTER TABLE storyos.tool_registration_tools ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.tool_registration_tools FORCE ROW LEVEL SECURITY;

CREATE POLICY tool_registration_tools_global_read ON storyos.tool_registration_tools FOR SELECT USING (true);

CREATE POLICY tool_registration_tools_revision_insert ON storyos.tool_registration_tools FOR INSERT WITH CHECK ((EXISTS ( SELECT 1 FROM storyos.tool_registration_revisions AS revision WHERE ((revision.tool_registration_revision = tool_registration_tools.tool_registration_revision) AND (revision.lifecycle = 'active'::text) AND (revision.xmin = (pg_current_xact_id())::xid)))));

GRANT SELECT,INSERT ON TABLE storyos.tool_registration_tools TO storyos_runtime;

CREATE TABLE storyos.processing_destination_identities (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    processing_destination_identity uuid NOT NULL,
    destination_kind text NOT NULL,
    control_classification text NOT NULL,
    CONSTRAINT processing_destination_identities_kind_check CHECK ((destination_kind = ANY (ARRAY['host_fake'::text, 'volcengine_agent_plan'::text, 'mcp_remote'::text]))),
    CONSTRAINT processing_destination_identities_control_check CHECK ((control_classification = CASE destination_kind WHEN 'host_fake'::text THEN 'host_internal'::text ELSE 'external'::text END)),
    CONSTRAINT processing_destination_identities_pkey PRIMARY KEY (owner_user_id, project_id, processing_destination_identity),
    CONSTRAINT processing_destination_identities_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

ALTER TABLE storyos.processing_destination_identities ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.processing_destination_identities FORCE ROW LEVEL SECURITY;

CREATE POLICY processing_destination_identities_exact_scope ON storyos.processing_destination_identities USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.processing_destination_identities TO storyos_runtime;

CREATE TABLE storyos.processing_destination_identity_evidence_revisions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    processing_destination_identity uuid NOT NULL,
    evidence_revision numeric(20,0) NOT NULL,
    evidence_kind text NOT NULL,
    endpoint text,
    account_boundary text,
    eligibility_evidence text,
    CONSTRAINT processing_destination_identity_evid_eligibility_evidence_check CHECK (((char_length(eligibility_evidence) >= 1) AND (char_length(eligibility_evidence) <= 500))),
    CONSTRAINT processing_destination_identity_evidenc_evidence_revision_check CHECK (((evidence_revision >= (1)::numeric) AND (evidence_revision <= '18446744073709551615'::numeric))),
    CONSTRAINT processing_destination_identity_evidence_account_boundary_check CHECK (((char_length(account_boundary) >= 1) AND (char_length(account_boundary) <= 500))),
    CONSTRAINT processing_destination_identity_evidence_kind_check CHECK ((evidence_kind = ANY (ARRAY['host_fake_boundary'::text, 'volcengine_agent_plan_boundary'::text, 'mcp_remote_boundary'::text]))),
    CONSTRAINT processing_destination_identity_evidence_revisio_endpoint_check CHECK (((char_length(endpoint) >= 1) AND (char_length(endpoint) <= 500))),
    CONSTRAINT processing_destination_identity_evidence_shape CHECK ((((evidence_kind = 'host_fake_boundary'::text) AND (endpoint IS NULL) AND (account_boundary IS NULL) AND (eligibility_evidence IS NULL)) OR ((evidence_kind = 'volcengine_agent_plan_boundary'::text) AND (endpoint IS NOT NULL) AND (account_boundary IS NOT NULL) AND (eligibility_evidence IS NOT NULL)) OR ((evidence_kind = 'mcp_remote_boundary'::text) AND (endpoint IS NOT NULL) AND (account_boundary IS NOT NULL) AND (eligibility_evidence IS NULL)))),
    CONSTRAINT processing_destination_identity_evidence_revisions_pkey PRIMARY KEY (owner_user_id, project_id, processing_destination_identity, evidence_revision),
    CONSTRAINT processing_destination_identi_owner_user_id_project_id_pro_fkey FOREIGN KEY (owner_user_id, project_id, processing_destination_identity) REFERENCES storyos.processing_destination_identities(owner_user_id, project_id, processing_destination_identity) MATCH FULL
);

ALTER TABLE storyos.processing_destination_identity_evidence_revisions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.processing_destination_identity_evidence_revisions FORCE ROW LEVEL SECURITY;

CREATE POLICY processing_destination_identity_evidence_revisions_exact_scope ON storyos.processing_destination_identity_evidence_revisions USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.processing_destination_identity_evidence_revisions TO storyos_runtime;

CREATE TABLE storyos.project_destination_grants (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    grant_id uuid NOT NULL,
    processing_destination_identity uuid NOT NULL,
    grant_kind text NOT NULL,
    grant_scope text,
    CONSTRAINT project_destination_grants_grant_scope_check CHECK ((grant_scope = ANY (ARRAY['project_writing_assistance_v1'::text, 'project_writing_assistance_v2'::text, 'project_writing_assistance_v3'::text, 'research_tool_v1'::text]))),
    CONSTRAINT project_destination_grants_kind_check CHECK ((grant_kind = ANY (ARRAY['host_fake_use'::text, 'volcengine_agent_plan_use'::text, 'research_tool_use'::text]))),
    CONSTRAINT project_destination_grants_research_scope CHECK (((grant_kind = 'research_tool_use'::text) = (grant_scope IS NOT DISTINCT FROM 'research_tool_v1'::text))),
    CONSTRAINT project_destination_grants_pkey PRIMARY KEY (owner_user_id, project_id, grant_id),
    CONSTRAINT project_destination_grants_owner_user_id_project_id_proces_fkey FOREIGN KEY (owner_user_id, project_id, processing_destination_identity) REFERENCES storyos.processing_destination_identities(owner_user_id, project_id, processing_destination_identity) MATCH FULL
);

CREATE INDEX project_destination_grants_identity_idx ON storyos.project_destination_grants USING btree (owner_user_id, project_id, processing_destination_identity);

ALTER TABLE storyos.project_destination_grants ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_destination_grants FORCE ROW LEVEL SECURITY;

CREATE POLICY project_destination_grants_exact_scope ON storyos.project_destination_grants USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_destination_grants TO storyos_runtime;

CREATE TABLE storyos.project_external_use_binding_revisions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    project_model_use_binding_revision uuid NOT NULL,
    processing_destination_identity uuid NOT NULL,
    evidence_revision numeric(20,0) NOT NULL,
    grant_id uuid NOT NULL,
    model_registration_revision uuid,
    tool_registration_revision uuid,
    credential_source text NOT NULL,
    credential_reference text,
    budget_bounds text NOT NULL,
    CONSTRAINT project_external_use_binding_revisio_credential_reference_check CHECK (((char_length(credential_reference) >= 1) AND (char_length(credential_reference) <= 500))),
    CONSTRAINT project_external_use_binding_revisions_budget_bounds_check CHECK ((budget_bounds = ANY (ARRAY['not_required'::text, 'unqualified'::text, 'agent_plan_request_v1'::text, 'agent_plan_request_v2'::text, 'agent_plan_request_v3'::text]))),
    CONSTRAINT project_external_use_binding_revisions_credential_shape CHECK (((credential_source = 'none'::text) = (credential_reference IS NULL))),
    CONSTRAINT project_external_use_binding_revisions_credential_source_check CHECK ((credential_source = ANY (ARRAY['none'::text, 'operator_quota'::text]))),
    CONSTRAINT project_external_use_binding_revisions_registration_shape CHECK ((num_nonnulls(model_registration_revision, tool_registration_revision) = 1)),
    CONSTRAINT project_external_use_binding_revisions_evidence_revision_check CHECK (((evidence_revision >= (1)::numeric) AND (evidence_revision <= '18446744073709551615'::numeric))),
    CONSTRAINT project_external_use_binding_revisions_pkey PRIMARY KEY (owner_user_id, project_id, project_model_use_binding_revision),
    CONSTRAINT project_external_use_binding_revisions_registration_key UNIQUE (owner_user_id, project_id, project_model_use_binding_revision, model_registration_revision),
    CONSTRAINT project_external_use_binding__owner_user_id_project_id_gra_fkey FOREIGN KEY (owner_user_id, project_id, grant_id) REFERENCES storyos.project_destination_grants(owner_user_id, project_id, grant_id) MATCH FULL,
    CONSTRAINT project_external_use_binding__owner_user_id_project_id_pro_fkey FOREIGN KEY (owner_user_id, project_id, processing_destination_identity, evidence_revision) REFERENCES storyos.processing_destination_identity_evidence_revisions(owner_user_id, project_id, processing_destination_identity, evidence_revision) MATCH FULL,
    CONSTRAINT project_external_use_binding_r_model_registration_revision_fkey FOREIGN KEY (model_registration_revision) REFERENCES storyos.model_registration_revisions(model_registration_revision),
    CONSTRAINT project_external_use_binding_r_tool_registration_revision_fkey FOREIGN KEY (tool_registration_revision) REFERENCES storyos.tool_registration_revisions(tool_registration_revision)
);

CREATE INDEX project_external_use_binding_revisions_grant_idx ON storyos.project_external_use_binding_revisions USING btree (owner_user_id, project_id, grant_id);

CREATE INDEX project_external_use_binding_revisions_refs_idx ON storyos.project_external_use_binding_revisions USING btree (owner_user_id, project_id, processing_destination_identity, evidence_revision);

ALTER TABLE storyos.project_external_use_binding_revisions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_external_use_binding_revisions FORCE ROW LEVEL SECURITY;

CREATE POLICY project_external_use_binding_revisions_exact_scope ON storyos.project_external_use_binding_revisions USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_external_use_binding_revisions TO storyos_runtime;

CREATE TABLE storyos.external_contract_compatibility_decisions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    external_compatibility_decision uuid NOT NULL,
    project_model_use_binding_revision uuid NOT NULL,
    decision_kind text NOT NULL,
    runtime_qualification text,
    qualification_authorization text,
    CONSTRAINT external_contract_compatibili_qualification_authorization_check CHECK (((char_length(qualification_authorization) >= 1) AND (char_length(qualification_authorization) <= 500))),
    CONSTRAINT external_contract_compatibility_decisions_authorization_shape CHECK (((runtime_qualification = 'qualification_authorized'::text) = (qualification_authorization IS NOT NULL))),
    CONSTRAINT external_contract_compatibility_decisions_kind_check CHECK ((decision_kind = ANY (ARRAY['host_fake_compatible'::text, 'volcengine_agent_plan_compatible'::text, 'tool_registration_compatible'::text]))),
    CONSTRAINT external_contract_compatibility_decisions_tool_shape CHECK (((decision_kind = 'tool_registration_compatible'::text) = (runtime_qualification IS NULL) AND (runtime_qualification IS NOT NULL OR qualification_authorization IS NULL))),
    CONSTRAINT external_contract_compatibility_decisions_qualification_check CHECK ((runtime_qualification = ANY (ARRAY['qualified'::text, 'pending'::text, 'qualification_authorized'::text]))),
    CONSTRAINT external_contract_compatibility_decisions_pkey PRIMARY KEY (owner_user_id, project_id, external_compatibility_decision),
    CONSTRAINT external_contract_compatibili_owner_user_id_project_id_pro_fkey FOREIGN KEY (owner_user_id, project_id, project_model_use_binding_revision) REFERENCES storyos.project_external_use_binding_revisions(owner_user_id, project_id, project_model_use_binding_revision) MATCH FULL
);

CREATE INDEX external_contract_compatibility_decisions_binding_idx ON storyos.external_contract_compatibility_decisions USING btree (owner_user_id, project_id, project_model_use_binding_revision);

ALTER TABLE storyos.external_contract_compatibility_decisions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.external_contract_compatibility_decisions FORCE ROW LEVEL SECURITY;

CREATE POLICY external_contract_compatibility_decisions_exact_scope ON storyos.external_contract_compatibility_decisions USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.external_contract_compatibility_decisions TO storyos_runtime;

-- Project Tool Enablement (ADR 0083). Each revision is append-only. An enabled revision names the
-- compatibility Decision of its binding records; a disabled revision names none, and the earlier
-- records stay as history.

CREATE TABLE storyos.project_tool_enablements (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    registration_id text NOT NULL,
    enablement_revision numeric(20,0) NOT NULL,
    enablement text NOT NULL,
    external_compatibility_decision uuid,
    receipt_id uuid NOT NULL,
    CONSTRAINT project_tool_enablements_pkey PRIMARY KEY (owner_user_id, project_id, registration_id, enablement_revision),
    CONSTRAINT project_tool_enablements_registration_id_check CHECK (char_length(registration_id) BETWEEN 1 AND 128),
    CONSTRAINT project_tool_enablements_revision_check CHECK (((enablement_revision >= (1)::numeric) AND (enablement_revision <= '18446744073709551615'::numeric))),
    CONSTRAINT project_tool_enablements_enablement_check CHECK ((enablement = ANY (ARRAY['enabled'::text, 'disabled'::text]))),
    CONSTRAINT project_tool_enablements_decision_shape CHECK (((enablement = 'enabled'::text) = (external_compatibility_decision IS NOT NULL))),
    CONSTRAINT project_tool_enablements_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT project_tool_enablements_decision_fkey FOREIGN KEY (owner_user_id, project_id, external_compatibility_decision) REFERENCES storyos.external_contract_compatibility_decisions(owner_user_id, project_id, external_compatibility_decision),
    CONSTRAINT project_tool_enablements_receipt_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);

CREATE INDEX project_tool_enablements_decision_idx ON storyos.project_tool_enablements USING btree (owner_user_id, project_id, external_compatibility_decision);

ALTER TABLE storyos.project_tool_enablements ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_tool_enablements FORCE ROW LEVEL SECURITY;

CREATE POLICY project_tool_enablements_exact_scope ON storyos.project_tool_enablements USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_tool_enablements TO storyos_runtime;

CREATE TABLE storyos.project_policy_revisions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    policy_revision numeric(20,0) NOT NULL,
    availability text NOT NULL,
    receipt_id uuid NOT NULL,
    external_compatibility_decision uuid NOT NULL,
    CONSTRAINT project_policy_revisions_availability_check CHECK ((availability = ANY (ARRAY['available'::text, 'unavailable'::text]))),
    CONSTRAINT project_policy_revisions_policy_revision_check CHECK (((policy_revision >= (1)::numeric) AND (policy_revision <= '18446744073709551615'::numeric))),
    CONSTRAINT project_policy_revisions_pkey PRIMARY KEY (owner_user_id, project_id, policy_revision),
    CONSTRAINT project_policy_revisions_decision_fkey FOREIGN KEY (owner_user_id, project_id, external_compatibility_decision) REFERENCES storyos.external_contract_compatibility_decisions(owner_user_id, project_id, external_compatibility_decision) MATCH FULL,
    CONSTRAINT project_policy_revisions_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT project_policy_revisions_owner_user_id_project_id_receipt__fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);

CREATE INDEX project_policy_revisions_decision_idx ON storyos.project_policy_revisions USING btree (owner_user_id, project_id, external_compatibility_decision);

ALTER TABLE storyos.project_policy_revisions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_policy_revisions FORCE ROW LEVEL SECURITY;

CREATE POLICY project_policy_revisions_exact_scope ON storyos.project_policy_revisions USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_policy_revisions TO storyos_runtime;

-- Agents, Conversations, and Agent Runs

CREATE TABLE storyos.project_agents (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    project_agent_id uuid NOT NULL,
    CONSTRAINT project_agents_owner_user_id_project_id_key UNIQUE (owner_user_id, project_id),
    CONSTRAINT project_agents_pkey PRIMARY KEY (owner_user_id, project_id, project_agent_id),
    CONSTRAINT project_agents_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL
);

ALTER TABLE storyos.project_agents ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_agents FORCE ROW LEVEL SECURITY;

CREATE POLICY project_agents_exact_scope ON storyos.project_agents USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_agents TO storyos_runtime;

CREATE TABLE storyos.project_conversations (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
    project_agent_id uuid NOT NULL,
    created_receipt_id uuid NOT NULL,
    CONSTRAINT project_conversations_pkey PRIMARY KEY (owner_user_id, project_id, conversation_id),
    CONSTRAINT project_conversations_owner_user_id_project_id_created_rec_fkey FOREIGN KEY (owner_user_id, project_id, created_receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
    CONSTRAINT project_conversations_owner_user_id_project_id_project_age_fkey FOREIGN KEY (owner_user_id, project_id, project_agent_id) REFERENCES storyos.project_agents(owner_user_id, project_id, project_agent_id) MATCH FULL
);

CREATE INDEX project_conversations_agent_idx ON storyos.project_conversations USING btree (owner_user_id, project_id, project_agent_id);

CREATE INDEX project_conversations_created_receipt_idx ON storyos.project_conversations USING btree (owner_user_id, project_id, created_receipt_id);

ALTER TABLE storyos.project_conversations ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.project_conversations FORCE ROW LEVEL SECURITY;

CREATE POLICY project_conversations_exact_scope ON storyos.project_conversations USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.project_conversations TO storyos_runtime;

CREATE TABLE storyos.conversation_memory_settings (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
    memory_settings_revision uuid NOT NULL,
    use_enabled boolean NOT NULL,
    contribution_enabled boolean NOT NULL,
    receipt_id uuid NOT NULL,
    is_current boolean DEFAULT true,
    CONSTRAINT conversation_memory_settings_pkey PRIMARY KEY (owner_user_id, project_id, conversation_id, memory_settings_revision),
    CONSTRAINT conversation_memory_settings_owner_user_id_project_id_conv_fkey FOREIGN KEY (owner_user_id, project_id, conversation_id) REFERENCES storyos.project_conversations(owner_user_id, project_id, conversation_id) MATCH FULL,
    CONSTRAINT conversation_memory_settings_owner_user_id_project_id_rece_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);

CREATE UNIQUE INDEX conversation_memory_settings_one_current ON storyos.conversation_memory_settings USING btree (owner_user_id, project_id, conversation_id) WHERE COALESCE(is_current, true);

CREATE INDEX conversation_memory_settings_receipt_idx ON storyos.conversation_memory_settings USING btree (owner_user_id, project_id, receipt_id);

ALTER TABLE storyos.conversation_memory_settings ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.conversation_memory_settings FORCE ROW LEVEL SECURITY;

CREATE POLICY conversation_memory_settings_exact_scope ON storyos.conversation_memory_settings USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.conversation_memory_settings TO storyos_runtime;

CREATE TABLE storyos.agent_runs (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    run_id uuid NOT NULL,
    project_agent_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
    memory_settings_revision uuid NOT NULL,
    grant_id uuid NOT NULL,
    project_model_use_binding_revision uuid NOT NULL,
    chapter_id uuid NOT NULL,
    author_message text NOT NULL,
    status storyos.agent_run_status NOT NULL,
    receipt_id uuid NOT NULL,
    claim_generation bigint DEFAULT 0 NOT NULL,
    fence_token bigint DEFAULT 0 NOT NULL,
    lease_expires_at timestamp with time zone,
    wakeup_pending boolean DEFAULT true NOT NULL,
    settlement jsonb,
    predecessor_run_id uuid,
    active_decision_position numeric(20,0) DEFAULT 0 NOT NULL,
    model_registration_revision uuid NOT NULL,
    CONSTRAINT agent_runs_active_decision_position_check CHECK (((active_decision_position >= (0)::numeric) AND (active_decision_position <= '18446744073709551615'::numeric))),
    CONSTRAINT agent_runs_author_message_check CHECK (((char_length(author_message) >= 1) AND (char_length(author_message) <= 8000))),
    CONSTRAINT agent_runs_claim_generation_check CHECK ((claim_generation >= 0)),
    CONSTRAINT agent_runs_fence_token_check CHECK ((fence_token >= 0)),
    CONSTRAINT agent_runs_pkey PRIMARY KEY (owner_user_id, project_id, run_id),
    CONSTRAINT agent_runs_binding_registration_fkey FOREIGN KEY (owner_user_id, project_id, project_model_use_binding_revision, model_registration_revision) REFERENCES storyos.project_external_use_binding_revisions(owner_user_id, project_id, project_model_use_binding_revision, model_registration_revision) MATCH FULL,
    CONSTRAINT agent_runs_owner_user_id_project_id_conversation_id_memory_fkey FOREIGN KEY (owner_user_id, project_id, conversation_id, memory_settings_revision) REFERENCES storyos.conversation_memory_settings(owner_user_id, project_id, conversation_id, memory_settings_revision) MATCH FULL,
    CONSTRAINT agent_runs_owner_user_id_project_id_grant_id_fkey FOREIGN KEY (owner_user_id, project_id, grant_id) REFERENCES storyos.project_destination_grants(owner_user_id, project_id, grant_id) MATCH FULL,
    CONSTRAINT agent_runs_owner_user_id_project_id_project_model_use_bind_fkey FOREIGN KEY (owner_user_id, project_id, project_model_use_binding_revision) REFERENCES storyos.project_external_use_binding_revisions(owner_user_id, project_id, project_model_use_binding_revision) MATCH FULL,
    CONSTRAINT agent_runs_owner_user_id_project_id_receipt_id_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
    CONSTRAINT agent_runs_predecessor_fk FOREIGN KEY (owner_user_id, project_id, predecessor_run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id)
);

CREATE INDEX agent_runs_binding_idx ON storyos.agent_runs USING btree (owner_user_id, project_id, project_model_use_binding_revision, model_registration_revision);

CREATE INDEX agent_runs_grant_idx ON storyos.agent_runs USING btree (owner_user_id, project_id, grant_id);

CREATE UNIQUE INDEX agent_runs_one_live_conversation ON storyos.agent_runs USING btree (owner_user_id, project_id, conversation_id) WHERE (status = ANY (ARRAY['queued'::text, 'claimed'::text, 'waiting'::text, 'paused'::text]));

CREATE INDEX agent_runs_receipt_idx ON storyos.agent_runs USING btree (owner_user_id, project_id, receipt_id);

CREATE INDEX agent_runs_registration_idx ON storyos.agent_runs USING btree (model_registration_revision);

CREATE INDEX agent_runs_settings_idx ON storyos.agent_runs USING btree (owner_user_id, project_id, conversation_id, memory_settings_revision);

ALTER TABLE storyos.agent_runs ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.agent_runs FORCE ROW LEVEL SECURITY;

CREATE POLICY agent_runs_exact_scope ON storyos.agent_runs USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

CREATE POLICY agent_runs_worker_select ON storyos.agent_runs FOR SELECT USING (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL)));

CREATE POLICY agent_runs_worker_update ON storyos.agent_runs FOR UPDATE USING (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL))) WITH CHECK (((current_setting('storyos.scope_mode'::text, true) = 'worker'::text) AND (storyos.uuid_setting('storyos.project_id'::text) IS NULL) AND (storyos.uuid_setting('storyos.owner_user_id'::text) IS NULL)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.agent_runs TO storyos_runtime;

-- Context Assembly manifests and operation requirements

CREATE TABLE storyos.operation_requirements (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    operation_requirement_id uuid NOT NULL,
    run_id uuid NOT NULL,
    input_snapshot_id uuid NOT NULL,
    receipt_id uuid NOT NULL,
    payload jsonb NOT NULL,
    requirement_role text DEFAULT 'primary'::text NOT NULL,
    decision_position numeric(20,0) DEFAULT 0 NOT NULL,
    CONSTRAINT operation_requirements_decision_position_check CHECK (((decision_position >= (0)::numeric) AND (decision_position <= '18446744073709551615'::numeric))),
    CONSTRAINT operation_requirements_requirement_role_check CHECK ((requirement_role = ANY (ARRAY['primary'::text, 'compaction'::text, 'later_request'::text, 'retrieval'::text, 'successor'::text, 'tool_call'::text]))),
    CONSTRAINT operation_requirements_pkey PRIMARY KEY (owner_user_id, project_id, operation_requirement_id),
    CONSTRAINT operation_requirements_owner_user_id_project_id_receipt_id_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
    CONSTRAINT operation_requirements_owner_user_id_project_id_run_id_fkey FOREIGN KEY (owner_user_id, project_id, run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL
);

CREATE UNIQUE INDEX operation_requirements_one_role ON storyos.operation_requirements USING btree (owner_user_id, project_id, run_id, requirement_role, decision_position);

CREATE INDEX operation_requirements_receipt_idx ON storyos.operation_requirements USING btree (owner_user_id, project_id, receipt_id);

ALTER TABLE storyos.operation_requirements ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.operation_requirements FORCE ROW LEVEL SECURITY;

CREATE POLICY operation_requirements_exact_scope ON storyos.operation_requirements USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.operation_requirements TO storyos_runtime;

CREATE TABLE storyos.context_assembly_manifests (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    context_assembly_manifest_id uuid NOT NULL,
    operation_requirement_id uuid NOT NULL,
    run_id uuid NOT NULL,
    sufficiency text NOT NULL,
    destination_context_manifest_id uuid,
    outbound_disclosure_manifest_id uuid,
    payload jsonb NOT NULL,
    receipt_id uuid NOT NULL,
    manifest_role text DEFAULT 'decision'::text NOT NULL,
    decision_position numeric(20,0) DEFAULT 0 NOT NULL,
    disclosure_grant_id uuid,
    disclosure_grant_scope text,
    CONSTRAINT context_assembly_manifests_decision_position_check CHECK (((decision_position >= (0)::numeric) AND (decision_position <= '18446744073709551615'::numeric))),
    CONSTRAINT context_assembly_manifests_destination_pair CHECK (((((destination_context_manifest_id IS NULL) AND (outbound_disclosure_manifest_id IS NULL)) OR ((destination_context_manifest_id IS NOT NULL) AND (outbound_disclosure_manifest_id IS NOT NULL))) IS TRUE)),
    CONSTRAINT context_assembly_manifests_disclosure_grant_scope_check CHECK ((disclosure_grant_scope = ANY (ARRAY['project_writing_assistance_v1'::text, 'project_writing_assistance_v2'::text, 'project_writing_assistance_v3'::text, 'research_tool_v1'::text]))),
    CONSTRAINT context_assembly_manifests_disclosure_grant_shape CHECK (((disclosure_grant_id IS NULL) = (disclosure_grant_scope IS NULL))),
    CONSTRAINT context_assembly_manifests_manifest_role_check CHECK ((manifest_role = ANY (ARRAY['decision'::text, 'compaction'::text, 'later_request'::text, 'retrieval'::text, 'successor'::text, 'tool_call'::text]))),
    CONSTRAINT context_assembly_manifests_sufficiency_check CHECK ((sufficiency = ANY (ARRAY['complete'::text, 'blocked'::text]))),
    CONSTRAINT context_assembly_manifests_owner_user_id_project_id_operati_key UNIQUE (owner_user_id, project_id, operation_requirement_id),
    CONSTRAINT context_assembly_manifests_pkey PRIMARY KEY (owner_user_id, project_id, context_assembly_manifest_id),
    CONSTRAINT context_assembly_manifests_owner_user_id_project_id_operat_fkey FOREIGN KEY (owner_user_id, project_id, operation_requirement_id) REFERENCES storyos.operation_requirements(owner_user_id, project_id, operation_requirement_id) MATCH FULL,
    CONSTRAINT context_assembly_manifests_owner_user_id_project_id_receip_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
    CONSTRAINT context_assembly_manifests_owner_user_id_project_id_run_id_fkey FOREIGN KEY (owner_user_id, project_id, run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL
);

CREATE UNIQUE INDEX context_assembly_manifests_one_role ON storyos.context_assembly_manifests USING btree (owner_user_id, project_id, run_id, manifest_role, decision_position);

CREATE INDEX context_assembly_manifests_receipt_idx ON storyos.context_assembly_manifests USING btree (owner_user_id, project_id, receipt_id);

ALTER TABLE storyos.context_assembly_manifests ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.context_assembly_manifests FORCE ROW LEVEL SECURITY;

CREATE POLICY context_assembly_manifests_exact_scope ON storyos.context_assembly_manifests USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.context_assembly_manifests TO storyos_runtime;

-- Model attempts

CREATE TABLE storyos.model_attempts (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    run_id uuid NOT NULL,
    model_attempt_id uuid NOT NULL,
    destination_attempt_id uuid NOT NULL,
    outbound_disclosure_event_id uuid NOT NULL,
    destination_context_manifest_id uuid NOT NULL,
    outbound_disclosure_manifest_id uuid NOT NULL,
    wire_payload_projection_id uuid NOT NULL,
    model_invocation_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
    decision_id uuid,
    continuation_binding_id uuid,
    dispatch_state text NOT NULL,
    payload jsonb NOT NULL,
    attempt_role storyos.attempt_role DEFAULT 'decision'::text NOT NULL,
    decision_position numeric(20,0) DEFAULT 0 NOT NULL,
    CONSTRAINT model_attempts_decision_position_check CHECK (((decision_position >= (0)::numeric) AND (decision_position <= '18446744073709551615'::numeric))),
    CONSTRAINT model_attempts_dispatch_state_check CHECK ((dispatch_state = ANY (ARRAY['uncertain'::text, 'settled'::text]))),
    CONSTRAINT model_attempts_pkey PRIMARY KEY (owner_user_id, project_id, model_attempt_id),
    CONSTRAINT model_attempts_owner_user_id_project_id_conversation_id_fkey FOREIGN KEY (owner_user_id, project_id, conversation_id) REFERENCES storyos.project_conversations(owner_user_id, project_id, conversation_id) MATCH FULL,
    CONSTRAINT model_attempts_owner_user_id_project_id_run_id_fkey FOREIGN KEY (owner_user_id, project_id, run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL
);

CREATE UNIQUE INDEX model_attempts_one_role ON storyos.model_attempts USING btree (owner_user_id, project_id, run_id, attempt_role, decision_position);

ALTER TABLE storyos.model_attempts ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.model_attempts FORCE ROW LEVEL SECURITY;

CREATE POLICY model_attempts_exact_scope ON storyos.model_attempts USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.model_attempts TO storyos_runtime;

-- ToolCalls (ADR 0034, ADR 0084). One row for each member of the validated function request batch of
-- one durable Agent Decision, in item order. The identity is the Model Attempt and the native call
-- identifier, so a second delivery of the same observation creates no second row. The Tool Gateway
-- claims each row one time: `attempt` holds the Admission Decision, the Wire Payload Projection
-- without the key, the Destination Attempt, and the Outbound Disclosure Event. The result text is data.

CREATE TABLE storyos.tool_calls (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    tool_call_id uuid NOT NULL,
    run_id uuid NOT NULL,
    decision_position numeric(20,0) NOT NULL,
    model_attempt_id uuid NOT NULL,
    decision_id uuid NOT NULL,
    call_id text NOT NULL,
    call_position integer NOT NULL,
    caller_route text NOT NULL,
    function_name text NOT NULL,
    registration_id text NOT NULL,
    tool_registration_revision uuid NOT NULL,
    tool_spec_revision text NOT NULL,
    arguments jsonb NOT NULL,
    argument_digest text NOT NULL,
    state text NOT NULL,
    refusal text,
    destination_attempt_id uuid,
    attempt jsonb,
    effect_outcome jsonb,
    result_text text,
    result_digest text,
    result_bytes integer,
    CONSTRAINT tool_calls_pkey PRIMARY KEY (owner_user_id, project_id, tool_call_id),
    CONSTRAINT tool_calls_attempt_call_key UNIQUE (owner_user_id, project_id, model_attempt_id, call_id),
    CONSTRAINT tool_calls_attempt_position_key UNIQUE (owner_user_id, project_id, model_attempt_id, call_position),
    CONSTRAINT tool_calls_call_id_check CHECK (char_length(call_id) BETWEEN 1 AND 256),
    CONSTRAINT tool_calls_call_position_check CHECK ((call_position >= 0)),
    CONSTRAINT tool_calls_caller_route_check CHECK ((caller_route = 'model'::text)),
    CONSTRAINT tool_calls_decision_position_check CHECK (((decision_position >= (0)::numeric) AND (decision_position <= '18446744073709551615'::numeric))),
    CONSTRAINT tool_calls_digest_check CHECK (argument_digest ~ '^sha256:[0-9a-f]{64}$'::text),
    CONSTRAINT tool_calls_name_check CHECK (char_length(function_name) BETWEEN 1 AND 128 AND char_length(registration_id) BETWEEN 1 AND 128 AND char_length(tool_spec_revision) BETWEEN 1 AND 200),
    CONSTRAINT tool_calls_state_check CHECK ((state = ANY (ARRAY['pending'::text, 'dispatched'::text, 'settled'::text, 'failed'::text, 'unknown'::text, 'refused'::text, 'awaiting_approval'::text]))),
    CONSTRAINT tool_calls_refusal_shape CHECK (((state = ANY (ARRAY['refused'::text, 'awaiting_approval'::text])) = (refusal IS NOT NULL)) AND (char_length(refusal) BETWEEN 1 AND 64)),
    CONSTRAINT tool_calls_attempt_shape CHECK (((destination_attempt_id IS NULL) = (attempt IS NULL)) AND ((state = ANY (ARRAY['pending'::text, 'awaiting_approval'::text])) <= (attempt IS NULL)) AND ((state = ANY (ARRAY['dispatched'::text, 'settled'::text, 'failed'::text, 'unknown'::text])) <= (attempt IS NOT NULL)) AND (attempt IS NULL OR jsonb_typeof(attempt) = 'object'::text)),
    CONSTRAINT tool_calls_outcome_shape CHECK (((effect_outcome IS NULL) = ((state = ANY (ARRAY['pending'::text, 'dispatched'::text])) OR (attempt IS NULL))) AND (effect_outcome IS NULL OR jsonb_typeof(effect_outcome) = 'object'::text)),
    CONSTRAINT tool_calls_result_shape CHECK (((result_text IS NULL) = (result_digest IS NULL)) AND ((result_text IS NULL) = (result_bytes IS NULL)) AND (result_text IS NULL OR state = 'settled'::text) AND (result_digest ~ '^sha256:[0-9a-f]{64}$'::text) AND (result_bytes BETWEEN 0 AND 262144)),
    CONSTRAINT tool_calls_owner_user_id_project_id_run_id_fkey FOREIGN KEY (owner_user_id, project_id, run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL,
    CONSTRAINT tool_calls_owner_user_id_project_id_model_attempt_id_fkey FOREIGN KEY (owner_user_id, project_id, model_attempt_id) REFERENCES storyos.model_attempts(owner_user_id, project_id, model_attempt_id) MATCH FULL
);

CREATE INDEX tool_calls_run_state_idx ON storyos.tool_calls USING btree (owner_user_id, project_id, run_id, state);

ALTER TABLE storyos.tool_calls ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.tool_calls FORCE ROW LEVEL SECURITY;

CREATE POLICY tool_calls_exact_scope ON storyos.tool_calls USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.tool_calls TO storyos_runtime;

GRANT UPDATE(state, refusal, destination_attempt_id, attempt, effect_outcome, result_text, result_digest, result_bytes) ON TABLE storyos.tool_calls TO storyos_runtime;

-- Activity Run Events. An append-only record of the AgentRun: one `tool_call_settled` event for each
-- resolved ToolCall, and one `tool_registration_quarantined` event when a dispatch found Tool Contract
-- Drift. The payload holds identities, the outcome, and the byte count, and no result text.

CREATE TABLE storyos.agent_run_events (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    run_id uuid NOT NULL,
    event_id uuid NOT NULL,
    event_position bigint NOT NULL,
    event_kind text NOT NULL,
    payload jsonb NOT NULL,
    recorded_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT agent_run_events_pkey PRIMARY KEY (owner_user_id, project_id, event_id),
    CONSTRAINT agent_run_events_position_key UNIQUE (owner_user_id, project_id, run_id, event_position),
    CONSTRAINT agent_run_events_position_check CHECK ((event_position >= 0)),
    CONSTRAINT agent_run_events_kind_check CHECK ((event_kind = ANY (ARRAY['tool_call_settled'::text, 'tool_registration_quarantined'::text]))),
    CONSTRAINT agent_run_events_payload_check CHECK ((jsonb_typeof(payload) = 'object'::text)),
    CONSTRAINT agent_run_events_owner_user_id_project_id_run_id_fkey FOREIGN KEY (owner_user_id, project_id, run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL
);

ALTER TABLE storyos.agent_run_events ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.agent_run_events FORCE ROW LEVEL SECURITY;

CREATE POLICY agent_run_events_exact_scope ON storyos.agent_run_events USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.agent_run_events TO storyos_runtime;

-- Source Snapshots and Source Gaps. The settlement of a research ToolCall writes them in its own
-- transaction: one immutable Snapshot for each captured search entry or page, and one Gap for each
-- requested URL that the server reported as failed, empty, or missing. The revisions of one URL in a
-- Project form one lineage in capture order. Each Snapshot, and each Gap that a line of the result
-- gives, records its byte span (offset, length) in the exact result text of its ToolCall. The
-- runtime role can only read and insert them.

CREATE TABLE storyos.source_snapshots (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    source_snapshot_id uuid NOT NULL,
    run_id uuid NOT NULL,
    tool_call_id uuid NOT NULL,
    entry_position integer NOT NULL,
    kind text NOT NULL,
    locator text NOT NULL,
    title text,
    published text,
    text text NOT NULL,
    text_bytes integer NOT NULL,
    result_offset integer NOT NULL,
    content_digest text NOT NULL,
    captured_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT source_snapshots_pkey PRIMARY KEY (owner_user_id, project_id, source_snapshot_id),
    CONSTRAINT source_snapshots_entry_key UNIQUE (owner_user_id, project_id, tool_call_id, entry_position),
    CONSTRAINT source_snapshots_entry_position_check CHECK ((entry_position >= 0)),
    CONSTRAINT source_snapshots_kind_check CHECK ((kind = ANY (ARRAY['search_result'::text, 'page'::text]))),
    CONSTRAINT source_snapshots_locator_check CHECK ((char_length(locator) BETWEEN 1 AND 8192) AND (char_length(title) BETWEEN 1 AND 8192) AND (char_length(published) BETWEEN 1 AND 128)),
    CONSTRAINT source_snapshots_text_shape CHECK ((text_bytes = octet_length(text)) AND (text_bytes <= 262144) AND (result_offset BETWEEN 0 AND 262144 - text_bytes) AND (content_digest ~ '^sha256:[0-9a-f]{64}$'::text)),
    CONSTRAINT source_snapshots_owner_user_id_project_id_run_id_fkey FOREIGN KEY (owner_user_id, project_id, run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL,
    CONSTRAINT source_snapshots_owner_user_id_project_id_tool_call_id_fkey FOREIGN KEY (owner_user_id, project_id, tool_call_id) REFERENCES storyos.tool_calls(owner_user_id, project_id, tool_call_id) MATCH FULL
);

CREATE INDEX source_snapshots_lineage_idx ON storyos.source_snapshots USING btree (owner_user_id, project_id, locator, captured_at, source_snapshot_id);

CREATE INDEX source_snapshots_run_idx ON storyos.source_snapshots USING btree (owner_user_id, project_id, run_id);

ALTER TABLE storyos.source_snapshots ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.source_snapshots FORCE ROW LEVEL SECURITY;

CREATE POLICY source_snapshots_exact_scope ON storyos.source_snapshots USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.source_snapshots TO storyos_runtime;

CREATE TABLE storyos.source_gaps (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    source_gap_id uuid NOT NULL,
    run_id uuid NOT NULL,
    tool_call_id uuid NOT NULL,
    gap_position integer NOT NULL,
    locator text NOT NULL,
    reason text NOT NULL,
    result_offset integer,
    result_bytes integer,
    recorded_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT source_gaps_pkey PRIMARY KEY (owner_user_id, project_id, source_gap_id),
    CONSTRAINT source_gaps_position_key UNIQUE (owner_user_id, project_id, tool_call_id, gap_position),
    CONSTRAINT source_gaps_position_check CHECK ((gap_position >= 0)),
    CONSTRAINT source_gaps_locator_check CHECK ((char_length(locator) BETWEEN 1 AND 8192)),
    CONSTRAINT source_gaps_reason_check CHECK ((reason = ANY (ARRAY['failed'::text, 'empty'::text, 'missing'::text, 'unattributed'::text]))),
    CONSTRAINT source_gaps_result_span_check CHECK (((result_offset IS NULL) = (result_bytes IS NULL)) AND (result_offset >= 0) AND (result_bytes >= 0) AND (result_offset + result_bytes <= 262144)),
    CONSTRAINT source_gaps_owner_user_id_project_id_run_id_fkey FOREIGN KEY (owner_user_id, project_id, run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL,
    CONSTRAINT source_gaps_owner_user_id_project_id_tool_call_id_fkey FOREIGN KEY (owner_user_id, project_id, tool_call_id) REFERENCES storyos.tool_calls(owner_user_id, project_id, tool_call_id) MATCH FULL
);

CREATE INDEX source_gaps_run_idx ON storyos.source_gaps USING btree (owner_user_id, project_id, run_id);

ALTER TABLE storyos.source_gaps ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.source_gaps FORCE ROW LEVEL SECURITY;

CREATE POLICY source_gaps_exact_scope ON storyos.source_gaps USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.source_gaps TO storyos_runtime;

-- Proposals

CREATE TABLE storyos.proposals (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    kind text NOT NULL,
    chapter_id uuid NOT NULL,
    manuscript_block_id uuid NOT NULL,
    source_run_id uuid,
    source_decision_id uuid,
    bundle_policy text DEFAULT 'none'::text NOT NULL,
    source_lineage jsonb,
    CONSTRAINT proposal_source_lineage_shape CHECK (((source_lineage IS NULL) OR ((jsonb_typeof(source_lineage) = 'object'::text) AND ((source_lineage ->> 'kind'::text) = 'tool_result_continuation'::text) AND ((source_lineage ->> 'run_id'::text) = (source_run_id)::text)))),
    CONSTRAINT proposal_source_shape CHECK (((source_run_id IS NOT NULL) AND (source_decision_id IS NOT NULL))),
    CONSTRAINT proposals_bundle_policy_check CHECK ((bundle_policy = ANY (ARRAY['none'::text, 'atomic'::text]))),
    CONSTRAINT proposals_kind_check CHECK ((kind = ANY (ARRAY['block_edit'::text, 'reversal'::text]))),
    CONSTRAINT proposals_pkey PRIMARY KEY (owner_user_id, project_id, proposal_id),
    CONSTRAINT proposals_owner_user_id_project_id_chapter_id_fkey FOREIGN KEY (owner_user_id, project_id, chapter_id) REFERENCES storyos.manuscript_objects(owner_user_id, project_id, manuscript_object_id) MATCH FULL,
    CONSTRAINT proposals_owner_user_id_project_id_fkey FOREIGN KEY (owner_user_id, project_id) REFERENCES storyos.projects(owner_user_id, project_id) MATCH FULL,
    CONSTRAINT proposals_owner_user_id_project_id_manuscript_block_id_fkey FOREIGN KEY (owner_user_id, project_id, manuscript_block_id) REFERENCES storyos.manuscript_blocks(owner_user_id, project_id, manuscript_block_id) MATCH FULL,
    CONSTRAINT proposals_owner_user_id_project_id_source_run_id_fkey FOREIGN KEY (owner_user_id, project_id, source_run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id)
);

ALTER TABLE storyos.proposals ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposals FORCE ROW LEVEL SECURITY;

CREATE POLICY proposals_exact_scope ON storyos.proposals USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposals TO storyos_runtime;

GRANT UPDATE(bundle_policy) ON TABLE storyos.proposals TO storyos_runtime;

CREATE TABLE storyos.proposal_revisions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    generation text NOT NULL,
    validation text NOT NULL,
    closure text NOT NULL,
    candidate_text text NOT NULL,
    base_authoritative_revision_id uuid NOT NULL,
    parent_revision_id uuid,
    candidate_blocks jsonb,
    edited_operation_id uuid,
    prior_operation_candidate_text text,
    CONSTRAINT proposal_revisions_candidate_blocks_check CHECK (((candidate_blocks IS NULL) OR ((jsonb_typeof(candidate_blocks) = 'array'::text) AND (jsonb_array_length(candidate_blocks) > 0)))),
    CONSTRAINT proposal_revisions_closure_check CHECK ((closure = ANY (ARRAY['open'::text, 'withdrawn'::text, 'superseded'::text]))),
    CONSTRAINT proposal_revisions_edited_operation_evidence CHECK (((edited_operation_id IS NULL) = (prior_operation_candidate_text IS NULL))),
    CONSTRAINT proposal_revisions_generation_check CHECK ((generation = ANY (ARRAY['generating'::text, 'ready_partial'::text, 'ready'::text]))),
    CONSTRAINT proposal_revisions_validation_check CHECK ((validation = ANY (ARRAY['pending'::text, 'valid'::text, 'invalid'::text, 'conflicted'::text]))),
    CONSTRAINT proposal_revisions_pkey PRIMARY KEY (owner_user_id, project_id, proposal_id, revision_id),
    CONSTRAINT proposal_revisions_owner_user_id_project_id_base_authorita_fkey FOREIGN KEY (owner_user_id, project_id, base_authoritative_revision_id) REFERENCES storyos.authoritative_revisions(owner_user_id, project_id, revision_id) MATCH FULL,
    CONSTRAINT proposal_revisions_owner_user_id_project_id_proposal_id_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id) REFERENCES storyos.proposals(owner_user_id, project_id, proposal_id) MATCH FULL,
    CONSTRAINT proposal_revisions_parent_fk FOREIGN KEY (owner_user_id, project_id, proposal_id, parent_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id)
);

CREATE TABLE storyos.proposal_heads (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    current_revision_id uuid NOT NULL,
    CONSTRAINT proposal_heads_pkey PRIMARY KEY (owner_user_id, project_id, proposal_id),
    CONSTRAINT proposal_heads_owner_user_id_project_id_proposal_id_curren_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, current_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL
);

ALTER TABLE storyos.proposal_heads ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_heads FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_heads_exact_scope ON storyos.proposal_heads USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.proposal_heads TO storyos_runtime;

CREATE TABLE storyos.proposal_operations (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    manuscript_block_id uuid NOT NULL,
    resolution text NOT NULL,
    reservation_state text NOT NULL,
    candidate_text text NOT NULL,
    predecessor_operation_ids uuid[] DEFAULT '{}'::uuid[] NOT NULL,
    candidate_blocks jsonb,
    CONSTRAINT proposal_operations_reservation_state_check CHECK ((reservation_state = ANY (ARRAY['unresolved'::text, 'resolved'::text]))),
    CONSTRAINT proposal_operations_resolution_check CHECK ((resolution = ANY (ARRAY['pending'::text, 'applied'::text, 'rejected'::text]))),
    CONSTRAINT proposal_operations_pkey PRIMARY KEY (owner_user_id, project_id, proposal_id, operation_id),
    CONSTRAINT proposal_operations_owner_user_id_project_id_manuscript_bl_fkey FOREIGN KEY (owner_user_id, project_id, manuscript_block_id) REFERENCES storyos.manuscript_blocks(owner_user_id, project_id, manuscript_block_id) MATCH FULL,
    CONSTRAINT proposal_operations_owner_user_id_project_id_proposal_id_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id) REFERENCES storyos.proposals(owner_user_id, project_id, proposal_id) MATCH FULL
);

ALTER TABLE storyos.proposal_revisions
    ADD CONSTRAINT proposal_revisions_edited_operation_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, edited_operation_id) REFERENCES storyos.proposal_operations(owner_user_id, project_id, proposal_id, operation_id);

ALTER TABLE storyos.proposal_revisions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_revisions FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_revisions_exact_scope ON storyos.proposal_revisions USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.proposal_revisions TO storyos_runtime;

CREATE UNIQUE INDEX proposal_operations_one_unresolved_block ON storyos.proposal_operations USING btree (owner_user_id, project_id, manuscript_block_id) WHERE (reservation_state = 'unresolved'::text);

ALTER TABLE storyos.proposal_operations ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_operations FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_operations_exact_scope ON storyos.proposal_operations USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.proposal_operations TO storyos_runtime;

CREATE TABLE storyos.validation_receipts (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    validation_receipt_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    proposal_revision_id uuid NOT NULL,
    result text NOT NULL,
    base_authoritative_revision_id uuid NOT NULL,
    manuscript_block_id uuid NOT NULL,
    candidate_text text NOT NULL,
    reservation_state text NOT NULL,
    CONSTRAINT validation_receipts_reservation_state_check CHECK ((reservation_state = ANY (ARRAY['unresolved'::text, 'resolved'::text]))),
    CONSTRAINT validation_receipts_result_check CHECK ((result = ANY (ARRAY['valid'::text, 'invalid'::text, 'conflicted'::text]))),
    CONSTRAINT validation_receipts_owner_user_id_project_id_proposal_id_pr_key UNIQUE (owner_user_id, project_id, proposal_id, proposal_revision_id),
    CONSTRAINT validation_receipts_pkey PRIMARY KEY (owner_user_id, project_id, validation_receipt_id),
    CONSTRAINT validation_receipts_owner_user_id_project_id_proposal_id_p_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL
);

ALTER TABLE storyos.validation_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.validation_receipts FORCE ROW LEVEL SECURITY;

CREATE POLICY validation_receipts_exact_scope ON storyos.validation_receipts USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.validation_receipts TO storyos_runtime;

-- Acceptance Receipts

CREATE TABLE storyos.acceptance_receipts (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    acceptance_receipt_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    proposal_revision_id uuid NOT NULL,
    validation_receipt_id uuid NOT NULL,
    selected_operation_id uuid NOT NULL,
    result text NOT NULL,
    selected_operation_ids uuid[] NOT NULL,
    applied_authoritative_revision_id uuid,
    target_resolution text,
    CONSTRAINT acceptance_receipts_applied_target_recorded CHECK (((result = 'authoritative_applied'::text) = ((applied_authoritative_revision_id IS NOT NULL) AND (target_resolution IS NOT NULL)))),
    CONSTRAINT acceptance_receipts_result_check CHECK ((result = ANY (ARRAY['authoritative_applied'::text, 'invalid'::text, 'conflicted'::text, 'refused'::text]))),
    CONSTRAINT acceptance_receipts_selected_operation_ids_present CHECK ((cardinality(selected_operation_ids) >= 1)),
    CONSTRAINT acceptance_receipts_target_resolution_check CHECK ((target_resolution = ANY (ARRAY['exact_base'::text, 'block_replacement'::text, 'proven_range'::text, 'base_block_replacement'::text]))),
    CONSTRAINT acceptance_receipts_owner_user_id_project_id_acceptance_rec_key UNIQUE (owner_user_id, project_id, acceptance_receipt_id, proposal_id, proposal_revision_id, result),
    CONSTRAINT acceptance_receipts_pkey PRIMARY KEY (owner_user_id, project_id, acceptance_receipt_id),
    CONSTRAINT acceptance_receipts_owner_user_id_project_id_acceptance_re_fkey FOREIGN KEY (owner_user_id, project_id, acceptance_receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
    CONSTRAINT acceptance_receipts_owner_user_id_project_id_proposal_id_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id) REFERENCES storyos.proposals(owner_user_id, project_id, proposal_id) MATCH FULL
);

ALTER TABLE storyos.acceptance_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.acceptance_receipts FORCE ROW LEVEL SECURITY;

CREATE POLICY acceptance_receipts_exact_scope ON storyos.acceptance_receipts USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.acceptance_receipts TO storyos_runtime;

CREATE FUNCTION storyos.normalize_acceptance_receipt_selection() RETURNS trigger
    LANGUAGE plpgsql
    SET search_path TO 'pg_catalog'
    AS $$
BEGIN
  IF NEW.selected_operation_ids IS NULL
     OR cardinality(NEW.selected_operation_ids) = 0 THEN
    NEW.selected_operation_ids := ARRAY[NEW.selected_operation_id];
  ELSE
    NEW.selected_operation_id := NEW.selected_operation_ids[1];
  END IF;
  RETURN NEW;
END
$$;

GRANT ALL ON FUNCTION storyos.normalize_acceptance_receipt_selection() TO storyos_runtime;

CREATE TRIGGER acceptance_receipts_normalize_selection BEFORE INSERT OR UPDATE ON storyos.acceptance_receipts FOR EACH ROW EXECUTE FUNCTION storyos.normalize_acceptance_receipt_selection();

-- Proposal validation conditions

CREATE TABLE storyos.proposal_validation_conditions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    proposal_revision_id uuid NOT NULL,
    acceptance_receipt_id uuid NOT NULL,
    validation text NOT NULL,
    conflict_id uuid,
    condition_kind text,
    CONSTRAINT proposal_validation_conditions_check CHECK (((validation = 'conflicted'::text) = (conflict_id IS NOT NULL))),
    CONSTRAINT proposal_validation_conditions_check1 CHECK (((validation = 'conflicted'::text) = (condition_kind IS NOT NULL))),
    CONSTRAINT proposal_validation_conditions_condition_kind_check CHECK (((condition_kind IS NULL) OR (condition_kind = ANY (ARRAY['proposal_conflict'::text, 'proposal_recovery_conflict'::text])))),
    CONSTRAINT proposal_validation_conditions_validation_check CHECK ((validation = ANY (ARRAY['invalid'::text, 'conflicted'::text]))),
    CONSTRAINT proposal_validation_condition_owner_user_id_project_id_conf_key UNIQUE (owner_user_id, project_id, conflict_id),
    CONSTRAINT proposal_validation_conditions_pkey PRIMARY KEY (owner_user_id, project_id, proposal_id, proposal_revision_id),
    CONSTRAINT proposal_validation_condition_owner_user_id_project_id_acc_fkey FOREIGN KEY (owner_user_id, project_id, acceptance_receipt_id, proposal_id, proposal_revision_id, validation) REFERENCES storyos.acceptance_receipts(owner_user_id, project_id, acceptance_receipt_id, proposal_id, proposal_revision_id, result) MATCH FULL,
    CONSTRAINT proposal_validation_condition_owner_user_id_project_id_pro_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL
);

ALTER TABLE storyos.proposal_validation_conditions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_validation_conditions FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_validation_conditions_exact_scope ON storyos.proposal_validation_conditions USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_validation_conditions TO storyos_runtime;

-- Acceptance refusals

CREATE TABLE storyos.acceptance_refusals (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    refusal_id uuid NOT NULL,
    idempotency_key uuid NOT NULL,
    correlation_id uuid NOT NULL,
    command_kind text DEFAULT 'acceptProposal'::text NOT NULL,
    boundary text NOT NULL,
    command_schema text DEFAULT 'storyos.command.accept-proposal.request.v1'::text NOT NULL,
    refusal_profile_revision text DEFAULT 'storyos.acceptance-refusal.fixed-fields.v1'::text NOT NULL,
    reason text NOT NULL,
    client_contract_revision text NOT NULL,
    security_policy_revision text NOT NULL,
    limit_profile_revision text NOT NULL,
    challenge_rate_policy_revision text NOT NULL,
    recorded_at timestamp with time zone DEFAULT clock_timestamp() NOT NULL,
    CONSTRAINT acceptance_refusals_boundary_check CHECK ((boundary = ANY (ARRAY['challenge'::text, 'writer_session'::text]))),
    CONSTRAINT acceptance_refusals_challenge_rate_policy_revision_check CHECK ((challenge_rate_policy_revision = 'storyos.project-command-challenge-rate.fixed-window.v1'::text)),
    CONSTRAINT acceptance_refusals_client_contract_revision_check CHECK ((client_contract_revision = 'storyos.web-client.release-1.v3'::text)),
    CONSTRAINT acceptance_refusals_command_kind_check CHECK ((command_kind = 'acceptProposal'::text)),
    CONSTRAINT acceptance_refusals_command_schema_check CHECK ((command_schema = 'storyos.command.accept-proposal.request.v1'::text)),
    CONSTRAINT acceptance_refusals_limit_profile_revision_check CHECK ((limit_profile_revision = 'storyos.foundation.absolute.v1'::text)),
    CONSTRAINT acceptance_refusals_reason_check CHECK ((reason = ANY (ARRAY['stale_writer'::text, 'session_changed'::text, 'invalid_challenge'::text]))),
    CONSTRAINT acceptance_refusals_refusal_profile_revision_check CHECK ((refusal_profile_revision = 'storyos.acceptance-refusal.fixed-fields.v1'::text)),
    CONSTRAINT acceptance_refusals_security_policy_revision_check CHECK ((security_policy_revision = 'storyos.web-security-policy.release-1.v1'::text)),
    CONSTRAINT acceptance_refusals_owner_user_id_project_id_refusal_id_key UNIQUE (owner_user_id, project_id, refusal_id),
    CONSTRAINT acceptance_refusals_pkey PRIMARY KEY (owner_user_id, project_id, idempotency_key),
    CONSTRAINT acceptance_refusals_owner_user_id_project_id_command_kind__fkey FOREIGN KEY (owner_user_id, project_id, command_kind, idempotency_key) REFERENCES storyos.project_command_challenges(owner_user_id, project_id, command_kind, idempotency_key) MATCH FULL,
    CONSTRAINT acceptance_refusals_owner_user_id_project_id_proposal_id_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id) REFERENCES storyos.proposals(owner_user_id, project_id, proposal_id) MATCH FULL
);

CREATE INDEX acceptance_refusals_proposal_idx ON storyos.acceptance_refusals USING btree (owner_user_id, project_id, proposal_id, refusal_id DESC);

ALTER TABLE storyos.acceptance_refusals ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.acceptance_refusals FORCE ROW LEVEL SECURITY;

CREATE POLICY acceptance_refusals_exact_scope ON storyos.acceptance_refusals USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.acceptance_refusals TO storyos_runtime;

-- Proposal Operation rejection

CREATE TABLE storyos.proposal_rejection_receipts (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    rejection_receipt_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    proposal_revision_id uuid NOT NULL,
    selected_operation_id uuid NOT NULL,
    result text NOT NULL,
    rejection_reason text,
    rejection_note text,
    preserved_generation text,
    preserved_validation text,
    preserved_closure text,
    CONSTRAINT proposal_rejection_receipts_check CHECK (((result = 'proposal_operations_resolved'::text) = (rejection_reason = 'author_declined'::text))),
    CONSTRAINT proposal_rejection_receipts_check1 CHECK ((((preserved_generation IS NULL) AND (preserved_validation IS NULL) AND (preserved_closure IS NULL)) OR ((result = 'proposal_operations_resolved'::text) AND (preserved_generation IS NOT NULL) AND (preserved_validation IS NOT NULL) AND (preserved_closure IS NOT NULL) AND (preserved_generation = ANY (ARRAY['generating'::text, 'ready_partial'::text, 'ready'::text])) AND (preserved_validation = ANY (ARRAY['pending'::text, 'valid'::text, 'invalid'::text, 'conflicted'::text])) AND (preserved_closure = ANY (ARRAY['open'::text, 'withdrawn'::text, 'superseded'::text]))))),
    CONSTRAINT proposal_rejection_receipts_result_check CHECK ((result = ANY (ARRAY['proposal_operations_resolved'::text, 'conflicted'::text, 'refused'::text]))),
    CONSTRAINT proposal_rejection_receipts_pkey PRIMARY KEY (owner_user_id, project_id, rejection_receipt_id),
    CONSTRAINT proposal_rejection_receipts_owner_user_id_project_id_propo_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id) REFERENCES storyos.proposals(owner_user_id, project_id, proposal_id) MATCH FULL,
    CONSTRAINT proposal_rejection_receipts_owner_user_id_project_id_rejec_fkey FOREIGN KEY (owner_user_id, project_id, rejection_receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);

ALTER TABLE storyos.proposal_rejection_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_rejection_receipts FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_rejection_receipts_exact_scope ON storyos.proposal_rejection_receipts USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_rejection_receipts TO storyos_runtime;

CREATE TABLE storyos.proposal_operation_resolutions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    resolution_event_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    proposal_revision_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    prior_resolution text NOT NULL,
    resulting_resolution text NOT NULL,
    rejection_receipt_id uuid NOT NULL,
    author_action_sequence numeric(20,0) NOT NULL,
    CONSTRAINT proposal_operation_resolutions_author_action_sequence_check CHECK (((author_action_sequence >= (1)::numeric) AND (author_action_sequence <= '18446744073709551615'::numeric))),
    CONSTRAINT proposal_operation_resolutions_prior_resolution_check CHECK ((prior_resolution = 'pending'::text)),
    CONSTRAINT proposal_operation_resolutions_resulting_resolution_check CHECK ((resulting_resolution = 'rejected'::text)),
    CONSTRAINT proposal_operation_resolution_owner_user_id_project_id_reje_key UNIQUE (owner_user_id, project_id, rejection_receipt_id, operation_id),
    CONSTRAINT proposal_operation_resolutions_pkey PRIMARY KEY (owner_user_id, project_id, resolution_event_id),
    CONSTRAINT proposal_operation_resolution_owner_user_id_project_id_pro_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, operation_id) REFERENCES storyos.proposal_operations(owner_user_id, project_id, proposal_id, operation_id) MATCH FULL,
    CONSTRAINT proposal_operation_resolution_owner_user_id_project_id_rej_fkey FOREIGN KEY (owner_user_id, project_id, rejection_receipt_id) REFERENCES storyos.proposal_rejection_receipts(owner_user_id, project_id, rejection_receipt_id) MATCH FULL
);

ALTER TABLE storyos.proposal_operation_resolutions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_operation_resolutions FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_operation_resolutions_exact_scope ON storyos.proposal_operation_resolutions USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_operation_resolutions TO storyos_runtime;

-- Proposal Operation reopening

CREATE TABLE storyos.proposal_operation_reopenings (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    reopen_event_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    source_proposal_revision_id uuid NOT NULL,
    resulting_proposal_revision_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    rejection_event_id uuid NOT NULL,
    prior_resolution text NOT NULL,
    resulting_resolution text NOT NULL,
    reopen_receipt_id uuid NOT NULL,
    author_action_sequence numeric(20,0) NOT NULL,
    preserved_generation text,
    preserved_closure text,
    CONSTRAINT proposal_operation_reopenings_author_action_sequence_check CHECK (((author_action_sequence >= (1)::numeric) AND (author_action_sequence <= '18446744073709551615'::numeric))),
    CONSTRAINT proposal_operation_reopenings_check CHECK ((((preserved_generation IS NULL) AND (preserved_closure IS NULL)) OR ((preserved_generation IS NOT NULL) AND (preserved_closure IS NOT NULL) AND (preserved_generation = ANY (ARRAY['generating'::text, 'ready_partial'::text, 'ready'::text])) AND (preserved_closure = ANY (ARRAY['open'::text, 'withdrawn'::text, 'superseded'::text]))))),
    CONSTRAINT proposal_operation_reopenings_prior_resolution_check CHECK ((prior_resolution = 'rejected'::text)),
    CONSTRAINT proposal_operation_reopenings_resulting_resolution_check CHECK ((resulting_resolution = 'pending'::text)),
    CONSTRAINT proposal_operation_reopenings_owner_user_id_project_id_reje_key UNIQUE (owner_user_id, project_id, rejection_event_id),
    CONSTRAINT proposal_operation_reopenings_owner_user_id_project_id_reop_key UNIQUE (owner_user_id, project_id, reopen_receipt_id, operation_id),
    CONSTRAINT proposal_operation_reopenings_pkey PRIMARY KEY (owner_user_id, project_id, reopen_event_id),
    CONSTRAINT proposal_operation_reopening_owner_user_id_project_id_pro_fkey1 FOREIGN KEY (owner_user_id, project_id, proposal_id, source_proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
    CONSTRAINT proposal_operation_reopening_owner_user_id_project_id_pro_fkey2 FOREIGN KEY (owner_user_id, project_id, proposal_id, resulting_proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
    CONSTRAINT proposal_operation_reopenings_owner_user_id_project_id_pro_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, operation_id) REFERENCES storyos.proposal_operations(owner_user_id, project_id, proposal_id, operation_id) MATCH FULL,
    CONSTRAINT proposal_operation_reopenings_owner_user_id_project_id_rej_fkey FOREIGN KEY (owner_user_id, project_id, rejection_event_id) REFERENCES storyos.proposal_operation_resolutions(owner_user_id, project_id, resolution_event_id) MATCH FULL,
    CONSTRAINT proposal_operation_reopenings_owner_user_id_project_id_reo_fkey FOREIGN KEY (owner_user_id, project_id, reopen_receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);

ALTER TABLE storyos.proposal_operation_reopenings ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_operation_reopenings FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_operation_reopenings_exact_scope ON storyos.proposal_operation_reopenings USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_operation_reopenings TO storyos_runtime;

-- Proposal Generations, stream events, and input fences

CREATE TABLE storyos.proposal_generations (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    generation_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    last_applied_stream_seq bigint NOT NULL,
    run_id uuid,
    CONSTRAINT proposal_generations_last_applied_stream_seq_check CHECK ((last_applied_stream_seq >= 0)),
    CONSTRAINT proposal_generations_owner_user_id_project_id_proposal_id_g_key UNIQUE (owner_user_id, project_id, proposal_id, generation_id),
    CONSTRAINT proposal_generations_pkey PRIMARY KEY (owner_user_id, project_id, generation_id),
    CONSTRAINT proposal_generations_owner_user_id_project_id_proposal_id_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id) REFERENCES storyos.proposals(owner_user_id, project_id, proposal_id) MATCH FULL,
    CONSTRAINT proposal_generations_run_fk FOREIGN KEY (owner_user_id, project_id, run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id)
);

ALTER TABLE storyos.proposal_generations ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_generations FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_generations_exact_scope ON storyos.proposal_generations USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.proposal_generations TO storyos_runtime;

CREATE TABLE storyos.proposal_stream_events (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    generation_id uuid NOT NULL,
    stream_seq bigint NOT NULL,
    proposal_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    batch_digest text NOT NULL,
    candidate_text text NOT NULL,
    CONSTRAINT proposal_stream_events_stream_seq_check CHECK ((stream_seq >= 1)),
    CONSTRAINT proposal_stream_events_pkey PRIMARY KEY (owner_user_id, project_id, generation_id, stream_seq),
    CONSTRAINT proposal_stream_events_owner_user_id_project_id_generation_fkey FOREIGN KEY (owner_user_id, project_id, generation_id) REFERENCES storyos.proposal_generations(owner_user_id, project_id, generation_id) MATCH FULL,
    CONSTRAINT proposal_stream_events_owner_user_id_project_id_proposal_i_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL
);

ALTER TABLE storyos.proposal_stream_events ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_stream_events FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_stream_events_exact_scope ON storyos.proposal_stream_events USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_stream_events TO storyos_runtime;

CREATE TABLE storyos.editor_input_fences (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    editor_input_fence_id uuid NOT NULL,
    editor_session_id uuid NOT NULL,
    writer_generation bigint NOT NULL,
    local_intent_sequence bigint NOT NULL,
    proposal_id uuid NOT NULL,
    generation_id uuid NOT NULL,
    CONSTRAINT editor_input_fences_local_intent_sequence_check CHECK ((local_intent_sequence >= 1)),
    CONSTRAINT editor_input_fences_writer_generation_check CHECK ((writer_generation >= 1)),
    CONSTRAINT editor_input_fences_owner_user_id_project_id_generation_id_key UNIQUE (owner_user_id, project_id, generation_id),
    CONSTRAINT editor_input_fences_pkey PRIMARY KEY (owner_user_id, project_id, editor_input_fence_id),
    CONSTRAINT editor_input_fences_owner_user_id_project_id_generation_id_fkey FOREIGN KEY (owner_user_id, project_id, generation_id) REFERENCES storyos.proposal_generations(owner_user_id, project_id, generation_id) MATCH FULL,
    CONSTRAINT editor_input_fences_owner_user_id_project_id_proposal_id_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id) REFERENCES storyos.proposals(owner_user_id, project_id, proposal_id) MATCH FULL
);

ALTER TABLE storyos.editor_input_fences ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.editor_input_fences FORCE ROW LEVEL SECURITY;

CREATE POLICY editor_input_fences_exact_scope ON storyos.editor_input_fences USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.editor_input_fences TO storyos_runtime;

CREATE TABLE storyos.proposal_pause_fences (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_pause_fence_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    generation_id uuid NOT NULL,
    proposal_revision_id uuid NOT NULL,
    admitted_through_stream_seq bigint NOT NULL,
    editor_input_fence_id uuid NOT NULL,
    projection_checkpoint_id uuid NOT NULL,
    CONSTRAINT proposal_pause_fences_admitted_through_stream_seq_check CHECK ((admitted_through_stream_seq >= 0)),
    CONSTRAINT proposal_pause_fences_owner_user_id_project_id_generation_i_key UNIQUE (owner_user_id, project_id, generation_id),
    CONSTRAINT proposal_pause_fences_pkey PRIMARY KEY (owner_user_id, project_id, proposal_pause_fence_id),
    CONSTRAINT proposal_pause_fences_owner_user_id_project_id_editor_inpu_fkey FOREIGN KEY (owner_user_id, project_id, editor_input_fence_id) REFERENCES storyos.editor_input_fences(owner_user_id, project_id, editor_input_fence_id) MATCH FULL,
    CONSTRAINT proposal_pause_fences_owner_user_id_project_id_generation__fkey FOREIGN KEY (owner_user_id, project_id, generation_id) REFERENCES storyos.proposal_generations(owner_user_id, project_id, generation_id) MATCH FULL,
    CONSTRAINT proposal_pause_fences_owner_user_id_project_id_proposal_id_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL
);

ALTER TABLE storyos.proposal_pause_fences ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_pause_fences FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_pause_fences_exact_scope ON storyos.proposal_pause_fences USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_pause_fences TO storyos_runtime;

-- Proposal Generation heads and transitions

CREATE TABLE storyos.proposal_generation_heads (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    generation_id uuid NOT NULL,
    CONSTRAINT proposal_generation_heads_pkey PRIMARY KEY (owner_user_id, project_id, proposal_id),
    CONSTRAINT proposal_generation_heads_owner_user_id_project_id_generat_fkey FOREIGN KEY (owner_user_id, project_id, generation_id) REFERENCES storyos.proposal_generations(owner_user_id, project_id, generation_id) MATCH FULL,
    CONSTRAINT proposal_generation_heads_owner_user_id_project_id_proposa_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id) REFERENCES storyos.proposals(owner_user_id, project_id, proposal_id) MATCH FULL
);

ALTER TABLE storyos.proposal_generation_heads ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_generation_heads FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_generation_heads_exact_scope ON storyos.proposal_generation_heads USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.proposal_generation_heads TO storyos_runtime;

CREATE TABLE storyos.proposal_generation_transitions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    transition_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    proposal_revision_id uuid NOT NULL,
    prior_generation_id uuid NOT NULL,
    resulting_generation_id uuid NOT NULL,
    prior_generation_state text NOT NULL,
    resulting_generation_state text NOT NULL,
    prior_run_id uuid NOT NULL,
    resulting_run_id uuid NOT NULL,
    preserved_validation text NOT NULL,
    preserved_closure text NOT NULL,
    preserved_operation_resolution text NOT NULL,
    receipt_id uuid NOT NULL,
    author_action_sequence numeric(20,0) NOT NULL,
    CONSTRAINT proposal_generation_transition_resulting_generation_state_check CHECK ((resulting_generation_state = ANY (ARRAY['generating'::text, 'ready_partial'::text, 'ready'::text]))),
    CONSTRAINT proposal_generation_transitions_author_action_sequence_check CHECK (((author_action_sequence >= (1)::numeric) AND (author_action_sequence <= '18446744073709551615'::numeric))),
    CONSTRAINT proposal_generation_transitions_prior_generation_state_check CHECK ((prior_generation_state = ANY (ARRAY['generating'::text, 'ready_partial'::text, 'ready'::text]))),
    CONSTRAINT proposal_generation_transitions_pkey PRIMARY KEY (owner_user_id, project_id, transition_id),
    CONSTRAINT proposal_generation_transiti_owner_user_id_project_id_pri_fkey1 FOREIGN KEY (owner_user_id, project_id, prior_run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL,
    CONSTRAINT proposal_generation_transiti_owner_user_id_project_id_res_fkey1 FOREIGN KEY (owner_user_id, project_id, resulting_run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL,
    CONSTRAINT proposal_generation_transitio_owner_user_id_project_id_pri_fkey FOREIGN KEY (owner_user_id, project_id, prior_generation_id) REFERENCES storyos.proposal_generations(owner_user_id, project_id, generation_id) MATCH FULL,
    CONSTRAINT proposal_generation_transitio_owner_user_id_project_id_pro_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
    CONSTRAINT proposal_generation_transitio_owner_user_id_project_id_rec_fkey FOREIGN KEY (owner_user_id, project_id, receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
    CONSTRAINT proposal_generation_transitio_owner_user_id_project_id_res_fkey FOREIGN KEY (owner_user_id, project_id, resulting_generation_id) REFERENCES storyos.proposal_generations(owner_user_id, project_id, generation_id) MATCH FULL
);

ALTER TABLE storyos.proposal_generation_transitions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_generation_transitions FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_generation_transitions_exact_scope ON storyos.proposal_generation_transitions USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_generation_transitions TO storyos_runtime;

-- Proposal compensation identity

CREATE FUNCTION storyos.require_proposal_compensation_identity() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
DECLARE receipt storyos.domain_receipts;
BEGIN
  IF TG_TABLE_NAME='domain_receipts' THEN receipt:=NEW;
  ELSE SELECT * INTO STRICT receipt FROM storyos.domain_receipts AS r WHERE
    (r.owner_user_id,r.project_id,r.receipt_id)=(NEW.owner_user_id,NEW.project_id,NEW.receipt_id); END IF;
  IF receipt.command_kind='undoLatestAuthorAction' AND receipt.result_kind='authoritative_applied'
    AND EXISTS(SELECT 1 FROM storyos.author_action_entries AS a JOIN storyos.author_action_entries AS s
      ON (s.owner_user_id,s.project_id,s.author_action_sequence)=(a.owner_user_id,a.project_id,a.compensated_source_sequence)
      JOIN storyos.domain_receipts AS r ON (r.owner_user_id,r.project_id,r.receipt_id)=(s.owner_user_id,s.project_id,s.receipt_id)
      WHERE (a.owner_user_id,a.project_id,a.receipt_id)=(receipt.owner_user_id,receipt.project_id,receipt.receipt_id) AND r.result_kind='proposal_revised') THEN
    IF NOT EXISTS(SELECT 1 FROM storyos.author_action_entries AS a JOIN storyos.author_action_entries AS s
      ON (s.owner_user_id,s.project_id,s.author_action_sequence)=(a.owner_user_id,a.project_id,a.compensated_source_sequence)
      JOIN storyos.domain_receipts AS r ON (r.owner_user_id,r.project_id,r.receipt_id)=(s.owner_user_id,s.project_id,s.receipt_id)
      JOIN storyos.proposal_revisions AS edited ON (edited.owner_user_id,edited.project_id,edited.revision_id)=
        (r.owner_user_id,r.project_id,r.proposal_revision_ids[1])
      JOIN storyos.proposal_revisions AS parent ON (parent.owner_user_id,parent.project_id,parent.proposal_id)=
        (edited.owner_user_id,edited.project_id,edited.proposal_id)
      JOIN storyos.proposal_revisions AS restored ON (restored.owner_user_id,restored.project_id,restored.proposal_id,restored.parent_revision_id)=
        (parent.owner_user_id,parent.project_id,parent.proposal_id,parent.revision_id)
      WHERE (a.owner_user_id,a.project_id,a.receipt_id)=(receipt.owner_user_id,receipt.project_id,receipt.receipt_id)
        AND a.disposition='compensation' AND s.disposition='forward' AND r.result_kind='proposal_revised'
        AND receipt.result_payload->>'proposal_revision_id'=restored.revision_id::text
        AND receipt.result_payload->>'source_proposal_revision_id'=parent.revision_id::text)
    THEN RAISE EXCEPTION 'Incomplete Proposal compensation identity' USING ERRCODE='23514'; END IF;
  END IF;
  RETURN NULL;
END $$;

CREATE CONSTRAINT TRIGGER proposal_compensation_receipt_complete AFTER INSERT ON storyos.domain_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION storyos.require_proposal_compensation_identity();

-- Proposal replans

CREATE TABLE storyos.proposal_replans (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    replan_event_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    source_proposal_revision_id uuid NOT NULL,
    resulting_proposal_revision_id uuid NOT NULL,
    source_condition_kind text NOT NULL,
    source_condition_ref uuid NOT NULL,
    replan_receipt_id uuid NOT NULL,
    author_action_sequence numeric(20,0) NOT NULL,
    preserved_generation text,
    preserved_closure text,
    CONSTRAINT proposal_replans_author_action_sequence_check CHECK (((author_action_sequence >= (1)::numeric) AND (author_action_sequence <= '18446744073709551615'::numeric))),
    CONSTRAINT proposal_replans_check CHECK ((((preserved_generation IS NULL) AND (preserved_closure IS NULL)) OR ((preserved_generation IS NOT NULL) AND (preserved_closure IS NOT NULL) AND (preserved_generation = ANY (ARRAY['generating'::text, 'ready_partial'::text, 'ready'::text])) AND (preserved_closure = ANY (ARRAY['open'::text, 'withdrawn'::text, 'superseded'::text]))))),
    CONSTRAINT proposal_replans_source_condition_kind_check CHECK ((source_condition_kind = ANY (ARRAY['proposal_conflict'::text, 'proposal_recovery_conflict'::text]))),
    CONSTRAINT proposal_replans_owner_user_id_project_id_replan_receipt_id_key UNIQUE (owner_user_id, project_id, replan_receipt_id),
    CONSTRAINT proposal_replans_owner_user_id_project_id_source_condition__key UNIQUE (owner_user_id, project_id, source_condition_ref),
    CONSTRAINT proposal_replans_pkey PRIMARY KEY (owner_user_id, project_id, replan_event_id),
    CONSTRAINT proposal_replans_owner_user_id_project_id_proposal_id_resu_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, resulting_proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
    CONSTRAINT proposal_replans_owner_user_id_project_id_proposal_id_sour_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, source_proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
    CONSTRAINT proposal_replans_owner_user_id_project_id_replan_receipt_i_fkey FOREIGN KEY (owner_user_id, project_id, replan_receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);

ALTER TABLE storyos.proposal_replans ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_replans FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_replans_exact_scope ON storyos.proposal_replans USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_replans TO storyos_runtime;

-- Active context compactions

CREATE TABLE storyos.active_context_compactions (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    compaction_id uuid NOT NULL,
    run_id uuid NOT NULL,
    prior_model_attempt_id uuid NOT NULL,
    prior_manifest_id uuid NOT NULL,
    prior_run_step_id uuid NOT NULL,
    producer_model_attempt_id uuid NOT NULL,
    producer_manifest_id uuid NOT NULL,
    producer_invocation_id uuid NOT NULL,
    producer text NOT NULL,
    mapping_kind text NOT NULL,
    mapping_revision text NOT NULL,
    known_inputs jsonb NOT NULL,
    output_text text NOT NULL,
    usage_kind text NOT NULL,
    loss_facts jsonb NOT NULL,
    input_digest text NOT NULL,
    install_state text NOT NULL,
    refusal_reason text,
    installed_run_step_id uuid,
    installed_model_invocation_id uuid,
    installed_model_attempt_id uuid,
    preserved_item_ids jsonb NOT NULL,
    admission jsonb NOT NULL,
    CONSTRAINT active_context_compactions_check CHECK (((((install_state = 'installed'::text) AND (installed_run_step_id IS NOT NULL) AND (installed_model_invocation_id IS NOT NULL) AND (installed_model_attempt_id IS NOT NULL) AND (refusal_reason IS NULL)) OR ((install_state = 'staged'::text) AND (installed_run_step_id IS NULL) AND (installed_model_invocation_id IS NULL) AND (installed_model_attempt_id IS NULL) AND (refusal_reason IS NULL)) OR ((install_state = 'refused'::text) AND (installed_run_step_id IS NULL) AND (installed_model_invocation_id IS NULL) AND (installed_model_attempt_id IS NULL) AND (refusal_reason IS NOT NULL))) IS TRUE)),
    CONSTRAINT active_context_compactions_install_state_check CHECK ((install_state = ANY (ARRAY['staged'::text, 'installed'::text, 'refused'::text]))),
    CONSTRAINT active_context_compactions_mapping_kind_check CHECK ((mapping_kind = ANY (ARRAY['host_managed'::text, 'native'::text]))),
    CONSTRAINT active_context_compactions_owner_user_id_project_id_run_id_key UNIQUE (owner_user_id, project_id, run_id),
    CONSTRAINT active_context_compactions_pkey PRIMARY KEY (owner_user_id, project_id, compaction_id),
    CONSTRAINT active_context_compactions_owner_user_id_project_id_instal_fkey FOREIGN KEY (owner_user_id, project_id, installed_model_attempt_id) REFERENCES storyos.model_attempts(owner_user_id, project_id, model_attempt_id),
    CONSTRAINT active_context_compactions_owner_user_id_project_id_prior__fkey FOREIGN KEY (owner_user_id, project_id, prior_model_attempt_id) REFERENCES storyos.model_attempts(owner_user_id, project_id, model_attempt_id) MATCH FULL,
    CONSTRAINT active_context_compactions_owner_user_id_project_id_produc_fkey FOREIGN KEY (owner_user_id, project_id, producer_model_attempt_id) REFERENCES storyos.model_attempts(owner_user_id, project_id, model_attempt_id) MATCH FULL,
    CONSTRAINT active_context_compactions_owner_user_id_project_id_run_id_fkey FOREIGN KEY (owner_user_id, project_id, run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL
);

ALTER TABLE storyos.active_context_compactions ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.active_context_compactions FORCE ROW LEVEL SECURITY;

CREATE POLICY active_context_compactions_exact_scope ON storyos.active_context_compactions USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT,UPDATE ON TABLE storyos.active_context_compactions TO storyos_runtime;

-- Context reference recoveries

CREATE TABLE storyos.context_reference_recoveries (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    recovery_id uuid NOT NULL,
    conversation_id uuid NOT NULL,
    predecessor_run_id uuid NOT NULL,
    successor_run_id uuid NOT NULL,
    predecessor_model_attempt_id uuid NOT NULL,
    predecessor_continuation_binding_id uuid,
    disposition text NOT NULL,
    block_reason text,
    run_step_id uuid,
    model_invocation_id uuid,
    model_attempt_id uuid,
    assembly_manifest_id uuid,
    lossless_provider_reconstruction boolean NOT NULL,
    semantic_erasure boolean NOT NULL,
    opaque_reused boolean NOT NULL,
    covered_content_included boolean NOT NULL,
    predecessor_terminal boolean NOT NULL,
    CONSTRAINT context_reference_recoveries_check CHECK ((predecessor_run_id <> successor_run_id)),
    CONSTRAINT context_reference_recoveries_check1 CHECK ((((lossless_provider_reconstruction = false) AND (semantic_erasure = false) AND (opaque_reused = false) AND (((disposition = 'rebuilt'::text) AND (block_reason IS NULL) AND (run_step_id IS NOT NULL) AND (model_invocation_id IS NOT NULL) AND (model_attempt_id IS NOT NULL) AND (assembly_manifest_id IS NOT NULL) AND predecessor_terminal) OR ((disposition = 'blocked'::text) AND (block_reason IS NOT NULL) AND (run_step_id IS NULL) AND (model_invocation_id IS NULL) AND (model_attempt_id IS NULL) AND (assembly_manifest_id IS NULL) AND (covered_content_included = false)) OR ((disposition = 'unknown_create'::text) AND (block_reason IS NULL) AND (run_step_id IS NULL) AND (model_invocation_id IS NULL) AND (model_attempt_id IS NULL) AND (assembly_manifest_id IS NULL) AND (covered_content_included = false)))) IS TRUE)),
    CONSTRAINT context_reference_recoveries_disposition_check CHECK ((disposition = ANY (ARRAY['rebuilt'::text, 'blocked'::text, 'unknown_create'::text]))),
    CONSTRAINT context_reference_recoveries_owner_user_id_project_id_succe_key UNIQUE (owner_user_id, project_id, successor_run_id),
    CONSTRAINT context_reference_recoveries_pkey PRIMARY KEY (owner_user_id, project_id, recovery_id),
    CONSTRAINT context_reference_recoveries_owner_user_id_project_id_asse_fkey FOREIGN KEY (owner_user_id, project_id, assembly_manifest_id) REFERENCES storyos.context_assembly_manifests(owner_user_id, project_id, context_assembly_manifest_id),
    CONSTRAINT context_reference_recoveries_owner_user_id_project_id_conv_fkey FOREIGN KEY (owner_user_id, project_id, conversation_id) REFERENCES storyos.project_conversations(owner_user_id, project_id, conversation_id) MATCH FULL,
    CONSTRAINT context_reference_recoveries_owner_user_id_project_id_mode_fkey FOREIGN KEY (owner_user_id, project_id, model_attempt_id) REFERENCES storyos.model_attempts(owner_user_id, project_id, model_attempt_id),
    CONSTRAINT context_reference_recoveries_owner_user_id_project_id_pre_fkey1 FOREIGN KEY (owner_user_id, project_id, predecessor_model_attempt_id) REFERENCES storyos.model_attempts(owner_user_id, project_id, model_attempt_id) MATCH FULL,
    CONSTRAINT context_reference_recoveries_owner_user_id_project_id_pred_fkey FOREIGN KEY (owner_user_id, project_id, predecessor_run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL,
    CONSTRAINT context_reference_recoveries_owner_user_id_project_id_succ_fkey FOREIGN KEY (owner_user_id, project_id, successor_run_id) REFERENCES storyos.agent_runs(owner_user_id, project_id, run_id) MATCH FULL
);

ALTER TABLE storyos.context_reference_recoveries ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.context_reference_recoveries FORCE ROW LEVEL SECURITY;

CREATE POLICY context_reference_recoveries_exact_scope ON storyos.context_reference_recoveries USING (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid))) WITH CHECK (((owner_user_id = (current_setting('storyos.owner_user_id'::text))::uuid) AND (project_id = (current_setting('storyos.project_id'::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.context_reference_recoveries TO storyos_runtime;

-- Proposal withdrawals

CREATE TABLE storyos.proposal_withdrawals (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    withdrawal_event_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    proposal_revision_id uuid NOT NULL,
    withdrawal_reason text NOT NULL,
    author_note text,
    withdrawal_receipt_id uuid NOT NULL,
    author_action_sequence numeric(20,0),
    preserved_generation text,
    preserved_validation text,
    CONSTRAINT proposal_withdrawals_author_action_sequence_check CHECK (((author_action_sequence IS NULL) OR ((author_action_sequence >= (1)::numeric) AND (author_action_sequence <= '18446744073709551615'::numeric)))),
    CONSTRAINT proposal_withdrawals_check CHECK (((withdrawal_reason = 'author_withdrew'::text) = (author_action_sequence IS NOT NULL))),
    CONSTRAINT proposal_withdrawals_check1 CHECK (((author_note IS NULL) OR (withdrawal_reason = 'author_withdrew'::text))),
    CONSTRAINT proposal_withdrawals_check2 CHECK ((((preserved_generation IS NULL) AND (preserved_validation IS NULL)) OR ((preserved_generation IS NOT NULL) AND (preserved_validation IS NOT NULL) AND (preserved_generation = ANY (ARRAY['generating'::text, 'ready_partial'::text, 'ready'::text])) AND (preserved_validation = ANY (ARRAY['pending'::text, 'valid'::text, 'invalid'::text, 'conflicted'::text]))))),
    CONSTRAINT proposal_withdrawals_withdrawal_reason_check CHECK ((withdrawal_reason = ANY (ARRAY['author_withdrew'::text, 'current_producer_withdrew'::text]))),
    CONSTRAINT proposal_withdrawals_owner_user_id_project_id_withdrawal_re_key UNIQUE (owner_user_id, project_id, withdrawal_receipt_id),
    CONSTRAINT proposal_withdrawals_pkey PRIMARY KEY (owner_user_id, project_id, withdrawal_event_id),
    CONSTRAINT proposal_withdrawals_owner_user_id_project_id_author_actio_fkey FOREIGN KEY (owner_user_id, project_id, author_action_sequence) REFERENCES storyos.author_action_entries(owner_user_id, project_id, author_action_sequence),
    CONSTRAINT proposal_withdrawals_owner_user_id_project_id_proposal_id__fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
    CONSTRAINT proposal_withdrawals_owner_user_id_project_id_withdrawal_r_fkey FOREIGN KEY (owner_user_id, project_id, withdrawal_receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL
);

ALTER TABLE storyos.proposal_withdrawals ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_withdrawals FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_withdrawals_exact_scope ON storyos.proposal_withdrawals USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_withdrawals TO storyos_runtime;

-- Undo Acceptance Receipts

CREATE TABLE storyos.undo_acceptance_receipts (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    undo_acceptance_receipt_id uuid NOT NULL,
    author_undo_receipt_id uuid NOT NULL,
    acceptance_receipt_id uuid NOT NULL,
    source_author_action_sequence numeric(20,0) NOT NULL,
    outcome text NOT NULL,
    authoritative_commit_id uuid,
    proposal_id uuid,
    proposal_revision_id uuid,
    reason text,
    reported_activity_position numeric(20,0) NOT NULL,
    CONSTRAINT undo_acceptance_receipts_check CHECK ((((outcome = 'compensated'::text) AND (authoritative_commit_id IS NOT NULL) AND (reason IS NULL) AND ((proposal_id IS NULL) = (proposal_revision_id IS NULL))) OR ((outcome = 'reversal_required'::text) AND (authoritative_commit_id IS NULL) AND (reason IS NULL) AND (proposal_id IS NOT NULL) AND (proposal_revision_id IS NOT NULL)) OR ((outcome = 'unavailable'::text) AND (authoritative_commit_id IS NULL) AND (proposal_id IS NULL) AND (proposal_revision_id IS NULL) AND (reason IS NOT NULL)))),
    CONSTRAINT undo_acceptance_receipts_outcome_check CHECK ((outcome = ANY (ARRAY['compensated'::text, 'reversal_required'::text, 'unavailable'::text]))),
    CONSTRAINT undo_acceptance_receipts_owner_user_id_project_id_author_un_key UNIQUE (owner_user_id, project_id, author_undo_receipt_id),
    CONSTRAINT undo_acceptance_receipts_pkey PRIMARY KEY (owner_user_id, project_id, undo_acceptance_receipt_id),
    CONSTRAINT undo_acceptance_receipts_owner_user_id_project_id_acceptan_fkey FOREIGN KEY (owner_user_id, project_id, acceptance_receipt_id) REFERENCES storyos.acceptance_receipts(owner_user_id, project_id, acceptance_receipt_id),
    CONSTRAINT undo_acceptance_receipts_owner_user_id_project_id_author_u_fkey FOREIGN KEY (owner_user_id, project_id, author_undo_receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT undo_acceptance_receipts_owner_user_id_project_id_proposal_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) DEFERRABLE INITIALLY DEFERRED
);

ALTER TABLE storyos.undo_acceptance_receipts ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.undo_acceptance_receipts FORCE ROW LEVEL SECURITY;

CREATE POLICY undo_acceptance_receipts_exact_scope ON storyos.undo_acceptance_receipts USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.undo_acceptance_receipts TO storyos_runtime;

CREATE FUNCTION storyos.keep_undo_acceptance_receipt_immutable() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
  RAISE EXCEPTION 'Undo Acceptance Receipts are immutable' USING ERRCODE = '23514';
END;
$$;

CREATE TRIGGER undo_acceptance_receipt_immutable BEFORE DELETE OR UPDATE ON storyos.undo_acceptance_receipts FOR EACH ROW EXECUTE FUNCTION storyos.keep_undo_acceptance_receipt_immutable();

-- Proposal withdrawal reopenings

CREATE TABLE storyos.proposal_withdrawal_reopenings (
    owner_user_id uuid NOT NULL,
    project_id uuid NOT NULL,
    reopen_event_id uuid NOT NULL,
    proposal_id uuid NOT NULL,
    source_proposal_revision_id uuid NOT NULL,
    resulting_proposal_revision_id uuid NOT NULL,
    withdrawal_event_id uuid NOT NULL,
    preserved_generation text,
    preserved_operation_resolution text,
    reopen_receipt_id uuid NOT NULL,
    author_action_sequence numeric(20,0) NOT NULL,
    CONSTRAINT proposal_withdrawal_reopenings_author_action_sequence_check CHECK (((author_action_sequence >= (1)::numeric) AND (author_action_sequence <= '18446744073709551615'::numeric))),
    CONSTRAINT proposal_withdrawal_reopenings_check CHECK ((((preserved_generation IS NULL) AND (preserved_operation_resolution IS NULL)) OR ((preserved_generation IS NOT NULL) AND (preserved_operation_resolution IS NOT NULL) AND (preserved_generation = ANY (ARRAY['generating'::text, 'ready_partial'::text, 'ready'::text])) AND (preserved_operation_resolution = ANY (ARRAY['pending'::text, 'applied'::text, 'rejected'::text]))))),
    CONSTRAINT proposal_withdrawal_reopening_owner_user_id_project_id_reop_key UNIQUE (owner_user_id, project_id, reopen_receipt_id),
    CONSTRAINT proposal_withdrawal_reopenings_pkey PRIMARY KEY (owner_user_id, project_id, reopen_event_id),
    CONSTRAINT proposal_withdrawal_reopenin_owner_user_id_project_id_pro_fkey1 FOREIGN KEY (owner_user_id, project_id, proposal_id, resulting_proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
    CONSTRAINT proposal_withdrawal_reopening_owner_user_id_project_id_pro_fkey FOREIGN KEY (owner_user_id, project_id, proposal_id, source_proposal_revision_id) REFERENCES storyos.proposal_revisions(owner_user_id, project_id, proposal_id, revision_id) MATCH FULL,
    CONSTRAINT proposal_withdrawal_reopening_owner_user_id_project_id_reo_fkey FOREIGN KEY (owner_user_id, project_id, reopen_receipt_id) REFERENCES storyos.domain_receipts(owner_user_id, project_id, receipt_id) MATCH FULL,
    CONSTRAINT proposal_withdrawal_reopening_owner_user_id_project_id_wit_fkey FOREIGN KEY (owner_user_id, project_id, withdrawal_event_id) REFERENCES storyos.proposal_withdrawals(owner_user_id, project_id, withdrawal_event_id) MATCH FULL
);

ALTER TABLE storyos.proposal_withdrawal_reopenings ENABLE ROW LEVEL SECURITY;

ALTER TABLE storyos.proposal_withdrawal_reopenings FORCE ROW LEVEL SECURITY;

CREATE POLICY proposal_withdrawal_reopenings_exact_scope ON storyos.proposal_withdrawal_reopenings USING (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid))) WITH CHECK (((owner_user_id = (NULLIF(current_setting('storyos.owner_user_id'::text, true), ''::text))::uuid) AND (project_id = (NULLIF(current_setting('storyos.project_id'::text, true), ''::text))::uuid)));

GRANT SELECT,INSERT ON TABLE storyos.proposal_withdrawal_reopenings TO storyos_runtime;
