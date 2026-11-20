//! The shared test support from `model_gateway_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_agent_run::CREATE_AGENT_RUN;
use crate::shared_tests::command_sequence_project_session::UPDATE_PROJECT_ASSISTANCE;
use crate::shared_tests::command_sequence_structure::{titled_chapter, titled_volume};
use crate::shared_tests::command_sequence_support::{command_issue, seed_project};
use crate::shared_tests::json_row::json_of;
use serde_json::Value;
use storyos_application::{
    ClaimedAgentRun, ConversationSelection, CreateAgentRunInput, GatewayClock, ProjectScope,
    UpdateProjectAssistanceInput, issue_project_command_challenge,
};
use storyos_contracts::CommandKind;
use storyos_core::{
    AssistanceAvailability, DeploymentDestination, ResearchToolOffer, SelectionReference,
    SkillPackageReference, SkillSet,
};
use tokio_postgres::Client;

pub(crate) const AVAILABLE_BYTES: &[u8] = br#"{"availability":"available"}"#;

pub(crate) const RUN_BYTES: &[u8] = br#"{"conversation":{"kind":"new"}}"#;

/// The durable dispatch evidence of one AgentRun.
#[derive(Debug, PartialEq)]
pub(crate) struct DispatchEvidence {
    pub(crate) status: String,
    pub(crate) settlement: Option<String>,
    pub(crate) model_attempts: i64,
    pub(crate) disclosure_events: i64,
    pub(crate) destination_manifests: i64,
    pub(crate) items: Option<String>,
    pub(crate) decision_id: Option<String>,
}

/// The embedding destination of a process with no embedding adapter. Each request refuses before
/// the claim.
pub(crate) struct NoEmbeddings;

impl storyos_application::EmbeddingProviderAdapter for NoEmbeddings {
    const REGISTRATIONS: &'static [&'static storyos_core::EmbeddingRegistration] = &[];
    type Prepared = std::convert::Infallible;

    async fn prepare(
        &self,
        _registration: &'static storyos_core::EmbeddingRegistration,
        _route: &storyos_application::EmbeddingRoute,
        _input: &storyos_core::EmbeddingInput,
    ) -> Result<
        storyos_application::PreparedEmbeddingRequest<Self::Prepared>,
        storyos_application::PreDispatchRefusal,
    > {
        Err(storyos_application::PreDispatchRefusal::CredentialUnavailable)
    }

    async fn exchange(
        &self,
        prepared: Self::Prepared,
    ) -> storyos_application::EmbeddingObservation {
        match prepared {}
    }
}

/// The runtime timer, as the Worker supplies it.
pub(crate) struct RuntimeClock;

impl GatewayClock for RuntimeClock {
    async fn wait(&self, delay: std::time::Duration) {
        tokio::time::sleep(delay).await;
    }
}

pub(crate) fn digest(kind: &str, bytes: &[u8]) -> String {
    format!(
        "sha256:storyos.command.{kind}.jcs.v1:{}",
        crate::author_edit::sha256_hex(bytes)
    )
}

/// Claims one queued AgentRun with a known fence and an expired lease.
pub(crate) async fn claim_run(
    admin: &Client,
    scope: ProjectScope,
    run_id: String,
) -> ClaimedAgentRun {
    let fence_token = admin
        .query_one(
            "UPDATE storyos.agent_runs
                SET claim_generation = claim_generation + 1,
                    fence_token = claim_generation + 1,
                    lease_expires_at = clock_timestamp(),
                    status = 'claimed'
              WHERE run_id = $1::text::uuid
          RETURNING fence_token",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    ClaimedAgentRun {
        project_scope: scope,
        run_id,
        fence_token,
    }
}

/// Admits one queued AgentRun on a new chapter of a new Project with fake assistance, in a
/// deployment that offers `destination` and no research destination.
pub(crate) async fn queued_run(
    store: &PostgresProjectReader,
    prefix: &str,
    destination: DeploymentDestination,
) -> (ProjectScope, String, String) {
    queued_run_offering(store, prefix, destination, None).await
}

/// Admits one queued AgentRun as `queued_run` does, in a deployment that also offers `research`.
pub(crate) async fn queued_run_offering(
    store: &PostgresProjectReader,
    prefix: &str,
    destination: DeploymentDestination,
    research: Option<&'static ResearchToolOffer>,
) -> (ProjectScope, String, String) {
    let (scope, chapter_id) = assisted_chapter(store, prefix).await;
    let run_id = admit_run_in(
        store,
        &scope,
        &chapter_id,
        prefix,
        destination,
        "Help with this passage.",
        ConversationSelection::New,
        research,
    )
    .await;
    (scope, chapter_id, run_id)
}

