//! Pure StoryOS Core classification for bounded author commands.

mod accept_proposal;
mod agent_run_control;
mod agent_run_kind;
mod append_proposal_generation_batch;
mod apply_memory_operations;
mod archive_path;
mod archive_zip;
mod assemble_context;
mod chapter_capture_schedule;
mod chapter_list;
mod compact_active_context;
mod continuation_input;
mod conversation_retention;
mod create_admission;
mod create_agent_run;
mod create_chapter;
mod create_project;
mod create_volume;
mod delete_chapter;
mod delete_memory_entry;
mod delete_volume;
mod deployment_destination;
mod earlier_author_messages;
mod embedding_request;
mod finish_rule;
mod function_calling;
mod grant_scope;
mod host_fake_profile;
mod host_tool;
mod hybrid_search_manuscript;
mod instruction_binding;
mod load_skill;
mod manuscript_payload;
mod manuscript_use;
mod memory_capture;
mod memory_conflict_flag;
mod memory_consolidation;
mod memory_delete_marks;
mod memory_entry;
mod memory_index;
mod memory_operation_decision;
mod memory_prune;
mod memory_work_schedule;
mod model_output;
mod model_registration;
mod ordinary_passage_request;
mod read_chapter;
mod read_memory_entry;
mod read_skill_resource;
mod read_tool_result;
mod rebuild_expired_reference;
mod record_memory_observations;
mod research_destination;
mod research_source;
mod research_synthesis;
mod retrieve_original_result;
mod run_result;
mod search_manuscript;
mod search_memory;
mod submit_research;
mod tool_call;
mod tool_call_recovery;
mod tool_dispatch;
mod tool_exposure;
mod tool_loop_profile;
mod tool_registration;
mod tool_result;
mod tool_result_context;
mod tool_result_stub;
mod tool_spec;
mod unknown_create_successor;
pub use apply_memory_operations::{
    APPLY_MEMORY_OPERATIONS_TOOL_SPEC, AdmittedOperation, MemoryOperationAction,
    MemoryOperationCandidate, MemoryOperationRefusal, PassFacts, PassMemoryEntry, PassSettlement,
    intake as apply_memory_operations_intake, parameters as apply_memory_operations_parameters,
    settle_operations_call,
};
pub use ordinary_passage_request::{
    HumanChapterReference, HumanPassageReference, ORDINARY_PASSAGE_PROFILE,
    OrdinaryPassageResolution, PASSAGE_REFERENCE_QUESTION, parse_ordinary_passage_request,
};
mod open_block_proposal;
mod passage_cut;
mod passage_ranking;
mod project_archive;
mod project_export;
mod readable_export;
mod readable_export_command;
mod reject_proposal_operations;
mod revision_comparison;
mod set_current_chapter;
mod skill_instruction;
mod skill_package;
mod skill_use;
mod statistics_profile;
mod take_over_project_writer;
mod transition_outcome;
mod undo_latest_author_action;
mod update_chapter;
mod update_project;
mod update_project_assistance;
mod update_project_instruction;
mod update_volume;
mod withdraw_proposal;
mod write_memory;

