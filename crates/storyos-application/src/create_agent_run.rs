use std::convert::Infallible;
use std::future::Future;

use storyos_core::CreateAgentRunRefusal;

use crate::{ActivityApplied, ProjectCommandSettlement, ProjectScope, RefusableCommandError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversationSelection {
    New,
    Existing { conversation_id: String },
}

/// One createAgentRun. The Server allocates the identities of the new Run and conversation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateAgentRunInput {
    pub conversation: ConversationSelection,
    pub author_message: String,
    pub chapter_id: String,
    pub passage_targets: Option<Vec<storyos_core::PassageContextTarget>>,
    pub candidate_target: Option<storyos_core::ProposalCandidateTarget>,
    pub run_id: String,
    pub conversation_id: String,
    pub project_agent_id: String,
    /// The destination of the deployment, from Host configuration. The Run binds it.
    pub destination: storyos_core::DeploymentDestination,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkingTargetAvailability {
    Current,
    Unavailable,
    Superseded { current_revision_id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunContext {
    pub record: storyos_core::CurrentPassageAssemblyRecord,
    pub assembly_manifest_id: String,
    pub destination_context_manifest_id: Option<String>,
    pub outbound_disclosure_manifest_id: Option<String>,
    pub working_target_availability: WorkingTargetAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedMemorySettings {
    pub use_enabled: bool,
    pub contribution_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentRunReadSelection {
    Current,
    ModelAttempt(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunSteeringInspect {
    pub steering_input_id: String,
    pub input_position: String,
    pub author_message: String,
    pub input_snapshot_id: Option<String>,
    pub model_attempt_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunRecord {
    pub project_agent_id: String,
    pub conversation_id: String,
    pub memory_settings_revision: String,
    pub captured_memory_settings: Option<CapturedMemorySettings>,
    pub run_id: String,
    pub status: AgentRunStatus,
    pub steering_inputs: Vec<AgentRunSteeringInspect>,
    pub context: AgentRunContext,
    pub decision: AgentRunDecisionInspect,
    pub model: Option<AgentRunModelInspect>,
    pub active_compaction: Option<ActiveCompactionInspect>,
    pub reference_recovery: Option<ReferenceRecoveryInspect>,
    pub original_result_retrieval: Option<OriginalResultRetrievalInspect>,
    pub unknown_create_successor: Option<UnknownCreateSuccessorInspect>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveCompactionInstallState {
    Staged,
    Installed,
    Refused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveCompactionMappingKind {
    HostManaged,
    Native,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActiveCompactionKnownInput {
    ModelAttempt {
        id: String,
    },
    Manifest {
        id: String,
    },
    Projection {
        source_class: String,
        source_version: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveCompactionInspect {
    pub compaction_id: String,
    pub install_state: ActiveCompactionInstallState,
    pub prior_model_attempt_id: String,
    pub prior_manifest_id: String,
    pub prior_run_step_id: String,
    pub producer_model_attempt_id: String,
    pub producer_manifest_id: String,
    pub producer_invocation_id: String,
    pub producer: String,
    pub mapping_kind: ActiveCompactionMappingKind,
    pub mapping_revision: String,
    pub known_inputs: Vec<ActiveCompactionKnownInput>,
    pub output_text: String,
    pub usage_kind: String,
    pub loss_facts: Vec<String>,
    pub refusal_reason: Option<String>,
    pub installed_run_step_id: Option<String>,
    pub installed_model_invocation_id: Option<String>,
    pub installed_model_attempt_id: Option<String>,
    pub preserved_item_ids: Vec<String>,
    pub admission: AgentRunContinuationAdmission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceRecoveryDisposition {
    Rebuilt,
    Blocked,
    UnknownCreate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceRecoveryInspect {
    pub recovery_id: String,
    pub disposition: ReferenceRecoveryDisposition,
    pub block_reason: Option<String>,
    pub predecessor_run_id: String,
    pub predecessor_continuation_binding_id: Option<String>,
    pub run_step_id: Option<String>,
    pub model_invocation_id: Option<String>,
    pub model_attempt_id: Option<String>,
    pub assembly_manifest_id: Option<String>,
    pub lossless_provider_reconstruction: bool,
    pub semantic_erasure: bool,
    pub opaque_reused: bool,
    pub covered_content_included: bool,
    pub predecessor_terminal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginalResultRetrievalDisposition {
    KeptUnknown,
    EvidenceOnly,
    Settled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalResultRetrievalInspect {
    pub reconciliation_id: String,
    pub disposition: OriginalResultRetrievalDisposition,
    pub keep_reason: Option<String>,
    pub original_model_attempt_id: String,
    pub response_reference_id: Option<String>,
    pub retrieval_attempt_id: Option<String>,
    pub assembly_manifest_id: Option<String>,
    pub repeats_original_create: bool,
    pub resumes_stream: bool,
    pub proves_create_idempotency: bool,
    pub supplies_decision: bool,
    pub supplies_tool_call: bool,
    pub advances_continuation: bool,
    pub reservation_released: bool,
    pub usage_kind: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownCreateSuccessorDisposition {
    Fenced,
    Dispatched,
    Paused,
    Prohibited,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownCreateSuccessorInspect {
    pub recovery_id: String,
    pub disposition: UnknownCreateSuccessorDisposition,
    pub pause_reason: Option<String>,
    pub lookup_unavailable_reason: Option<String>,
    pub predecessor_model_attempt_id: String,
    pub successor_model_attempt_id: Option<String>,
    pub model_invocation_id: String,
    pub predecessor_fenced: bool,
    pub allowance_consumed: bool,
    pub predecessor_usage_kind: String,
    pub predecessor_reservation_released: bool,
    pub successor_settles_predecessor: bool,
    pub supplies_tool_call: bool,
    pub advances_predecessor_continuation: bool,
    pub reuses_changed_context: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunStatus {
    Queued,
    Claimed,
    Waiting,
    Paused,
    Completed,
    Refused,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentRunDecisionInspect {
    Absent,
    ExecutionRefused {
        capability: String,
    },
    Advisory {
        decision_id: String,
        selected: bool,
        text: String,
        continuation_binding_id: Option<String>,
    },
    ProseChange {
        decision_id: String,
        selected: bool,
        text: String,
        producer_input: String,
        locations: Option<Vec<storyos_contracts::ProseChangeLocationInspect>>,
        continuation_binding_id: Option<String>,
        opened_proposal_id: Option<String>,
    },
    Clarification {
        decision_id: String,
        selected: bool,
        question: String,
        required_reply: String,
        continuation_binding_id: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRunInputMapping {
    None,
    Incremental,
    Full,
    NewTransport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunContinuationAdmission {
    pub processing_destination_identity: String,
    pub evidence_revision: String,
    pub model_registration_revision: String,
    pub adapter_mapping: String,
    pub project_model_use_binding_revision: String,
    pub external_compatibility_decision: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunModelInspect {
    pub model_attempt_id: String,
    pub destination_attempt_id: String,
    pub outbound_disclosure_event_id: String,
    pub model_invocation_id: String,
    pub dispatch_state: String,
    pub prior_continuation_binding_id: Option<String>,
    pub known_prior_continuation_binding_id: Option<String>,
    pub input_mapping: AgentRunInputMapping,
    pub admission: AgentRunContinuationAdmission,
    pub evidence: Vec<AgentRunEvidence>,
    pub items: Vec<AgentRunStreamItem>,
    pub usage_kind: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunStreamItem {
    pub item_id: String,
    pub role: String,
    pub state: String,
    pub phase: String,
    pub text: Option<String>,
    pub summary: Option<String>,
    pub call_id: Option<String>,
    pub arguments: Option<String>,
    pub refusal: Option<String>,
    pub hosted_report: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentRunEvidence {
    SentContent {
        attempt_id: String,
        availability: EvidenceAvailability,
        content: String,
    },
    StoredReference {
        attempt_id: String,
        availability: EvidenceAvailability,
        reference_id: String,
    },
    ProviderReport {
        attempt_id: String,
        availability: EvidenceAvailability,
        report: String,
    },
    ProviderOpaque {
        attempt_id: String,
        availability: EvidenceAvailability,
        unknown_facts: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceAvailability {
    Current,
    Unknown,
}

/// The applied effect of one createAgentRun: the queued Run and the settings that it captures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateAgentRunApplied {
    pub project_agent_id: String,
    pub conversation_id: String,
    pub memory_settings_revision: String,
    pub run_id: String,
}

pub type CreateAgentRunSettlement = ProjectCommandSettlement<
    ActivityApplied<CreateAgentRunApplied>,
    Infallible,
    Infallible,
    Infallible,
>;

pub type CreateAgentRunCommandError = RefusableCommandError<CreateAgentRunRefusal>;

/// A failure while reading an AgentRun or assembling its Context.
#[derive(Debug)]
pub enum CreateAgentRunError {
    BindingConflict,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for CreateAgentRunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BindingConflict => formatter.write_str("The createAgentRun binding conflicts"),
            Self::Unavailable(_) => formatter.write_str("The createAgentRun store is unavailable"),
        }
    }
}

impl std::error::Error for CreateAgentRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(source) => Some(source.as_ref()),
            Self::BindingConflict => None,
        }
    }
}

/// Reads one AgentRun in exact Project Scope.
pub trait AgentRunReadStore: Sync {
    fn read_agent_run(
        &self,
        scope: &ProjectScope,
        run_id: &str,
        selection: &AgentRunReadSelection,
    ) -> impl Future<Output = Result<Option<AgentRunRecord>, CreateAgentRunError>> + Send;
}

pub async fn open_agent_run(
    store: &impl AgentRunReadStore,
    scope: &ProjectScope,
    run_id: &str,
) -> Result<Option<AgentRunRecord>, CreateAgentRunError> {
    store
        .read_agent_run(scope, run_id, &AgentRunReadSelection::Current)
        .await
}

pub async fn inspect_agent_run(
    store: &impl AgentRunReadStore,
    scope: &ProjectScope,
    run_id: &str,
    selection: &AgentRunReadSelection,
) -> Result<Option<AgentRunRecord>, CreateAgentRunError> {
    store.read_agent_run(scope, run_id, selection).await
}