/// A Project of `prefix` with the available assistance on `host_fake_tool_loop`, Volume A, and
/// Chapter A, and no Run. `prefix` gives the identities of the commands.
pub(crate) async fn assisted_chapter(
    store: &PostgresProjectReader,
    prefix: &str,
) -> (ProjectScope, String) {
    let scope: ProjectScope = seed_project(store, &format!("{prefix}0")).await;
    let assistance = command_issue(
        &scope,
        &format!("{prefix}1"),
        &UPDATE_PROJECT_ASSISTANCE,
        &digest(
            CommandKind::UpdateProjectAssistance.as_str(),
            AVAILABLE_BYTES,
        ),
    );
    issue_project_command_challenge(store, &assistance)
        .await
        .unwrap();
    let assistance_call = crate::shared_tests::command_sequence_support::command_call(
        assistance.binding.clone(),
        &assistance.nonce_digest,
        &format!("{prefix}2"),
        AVAILABLE_BYTES,
        UpdateProjectAssistanceInput {
            availability: AssistanceAvailability::Available,
            expected_revision: 0,
            destination: Some(DeploymentDestination::HostFakeToolLoop),
        },
    );
    store
        .update_project_assistance(&assistance_call.envelope, &assistance_call.input)
        .await
        .unwrap();
    let volume_id = titled_volume(
        store,
        &scope,
        &format!("{prefix}3"),
        "Volume A",
        /*expected_tree_revision*/ 1,
    )
    .await;
    let chapter_id = titled_chapter(
        store,
        &scope,
        &format!("{prefix}4"),
        &volume_id,
        "Chapter A",
        /*expected_tree_revision*/ 2,
    )
    .await;
    (scope, chapter_id)
}

/// Admits one queued AgentRun of a new conversation on `chapter_id`, in a deployment that offers
/// `destination`. `prefix` gives the identities of the command and the Run.
pub(crate) async fn admit_run(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    destination: DeploymentDestination,
    author_message: &str,
) -> String {
    let conversation = ConversationSelection::New;
    admit_run_in(
        store,
        scope,
        chapter_id,
        prefix,
        destination,
        author_message,
        conversation,
        None,
    )
    .await
}

/// Admits one queued AgentRun of `conversation`, as `admit_run` does, in a deployment that also
/// offers `research`.
#[expect(
    clippy::too_many_arguments,
    reason = "each input of one createAgentRun is a test input"
)]
pub(crate) async fn admit_run_in(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    destination: DeploymentDestination,
    author_message: &str,
    conversation: ConversationSelection,
    research: Option<&'static ResearchToolOffer>,
) -> String {
    let input = (conversation, research, None, None);
    admit_with(
        store,
        scope,
        chapter_id,
        prefix,
        destination,
        author_message,
        input,
        Default::default(),
    )
    .await
}

/// Admits one queued AgentRun of a new conversation of the tool loop profile, as `admit_run`
/// does, in a deployment with the embedding configuration `embedding`.
pub(crate) async fn admit_embedding_run(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    embedding: Option<&'static storyos_core::EmbeddingRegistration>,
) -> String {
    admit_with(
        store,
        scope,
        chapter_id,
        prefix,
        DeploymentDestination::HostFakeToolLoop,
        "Help with this passage.",
        (ConversationSelection::New, None, None, embedding),
        Default::default(),
    )
    .await
}

/// Admits one queued AgentRun of a new conversation, as `admit_run` does, with the Selection
/// References `references`.
pub(crate) async fn admit_referenced_run(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    destination: DeploymentDestination,
    author_message: &str,
    references: Vec<SelectionReference>,
) -> String {
    let input = (ConversationSelection::New, None, Some(references), None);
    admit_with(
        store,
        scope,
        chapter_id,
        prefix,
        destination,
        author_message,
        input,
        Default::default(),
    )
    .await
}

/// Admits one queued AgentRun of `conversation` of the tool loop profile, as `admit_run` does,
/// in a deployment that offers `skills`, with the Skill selections `selections`.
pub(crate) async fn admit_selected_run(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    author_message: &str,
    conversation: ConversationSelection,
    skills: (std::sync::Arc<SkillSet>, Vec<SkillPackageReference>),
) -> String {
    admit_with(
        store,
        scope,
        chapter_id,
        prefix,
        DeploymentDestination::HostFakeToolLoop,
        author_message,
        (conversation, None, None, None),
        skills,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "each input of one createAgentRun is a test input"
)]
pub(crate) async fn admit_with(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    destination: DeploymentDestination,
    author_message: &str,
    conversation: (
        ConversationSelection,
        Option<&'static ResearchToolOffer>,
        Option<Vec<SelectionReference>>,
        Option<&'static storyos_core::EmbeddingRegistration>,
    ),
    skills: (std::sync::Arc<SkillSet>, Vec<SkillPackageReference>),
) -> String {
    let run_id = format!("018f0000-0000-7001-8000-00000004{prefix}6");
    let inputs = (destination, author_message, run_id);
    admit_with_run_id(
        store,
        scope,
        chapter_id,
        prefix,
        inputs,
        conversation,
        skills,
    )
    .await
}