pub use accept_proposal::{
    AcceptProposal, AcceptProposalConflict, AcceptProposalInvalid, AcceptProposalOutcome,
    AcceptProposalRefusal, AcceptanceBase, AcceptanceCondition, AcceptanceTargetResolution,
    AcceptanceValidation, ProposalBundlePolicy, ProposalOperationSelection, ProposalSelectionFacts,
    ProposalSelectionIntent, accept_proposal, classify_proposal_selection,
};
pub use agent_run_control::{
    AgentRunStatus, CancelAgentRunConflict, CancelAgentRunNoEffect, PauseAgentRunConflict,
    PauseAgentRunNoEffect, SteerAgentRunConflict, SteerAgentRunNoEffect, SteeringReader,
    classify_cancel_agent_run, classify_pause_agent_run, classify_steer_agent_run,
};
pub use agent_run_kind::{AgentRunKind, MEMORY_CAPTURE_MAX_RUN_STEPS, yields_to_author};
pub use append_proposal_generation_batch::{
    AppendProposalGenerationBatch, AppendProposalGenerationBatchConflict,
    AppendProposalGenerationBatchRefusal, AppendProposalGenerationBatchResult,
    append_proposal_generation_batch,
};
pub use archive_path::{
    ARCHIVE_PATH_MAX_BYTES, ARCHIVE_PATH_MAX_SEGMENTS, ARCHIVE_PATH_PROFILE, AdmittedArchivePath,
    ArchivePathRefusal, admit_archive_path,
};
pub use assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ContextBlockReason, ContextCause, ContextSourceClass,
    ContextSufficiency, CurrentPassageAssembly, CurrentPassageAssemblyRecord, DestinationIo,
    InstructionBindingInput, PassageContextTarget, ProjectionMode, RejectionReason,
    assemble_current_passage_context, assemble_passage_collection, count_context_item_tokens,
    decode_assembly_record, encode_assembly_record,
};
pub use chapter_capture_schedule::{
    CHAPTER_CAPTURE_INTERVAL, CHAPTER_CAPTURE_RATE, CHAPTER_QUIET_TIME, ChapterCaptureFacts,
    ChapterCaptureInput, ChapterCaptureSkip, ChapterCaptureTurn, ChapterRowChange,
    chapter_capture_due, chapter_capture_input, chapter_capture_turn, decide_chapter_capture,
};
pub use chapter_list::{ChapterListEntry, assemble_chapter_list};
pub use compact_active_context::{
    ACTIVE_COMPACTION_POSITIONS_PREFIX, ACTIVE_COMPACTION_REQUEST_PREFIX,
    AGENT_PLAN_COMPACTION_PRODUCER, COMPACTION_LOSS_SEMANTIC_PRESERVATION_UNKNOWN,
    COMPACTION_SUMMARY_UNAVAILABLE, COMPACTION_THRESHOLD_PERCENT, CompactionInstallFacts,
    CompactionInstallRefusal, CompactionKnownInput, CompactionRecordState, CompactionRequestFacts,
    CompactionSource, CompactionStep, CompactionTrigger, DecisionRequestKind,
    HOST_FAKE_COMPACTION_OUTPUT, HOST_FAKE_COMPACTION_PRODUCER, StagedCompaction,
    SummaryObservation, active_context_input_digest, compaction_assembly_record, compaction_due,
    compaction_requested, compaction_threshold, conversation_too_long, decide_compaction_install,
    decision_request_kind, later_request_record, later_request_too_long, next_compaction_step,
    stage_compaction, summary_fits_item, summary_text,
};
pub use continuation_input::{
    ContinuationIdentity, ContinuationInputMapping, ContinuationMappingInput, PriorInput,
    continuation_boundary_matches, continuation_mapping_can_represent, map_continuation_input,
};
pub use conversation_retention::earlier_turns_expired;
pub use create_admission::{
    CreateAdmission, CreateAdmissionFacts, RouteFacts, admit_create, admit_route,
};
pub use create_agent_run::{
    AssistanceAdmission, ChapterAdmission, ConversationAdmission, CreateAgentRun,
    CreateAgentRunOutcome, CreateAgentRunRefusal, create_agent_run,
};
pub use create_chapter::{
    CreateChapter, CreateChapterApplied, CreateChapterConflict, CreateChapterCurrent,
    CreateChapterOpen, CreateChapterPlacement, CreateChapterRefusal, CreateChapterResult,
    VolumeJoin, create_chapter,
};
pub use create_project::{CreateProjectResult, ProjectPresence, create_project};
pub use create_volume::{
    CreateVolume, CreateVolumeApplied, CreateVolumeConflict, CreateVolumeRefusal,
    CreateVolumeResult, create_volume,
};
pub use delete_chapter::{
    ChapterRemovalLifecycle, DeleteChapter, DeleteChapterApplied, DeleteChapterConflict,
    DeleteChapterCurrent, DeleteChapterNoEffect, DeleteChapterRefusal, DeleteChapterResult,
    delete_chapter,
};
pub use delete_memory_entry::{
    DeleteMemoryEntry, DeleteMemoryEntryConflict, DeleteMemoryEntryNoEffect,
    DeleteMemoryEntryRefusal, DeleteMemoryEntryResult, LastMemoryVersion, delete_memory_entry,
};
pub use delete_volume::{
    DeleteVolume, DeleteVolumeApplied, DeleteVolumeConflict, DeleteVolumeNoEffect,
    DeleteVolumeRefusal, DeleteVolumeResult, VolumeChildPolicy, VolumeRemovalLifecycle,
    delete_volume,
};
pub use deployment_destination::{
    AGENT_PLAN_ACCOUNT_BOUNDARY, AGENT_PLAN_CREDENTIAL_REFERENCE, AGENT_PLAN_ELIGIBILITY_EVIDENCE,
    AGENT_PLAN_ENDPOINT, AGENT_PLAN_STORED_RESPONSE_RETENTION, BudgetBounds, DeploymentDestination,
    DestinationKind, RequestBounds, RuntimeQualification,
};
pub use earlier_author_messages::{
    EARLIER_AUTHOR_MESSAGES_TOKEN_LIMIT, EarlierAuthorMessage, EarlierAuthorMessageSource,
    is_earlier_author_message, keep_earlier_author_messages,
};
pub use embedding_request::{
    AGENT_PLAN_EMBEDDING_REGISTRATION, EMBEDDING_DIMENSIONS, EMBEDDING_REQUEST_BOUNDS,
    EMBEDDING_REQUEST_TIMEOUT, EMBEDDING_TEXT_TOKEN_LIMIT, EmbeddingAdapter,
    EmbeddingAdmissionFacts, EmbeddingBindingFacts, EmbeddingCause, EmbeddingInput,
    EmbeddingRefusal, EmbeddingRegistration, EmbeddingSettlement, HOST_FAKE_EMBEDDING_REGISTRATION,
    MANUSCRIPT_EMBEDDING_PURPOSE, admit_embedding, embedding_binding_to_make,
};
pub use finish_rule::{
    FinishRuleOutcome, MAX_RUN_STEPS, PriorStepCall, RunStepLimit, ToolChoice,
    UNEXECUTED_CALL_OUTPUT, apply_finish_rule, step_tool_choice,
};
pub use function_calling::{
    PROBE_FIXTURE_CAPABILITY_PROFILE, PROBE_FIXTURE_REGISTRATION, ToolProbe, tool_probe,
};
pub use grant_scope::{GrantFacts, GrantScope, admit_disclosure};
pub use host_fake_profile::{
    ADVISORY_TEXT, ExecutionCapability, HOST_FAKE_MAPPING_REVISION, PROSE_CHANGE_TEXT,
    SECOND_PROSE_CHANGE_TEXT, requested_execution_capability,
};
pub use host_tool::{
    FINISH_DOES_NOT_COUNT_RESULT, HOST_TOOL_REGISTRATION, HostToolRegistration, execute_host_tool,
    host_tool, host_tool_result, host_tool_result_class,
};
pub use hybrid_search_manuscript::{
    HYBRID_SEARCH_MANUSCRIPT_TOOL_SPEC, HybridSearchRefusal, NO_SEARCH_BY_MEANING,
    admit_hybrid_search, hybrid_search_result,
};
pub use instruction_binding::{
    BoundInstruction, assemble_instruction_binding, project_instruction_binding,
};
pub use load_skill::{
    LOAD_SKILL_TOOL_SPEC, LoadSkillRefusal, SKILL_LOAD_LIMIT, SkillLoadFacts, admit_skill_load,
    skill_load_result,
};
pub use manuscript_payload::{
    ApplyVersionedAuthorEdit, ApplyVersionedAuthorEditOutcome, COORDINATE_VERSION,
    MANUSCRIPT_SCHEMA_VERSION, ManuscriptBlock, ManuscriptBlockKind, ManuscriptPayload,
    VersionedTargetOwnership, apply_versioned_author_edit, chapter_display_body,
    upgrade_legacy_manuscript,
};
pub use manuscript_use::{ManuscriptUse, StoredManuscriptCall, manuscript_uses};
pub use memory_capture::{
    CaptureAdmissionFacts, CaptureEnd, CaptureInputKind, CaptureReferences, CaptureRefusal,
    CaptureRunEnd, MemoryCaptureAssembly, admit_memory_capture, assemble_capture_references,
    assemble_memory_capture_context, capture_end, keep_capture_references,
};
pub use memory_conflict_flag::MemoryConflictFlag;
pub use memory_consolidation::{
    ConsolidationOutcome, MEMORY_CONSOLIDATION_MAX_RUN_STEPS, MemoryConsolidationAssembly,
    PassConflict, PassObservation, PassOperationResult, admit_memory_consolidation,
    assemble_memory_consolidation_context, consolidation_failure, observation_return_refusal,
    observation_returns, pass_observation_count,
};
pub use memory_delete_marks::{
    MEMORY_DELETE_MARK_CAUSES, MemoryDeleteMark, assemble_memory_delete_marks,
    memory_delete_marks_text,
};
pub use memory_entry::{
    MemoryEntryOrigin, MemoryEntryState, MemoryEntryType, MemorySourceReference,
    memory_revision_pin,
};
pub use memory_index::{
    MEMORY_INDEX_MAX_ENTRIES, MEMORY_INDEX_TOKEN_LIMIT, MemoryIndexEntry, assemble_memory_index,
    memory_index_text,
};
pub use memory_prune::{MEMORY_UNUSED_AFTER_DAYS, memory_entry_unused};
pub use memory_work_schedule::{
    CONSOLIDATION_INTERVAL, CONVERSATION_QUIET_TIME, ConsolidationFacts, ConsolidationSkip,
    ConversationCaptureFacts, ConversationCaptureInput, ConversationCaptureSkip, ConversationRole,
    ConversationRunMessages, capture_settlement_due, consolidation_due, conversation_capture_due,
    conversation_capture_input, decide_consolidation, decide_conversation_capture,
};
pub use model_output::{
    AgentDecisionKind, AgentDecisionOutcome, NativeStreamItem, StreamItemRole, StreamItemState,
    validate_agent_decision,
};
pub use model_registration::{
    AGENT_PLAN_CAPABILITY_PROFILE, COMPACTION_REQUIREMENT, CapabilityCombination,
    CapabilityEntries, CapabilityEvidence, ContextWindow, ModelAdapter, ModelCapability,
    ModelCapabilityProfile, ModelRegistration, RECOVERY_REQUIREMENT, model_registration,
};
pub use open_block_proposal::{
    OpenBlockProposal, OpenBlockProposalConflict, OpenBlockProposalRefusal,
    OpenBlockProposalResult, open_block_proposal,
};
pub use passage_cut::{
    PASSAGE_CHARACTER_LIMIT, PASSAGE_CUT_REVISION, Passage, PassageBlock, cut_passages,
    passage_embedding_text,
};
pub use passage_ranking::{
    ChapterPassages, LocatedPassage, NearestPassage, PASSAGE_FUSION_CONSTANT, PASSAGE_LIST_LIMIT,
    PASSAGE_RESULT_LIMIT, PASSAGE_TERM_LIMIT, PASSAGE_TERM_MAX_CHARS, PassagePlace, PassageTerms,
    PassageTermsRefusal, RankingBlock, RankingChapter, admit_passage_ranking, current_passages,
    exact_term_list, fuse_passage_lists, vector_list,
};
pub use project_archive::{
    ARCHIVE_ENTRY_DIGEST_PROFILE, ARCHIVE_ROOT_DIGEST_PROFILE, ARCHIVE_SERIALIZATION_PROFILE,
    ArchiveEntryDescriptor, ArchiveEntrySource, BuiltProjectArchive,
    PROJECT_EXPORT_ARCHIVE_PROFILE, PROJECT_EXPORT_TABLES, ProjectArchiveBuildRefusal,
    ProjectArchiveRootFacts, build_project_archive, canonical_json, classify_export_record,
    hex_sha256, package_verified_project_archive_zip, require_delivered_families,
};
pub use project_export::{
    ExportProjectArchive, ExportProjectArchiveRefusal, ExportProjectArchiveResult,
    export_project_archive,
};
pub use read_chapter::{
    BlockRange, ChapterReadRefusal, READ_CHAPTER_TOOL_SPEC, ReadableChapter, admit_chapter_read,
};
pub use read_memory_entry::{
    MEMORY_READ_NOT_FOUND, MemoryEntryRead, MemoryFlagSource, MemoryReadFlag,
    READ_MEMORY_ENTRY_TOOL_SPEC, memory_read_not_found,
};
pub use read_skill_resource::{
    READ_SKILL_RESOURCE_TOOL_SPEC, SKILL_RESOURCE_READ_LIMIT, SkillResourceReadFacts,
    SkillResourceRefusal, admit_skill_resource_read,
};
pub use read_tool_result::{
    READ_TOOL_RESULT_TOOL_SPEC, ReadTarget, ReadToolResultRefusal, StoredCall, read_tool_result,
};
pub use readable_export::{
    READABLE_EXPORT_PROFILE, READABLE_EXPORT_UNAVAILABLE_MARKER, ReadableExportChapter,
    ReadableExportVolume, render_readable_manuscript,
};
pub use readable_export_command::{
    ExportHumanReadableManuscript, ExportHumanReadableManuscriptRefusal,
    ExportHumanReadableManuscriptResult, export_human_readable_manuscript,
};
pub use rebuild_expired_reference::{
    ContinuationReferenceCondition, ExpiryRebuildBlock, ExpiryRebuildDecision,
    ExpiryRebuildDisposition, ExpiryRebuildFacts, decide_confirmed_expiry_rebuild,
};
pub use record_memory_observations::{
    CAPTURE_OBSERVATIONS_MAX, CaptureOutcome, MemoryObservation, ObservationCallSettlement,
    ObservationCandidate, ObservationKind, ObservationRefusal,
    RECORD_MEMORY_OBSERVATIONS_TOOL_SPEC, observations_result, settle_observation_call,
    validate_observations,
};
pub use reject_proposal_operations::{
    RejectProposalOperations, RejectProposalOperationsConflict, RejectProposalOperationsRefusal,
    RejectProposalOperationsResult, reject_proposal_operations,
};
pub use research_destination::{
    EXA_ACCOUNT_BOUNDARY, EXA_CREDENTIAL_REFERENCE, EXA_MCP_SERVER_URL, EXA_TOOL_REGISTRATION,
    RESEARCH_TOOL_OFFER, ResearchToolOffer, tool_registration,
};
pub use research_source::{
    CapturedSource, ResearchCapture, ResultFormat, ResultSpan, SOURCE_LOCATOR_MAX_CHARS,
    SOURCE_PUBLISHED_MAX_CHARS, SOURCE_TITLE_MAX_CHARS, SourceGap, SourceGapReason, SourceKind,
    capture_research_sources, tool_result_source_version,
};
pub use research_synthesis::{
    CitableSource, CitableSources, Citation, CitedSource, ClaimCandidate, ResearchClaim,
    ResearchSynthesis, SynthesisRefusal, citable_sources_tag, validate_research_synthesis,
};
pub use retrieve_original_result::{
    OriginalResultKeepReason, OriginalResultRetrievalDecision, OriginalResultRetrievalFacts,
    RetainedResponseReference, RetrievalBounds, RetrievalCapability, RetrievedOriginalResult,
    decide_original_result_retrieval,
};
pub use revision_comparison::{
    ExactRevisionTexts, ReplacementSpan, RevisionComparison, RevisionComparisonAccess,
    inspect_revision_comparison,
};
pub use run_result::{RunCallPosition, run_results};
pub use search_manuscript::{
    MANUSCRIPT_SEARCH_MATCH_LIMIT, SEARCH_MANUSCRIPT_TOOL_SPEC, admit_manuscript_search,
    exact_matches,
};
pub use search_memory::{SEARCH_MEMORY_TOOL_SPEC, search_memory};
pub use set_current_chapter::{
    SetCurrentChapter, SetCurrentChapterConflict, SetCurrentChapterNoEffect,
    SetCurrentChapterRefusal, SetCurrentChapterResult, set_current_chapter,
};
pub use skill_instruction::{
    CONVERSATION_SKILL_LIMIT, SKILL_SELECTION_LIMIT, SkillInstructionSource,
    SkillInstructionUnavailable, SkillProvenance, SkillRequirement, SkillSelectionUnavailable,
    admit_skill_selections, assemble_skill_catalog, assemble_skill_instructions,
    conversation_skill_requirements, is_conversation_skill, skill_reference_well_formed,
};
pub use skill_package::{
    FrontmatterValue, SKILL_CATALOG_TOKEN_LIMIT, SKILL_MD_PATH, SKILL_PACKAGE_BYTE_LIMIT,
    SKILL_PACKAGE_FILE_LIMIT, SKILL_PACKAGE_INVENTORY_PROFILE, SKILL_SET_LIMIT, SkillCatalogEntry,
    SkillCatalogPlan, SkillFile, SkillFileInput, SkillPackage, SkillPackageInput,
    SkillPackageReference, SkillPackageRefusal, SkillSet, SkillSetRefusal, admit_skill_package,
    plan_skill_catalog, skill_catalog_text, skill_md_frontmatter,
};
pub use skill_use::{SkillUse, StoredSkillLoad, skill_uses};
pub use statistics_profile::{
    STATISTICS_COUNTING_PROFILE, TextStatistics, count_stored_text, count_stored_texts,
};
pub use submit_research::{
    RESEARCH_RECORDED_RESULT, SUBMIT_RESEARCH_TOOL_SPEC, SubmitResearchRefusal,
};
pub use take_over_project_writer::{
    CurrentWriter, StaleWriterObservation, TakeOverProjectWriter, TakeOverProjectWriterNoEffect,
    TakeOverProjectWriterResult, take_over_project_writer,
};
pub use tool_call::{
    CallValidationRefusal, CurrentToolAuthority, ExposureSnapshot, MAX_CALL_ID_CHARS,
    MAX_FUNCTION_NAME_CHARS, RefusedToolCall, RequestedToolCall, SnapshotTool, ToolCallBatch,
    ToolCallBatchRefusal, ValidatedToolCall, validate_tool_call_batch,
};
pub use tool_call_recovery::{
    LateToolEvidence, ToolCallClass, claimed_tool_reservation, late_tool_evidence, tool_call_class,
};
pub use tool_dispatch::{
    EffectOutcome, FunctionRequestStep, ToolAttemptSettlement, ToolCallPlan, ToolCallSettlement,
    ToolCallState, ToolDispatchFacts, ToolDispatchRefusal, ToolEffectOutcome, ToolEffectRequest,
    ToolObservation, ToolResultText, admit_tool_dispatch, function_request_step, settle_tool_call,
};
pub use tool_exposure::{
    BoundTool, ExposedTool, ExposureFacts, FUNCTION_CALL_NATIVE_TYPE, FunctionDeclaration,
    RESEARCH_TOOL_UNAVAILABLE_CAPABILITY, ResponseDisposition, ResponseFacts,
    ToolCompatibilityDecision, ToolExposure, function_request_refusal_capability,
    requests_function, response_disposition,
};
pub use tool_loop_profile::{
    AGENT_PLAN_REGISTRATION, AGENT_PLAN_WRITING_RULES_MAPPING_REVISION,
    HOST_FAKE_TOOL_LOOP_CAPABILITY_PROFILE, HOST_FAKE_TOOL_LOOP_MAPPING_REVISION,
    HOST_FAKE_TOOL_LOOP_REGISTRATION,
};
pub use tool_registration::{
    ContractCheck, ContractDifference, MCP_STREAMABLE_HTTP_ADAPTER_REVISION, RegisteredTool,
    ToolCallRefusal, ToolRegistration, ToolRegistrationKind, ToolRegistrationLifecycle,
};
pub use tool_result::{
    BoundedToolResult, TOOL_RESULT_SOURCE_TAG_INSTRUCTION, ToolResultProjection, Unavailability,
};
pub use tool_result_context::{
    SKILL_LOAD_TRUST, SKILL_RESOURCE_TRUST, TOOL_RESULT_TRUST, ToolResultCandidate, ToolResultForm,
    ToolResultOutput, ToolResultSource, assemble_tool_result_continuation, function_outputs,
    is_function_output, is_skill_load_output, stub_tool_call_ids,
};
pub use tool_result_stub::TOOL_RESULT_STUB_TOKEN_LIMIT;
pub use tool_spec::{
    ExternalRead, FINISH_TOOL_SPEC, IntakeField, IntakeRefusal, PROPOSE_EDIT_TOOL_SPEC,
    ServerArguments, TOOL_RESULT_TEXT_MAX_BYTES, ToolEffectEnvelope, ToolIntake, ToolOperation,
    ToolSpec, WEB_FETCH_TOOL_SPEC, WEB_SEARCH_TOOL_SPEC, tool_spec,
};
pub use transition_outcome::{ReasonCode, ReceiptResult, TransitionOutcome};
pub use undo_latest_author_action::{
    AuthorUndoFrontier, AuthorUndoFrontierKind, UndoLatestAuthorAction,
    UndoLatestAuthorActionApplied, UndoLatestAuthorActionConflict, UndoLatestAuthorActionOutcome,
    UndoLatestAuthorActionUnavailable, undo_latest_author_action,
};
pub use unknown_create_successor::{
    LookupUnavailable, SuccessorAllowance, SuccessorConditions, SuccessorEffect, SuccessorLookup,
    SuccessorPauseReason, UnknownCreateScript, UnknownCreateSuccessorDecision,
    UnknownCreateSuccessorFacts, decide_unknown_create_successor, route_successor_conditions,
    scripted_successor_conditions, unknown_create_script,
};
pub use update_chapter::{
    ChapterJoin, UpdateChapter, UpdateChapterApplied, UpdateChapterConflict, UpdateChapterNoEffect,
    UpdateChapterRefusal, UpdateChapterResult, update_chapter,
};
pub use update_project::{
    UpdateProject, UpdateProjectApplied, UpdateProjectConflict, UpdateProjectNoEffect,
    UpdateProjectRefusal, UpdateProjectResult, update_project,
};
pub use update_project_assistance::{
    AssistanceAvailability, AssistanceBindingPresence, UpdateProjectAssistance,
    UpdateProjectAssistanceApplied, UpdateProjectAssistanceConflict,
    UpdateProjectAssistanceNoEffect, UpdateProjectAssistanceResult, update_project_assistance,
};
pub use update_project_instruction::{
    UpdateProjectInstruction, UpdateProjectInstructionApplied, UpdateProjectInstructionConflict,
    UpdateProjectInstructionNoEffect, UpdateProjectInstructionRefusal,
    UpdateProjectInstructionResult, update_project_instruction,
};
pub use update_volume::{
    UpdateVolume, UpdateVolumeApplied, UpdateVolumeConflict, UpdateVolumeNoEffect,
    UpdateVolumeRefusal, UpdateVolumeResult, update_volume,
};
pub use withdraw_proposal::{
    WithdrawProposal, WithdrawProposalConflict, WithdrawProposalNoEffect, WithdrawProposalRefusal,
    WithdrawProposalResult, WithdrawalCause, withdraw_proposal,
};
pub use write_memory::{
    CurrentMemoryEntry, MEMORY_CHARACTER_TEXT_MAX_CHARS, MEMORY_DESCRIPTION_MAX_CHARS,
    MEMORY_TEXT_MAX_CHARS, MEMORY_TITLE_MAX_CHARS, MemoryVersionWrite, MemoryWrite,
    MemoryWriteAction, MemoryWriteRefusal, WRITE_MEMORY_TOOL_SPEC, decide_memory_write,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplyAuthorEdit {
    pub chapter_id: String,
    pub current_authoritative_revision_id: String,
    pub current_body: String,
    pub expected_authoritative_revision_id: String,
    pub expected_proposal_head_revision_ids: Vec<String>,
    pub current_ownership: CurrentOwnershipFacts,
    pub target_refs: Vec<String>,
    pub observed_ownership_partition: String,
    pub author_edit_units: Vec<AuthorEditUnit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentOwnershipFacts {
    pub proposal_head_revision_ids: Vec<String>,
    pub unresolved_reservation_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorEditUnit {
    pub normalized_primitives: Vec<AuthorEditPrimitive>,
    pub selection_snapshot: SelectionSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthorEditPrimitive {
    ReplaceSelection {
        from: u32,
        to: u32,
        text: String,
    },
    ReplaceBlockSelection {
        manuscript_block_id: String,
        from: u32,
        to: u32,
        text: String,
    },
    SplitBlock {
        manuscript_block_id: String,
        offset: u32,
        new_manuscript_block_id: String,
    },
    JoinBlocks {
        left_manuscript_block_id: String,
        right_manuscript_block_id: String,
    },
    MoveBlock {
        manuscript_block_id: String,
        to_index: u32,
    },
    RetypeBlock {
        manuscript_block_id: String,
        block_kind: crate::ManuscriptBlockKind,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionSnapshot {
    pub coordinate_profile: String,
    pub from: u32,
    pub to: u32,
}

/// The Core classification of one Author Edit.
pub type ApplyAuthorEditOutcome =
    TransitionOutcome<AuthorEditApplied, AuthorEditNoEffect, AuthorEditConflict, AuthorEditRefusal>;

/// The applied variants of an Author Edit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditApplied {
    /// The edit changes the Authoritative Revision to `body`.
    AuthoritativeApplied { body: String },
    /// The edit changes the candidate of one Proposal to `candidate_text`.
    ProposalRevised { candidate_text: String },
}

impl AuthorEditApplied {
    /// The Domain Receipt result kind of this applied variant.
    pub fn result_kind(&self) -> &'static str {
        match self {
            Self::AuthoritativeApplied { .. } => "authoritative_applied",
            Self::ProposalRevised { .. } => "proposal_revised",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditNoEffect {
    ContentUnchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditConflict {
    StaleAuthoritativeHead,
    ProposalHeadPresent,
    OwnershipChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthorEditRefusal {
    UnsupportedIntentShape,
    InvalidSelection,
    TargetMismatch,
}

transition_outcome::reason_codes!(AuthorEditNoEffect {
    ContentUnchanged => "content_unchanged",
});
transition_outcome::reason_codes!(AuthorEditConflict {
    StaleAuthoritativeHead => "stale_authoritative_head",
    ProposalHeadPresent => "proposal_head_present",
    OwnershipChanged => "ownership_changed",
});
transition_outcome::reason_codes!(AuthorEditRefusal {
    UnsupportedIntentShape => "unsupported_intent_shape",
    InvalidSelection => "invalid_selection",
    TargetMismatch => "target_mismatch",
});

pub const UTF16_COORDINATE_PROFILE: &str = "storyos.editor.utf16-code-unit.v1";

pub fn apply_author_edit(command: &ApplyAuthorEdit) -> ApplyAuthorEditOutcome {
    if command.expected_authoritative_revision_id != command.current_authoritative_revision_id {
        return TransitionOutcome::Conflicted(AuthorEditConflict::StaleAuthoritativeHead);
    }
    if command.expected_proposal_head_revision_ids
        != command.current_ownership.proposal_head_revision_ids
    {
        return TransitionOutcome::Conflicted(AuthorEditConflict::ProposalHeadPresent);
    }
    let current_partition = if command
        .current_ownership
        .proposal_head_revision_ids
        .is_empty()
        && command
            .current_ownership
            .unresolved_reservation_refs
            .is_empty()
    {
        "authoritative"
    } else {
        "mixed"
    };
    if command.observed_ownership_partition != current_partition {
        return TransitionOutcome::Conflicted(AuthorEditConflict::OwnershipChanged);
    }
    if current_partition == "authoritative" {
        return apply_author_edit_body(command, AuthorEditAppliedKind::Authoritative);
    }
    if command.expected_proposal_head_revision_ids.is_empty() {
        return TransitionOutcome::Conflicted(AuthorEditConflict::OwnershipChanged);
    }
    apply_author_edit_body(command, AuthorEditAppliedKind::Proposal)
}

enum AuthorEditAppliedKind {
    Authoritative,
    Proposal,
}

fn apply_author_edit_body(
    command: &ApplyAuthorEdit,
    kind: AuthorEditAppliedKind,
) -> ApplyAuthorEditOutcome {
    if command.target_refs != [format!("manuscript:{}", command.chapter_id)] {
        return TransitionOutcome::Refused(AuthorEditRefusal::TargetMismatch);
    }
    if command.author_edit_units.is_empty() {
        return TransitionOutcome::Refused(AuthorEditRefusal::UnsupportedIntentShape);
    }
    let mut body = command.current_body.clone();
    for unit in &command.author_edit_units {
        let [AuthorEditPrimitive::ReplaceSelection { from, to, text }] =
            unit.normalized_primitives.as_slice()
        else {
            return TransitionOutcome::Refused(AuthorEditRefusal::UnsupportedIntentShape);
        };
        if unit.selection_snapshot.coordinate_profile != UTF16_COORDINATE_PROFILE
            || unit.selection_snapshot.from != *from
            || unit.selection_snapshot.to != *to
        {
            return TransitionOutcome::Refused(AuthorEditRefusal::InvalidSelection);
        }
        if let Err(reason) = replace_checked_utf16_range(&mut body, *from, *to, text) {
            return TransitionOutcome::Refused(reason);
        }
    }
    if body == command.current_body {
        TransitionOutcome::NoEffect(AuthorEditNoEffect::ContentUnchanged)
    } else {
        match kind {
            AuthorEditAppliedKind::Authoritative => {
                TransitionOutcome::Applied(AuthorEditApplied::AuthoritativeApplied { body })
            }
            AuthorEditAppliedKind::Proposal => {
                TransitionOutcome::Applied(AuthorEditApplied::ProposalRevised {
                    candidate_text: body,
                })
            }
        }
    }
}

pub(crate) fn replace_checked_utf16_range(
    body: &mut String,
    from: u32,
    to: u32,
    text: &str,
) -> Result<(), AuthorEditRefusal> {
    let Some(from_byte) = utf16_offset_to_byte(body, from) else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    if from > to {
        return Err(AuthorEditRefusal::InvalidSelection);
    }
    let to_byte = if from == to {
        from_byte
    } else if let Some(suffix_byte) = utf16_offset_to_byte(&body[from_byte..], to - from) {
        from_byte + suffix_byte
    } else {
        return Err(AuthorEditRefusal::InvalidSelection);
    };
    let removed = to_byte - from_byte;
    if text.len() > removed {
        body.reserve(text.len() - removed);
    }
    body.replace_range(from_byte..to_byte, text);
    Ok(())
}

pub(crate) fn utf16_offset_to_byte(value: &str, wanted: u32) -> Option<usize> {
    if wanted == 0 {
        return Some(0);
    }
    let mut units = 0_u32;
    for (byte, character) in value.char_indices() {
        units += character.len_utf16() as u32;
        if units == wanted {
            return Some(byte + character.len_utf8());
        }
        if units > wanted {
            return None;
        }
    }
    (units == wanted).then_some(value.len())
}

#[cfg(test)]
mod shared_tests;

#[cfg(test)]
#[path = "apply_author_edit_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "create_project_tests.rs"]
mod create_project_tests;

#[cfg(test)]
#[path = "update_project_tests.rs"]
mod update_project_tests;

#[cfg(test)]
#[path = "create_volume_tests.rs"]
mod create_volume_tests;

#[cfg(test)]
#[path = "create_chapter_tests.rs"]
mod create_chapter_tests;

#[cfg(test)]
#[path = "manuscript_payload_tests.rs"]
mod manuscript_payload_tests;

#[cfg(test)]
#[path = "update_volume_tests.rs"]
mod update_volume_tests;

#[cfg(test)]
#[path = "set_current_chapter_tests.rs"]
mod set_current_chapter_tests;

#[cfg(test)]
#[path = "undo_latest_author_action_tests.rs"]
mod undo_latest_author_action_tests;

mod prose_change_locations;
pub use prose_change_locations::{
    DeclaredLocation, DeclaredTargetKind, ProseChangeCandidate, ProseChangeRefusal,
    prose_change_refusal,
};
mod propose_edit;
pub use propose_edit::{
    BlockChange, MAX_REFUSED_PROPOSE_EDIT_CALLS, PROPOSAL_WAITS_RESULT, ProposeEditHistory,
    ProposeEditRefusal, ProposedEdit, propose_edit_parameters, screen_proposed_edits,
};
mod selection_reference;
pub use selection_reference::{
    AdmittedSelection, ChapterHeadPlace, CurrentCandidate, GrownCandidate,
    SELECTION_REFERENCE_LIMIT, SelectionReference, TextPoint, advance_candidate_references,
    assemble_selection_references, follow_chapter_head, follow_current_candidates,
    merge_steering_references, reference_joined_candidates, select_candidate_text,
    select_chapter_text,
};

mod assemble_candidate_context;
pub use assemble_candidate_context::{ProposalCandidateTarget, assemble_candidate_context};