/// Admits one queued AgentRun of a new Conversation of the tool loop profile with the identity
/// `run_id`, as `admit_run` does.
pub(crate) async fn admit_run_with_id(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    author_message: &str,
    run_id: String,
) -> String {
    let inputs = (
        DeploymentDestination::HostFakeToolLoop,
        author_message,
        run_id,
    );
    let conversation = (ConversationSelection::New, None, None, None);
    admit_with_run_id(
        store,
        scope,
        chapter_id,
        prefix,
        inputs,
        conversation,
        Default::default(),
    )
    .await
}

pub(crate) async fn admit_with_run_id(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    chapter_id: &str,
    prefix: &str,
    (destination, author_message, run_id): (DeploymentDestination, &str, String),
    (conversation, research, selection_references, embedding): (
        ConversationSelection,
        Option<&'static ResearchToolOffer>,
        Option<Vec<SelectionReference>>,
        Option<&'static storyos_core::EmbeddingRegistration>,
    ),
    (skills, skill_selections): (std::sync::Arc<SkillSet>, Vec<SkillPackageReference>),
) -> String {
    let run = command_issue(
        scope,
        &format!("{prefix}5"),
        &CREATE_AGENT_RUN,
        &digest(CommandKind::CreateAgentRun.as_str(), RUN_BYTES),
    );
    issue_project_command_challenge(store, &run).await.unwrap();
    let call = crate::shared_tests::command_sequence_support::command_call(
        run.binding.clone(),
        &run.nonce_digest,
        &format!("{prefix}6"),
        RUN_BYTES,
        CreateAgentRunInput {
            passage_targets: None,
            selection_references,
            conversation,
            author_message: author_message.to_owned(),
            chapter_id: chapter_id.to_owned(),
            run_id,
            conversation_id: format!("018f0000-0000-7001-8000-00000006{prefix}6"),
            project_agent_id: format!("018f0000-0000-7001-8000-00000005{prefix}6"),
            destination: Some(destination),
            research,
            embedding,
            skills,
            skill_selections,
        },
    );
    store
        .create_agent_run(&call.envelope, &call.input)
        .await
        .unwrap();
    call.input.run_id
}

pub(crate) async fn dispatch_evidence(admin: &Client, run_id: &str) -> DispatchEvidence {
    let row = admin
        .query_one(
            "SELECT run.status, run.settlement::text,
                    (SELECT count(*) FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id),
                    (SELECT count(outbound_disclosure_event_id) FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id),
                    (SELECT count(destination_context_manifest_id)
                       FROM storyos.context_assembly_manifests AS assembly
                      WHERE assembly.run_id = run.run_id),
                    (SELECT attempt.payload->>'items' FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
                        AND attempt.decision_position = 0),
                    (SELECT attempt.decision_id::text FROM storyos.model_attempts AS attempt
                      WHERE attempt.run_id = run.run_id AND attempt.attempt_role = 'decision'
                        AND attempt.decision_position = 0)
               FROM storyos.agent_runs AS run
              WHERE run.run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap();
    DispatchEvidence {
        status: row.get(0),
        settlement: row.get(1),
        model_attempts: row.get(2),
        disclosure_events: row.get(3),
        destination_manifests: row.get(4),
        items: row.get(5),
        decision_id: row.get(6),
    }
}

pub(crate) async fn conversation(admin: &Client, run_id: &str) -> String {
    admin
        .query_one(
            "SELECT conversation_id::text FROM storyos.agent_runs WHERE run_id = $1::text::uuid",
            &[&run_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

/// Whether [`sent_context`] gives the replaced input label of each request.
#[derive(Clone, Copy)]
pub(crate) enum ReplacedInputLabel {
    Include,
    Omit,
}

/// For each Model Attempt of the Run in the order of its creation: its role, the position of its
/// RunStep, for each source class of `classes` the text of each of its items that the fake
/// destination received, and with [`ReplacedInputLabel::Include`] the replaced input label.
pub(crate) async fn sent_context(
    admin: &Client,
    run_id: &str,
    classes: &[&str],
    label: ReplacedInputLabel,
) -> Value {
    let replaced = matches!(label, ReplacedInputLabel::Include);
    json_of(
        admin,
        "SELECT jsonb_agg(jsonb_build_array(attempt_role, decision_position)
                      || (SELECT jsonb_agg(jsonb_path_query_array(wire,
                                   '$.context[*] ? (@.source_class == $class).content',
                                   jsonb_build_object('class', class)) ORDER BY place)
                            FROM unnest($2::text[]) WITH ORDINALITY AS c(class, place))
                      || CASE WHEN $3 THEN jsonb_build_array(wire->'replaces_earlier_input')
                              ELSE '[]'::jsonb END
                      ORDER BY model_attempt_id)::text
               FROM storyos.model_attempts,
                    LATERAL (SELECT (payload #>> '{wire,serialized_payload}')::jsonb) AS w(wire)
              WHERE run_id = $1::text::uuid",
        &[&run_id, &classes, &replaced],
    )
    .await
}
