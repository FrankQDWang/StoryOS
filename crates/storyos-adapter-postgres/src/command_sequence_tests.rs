use std::fmt::Debug;

use storyos_application::{
    ArchiveProjectInput, AuthorCommandAdmissionIds, CloseEditorFlowDraftInput,
    CloseEditorFlowDraftSettlement, CreateChapterInput, CreateChapterSettlement, CreateVolumeInput,
    CreateVolumeSettlement, DeleteChapterInput, DeleteChapterSettlement, DeleteVolumeInput,
    DeleteVolumeSettlement, EditorClientBinding, ExpandRefusedEditDraftSettlement,
    ExpandRefusedEditDraftToProposalInput, ProjectCommandChallengeBinding, ProjectCommandEnvelope,
    ProjectCommandError, ProjectCommandSettlement, ProjectScope, RejectProposalOperationsInput,
    RejectProposalOperationsSettlement, RejectionNote, ReopenRejectedOperationsInput,
    ReopenRejectedOperationsSettlement, ReopenWithdrawnProposalInput,
    ReopenWithdrawnProposalSettlement, ReplanProposalInput, ReplanProposalSettlement,
    SetCurrentChapterInput, StructureAuthority, StructureAuthorityEvidence, StructureSettlement,
    UpdateChapterInput, UpdateChapterSettlement, UpdateProjectAssistanceInput, UpdateProjectInput,
    UpdateVolumeInput, UpdateVolumeSettlement, WithdrawProposalInput, WithdrawProposalSettlement,
    WithdrawalNote, issue_project_command_challenge,
};
use storyos_application::{
    ChapterId, EditorSessionId, IssueProjectCommandChallenge, OpenChapter, VolumeId, open_chapter,
};
use storyos_application::{
    CompleteReadyPartialProposalInput, CompleteReadyPartialProposalSettlement,
    ContinueProposalGenerationInput, ContinueProposalGenerationSettlement,
};
use storyos_application::{TakeOverProjectWriterInput, TakeOverProjectWriterSettlement};
use storyos_core::{
    AssistanceAvailability, CreateChapterPlacement, EXCLUSIVE_AUTHORITATIVE_EDGES_V1,
    OpenInlineProposalAnchor, PROSEMIRROR_TOKEN_UTF16_V1, ReasonCode, ReceiptResult,
    TransitionOutcome, proposal_anchor_base_slice_digest,
};
use tokio_postgres::error::SqlState;
use tokio_postgres::{Client, NoTls};
use uuid::Uuid;

use super::{
    ActivitySequences, Classification, CommandSpec, LockedProject, ProfileApplied,
    ProfileSequences, ProfileWrite, ProjectCommand, ReceiptRefs, ZeroAuthorityRows,
    ZeroAuthorityWrite, ZeroOutcome, settle_project_command, unavailable,
};
use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::set_current_chapter_authority_tests::{open_session, seed_two_chapters};
use crate::update_volume_tests::seed_project;

/// One admitted command envelope and its typed Manuscript Structure input.
#[derive(Clone)]
pub(crate) struct CommandCall<I> {
    pub(crate) envelope: ProjectCommandEnvelope,
    pub(crate) input: I,
}

/// Builds a command call whose ids derive from one four-digit suffix.
pub(crate) fn command_call<I>(
    binding: ProjectCommandChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
    bytes: &[u8],
    input: I,
) -> CommandCall<I> {
    CommandCall {
        envelope: ProjectCommandEnvelope {
            project_scope: binding.project_scope.clone(),
            client_binding: EditorClientBinding {
                binding_ref: binding.client_session_binding_digest.clone(),
                session_generation: binding.client_session_generation,
                client_contract_revision: binding.client_contract_revision.clone(),
                security_policy_revision: binding.security_policy_revision.clone(),
            },
            challenge_binding: binding,
            nonce_digest: nonce_digest.to_owned(),
            canonical_command_bytes: bytes.to_vec(),
            correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
            ids: AuthorCommandAdmissionIds {
                command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
                author_command_admission_id: format!(
                    "018f0000-0000-7001-8000-00000002{ids_suffix}"
                ),
                receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
            },
        },
        input,
    }
}

pub(crate) async fn create_volume(
    store: &PostgresProjectReader,
    call: &CommandCall<CreateVolumeInput>,
) -> Result<CreateVolumeSettlement, ProjectCommandError> {
    store.create_volume(&call.envelope, &call.input).await
}

pub(crate) async fn update_volume(
    store: &PostgresProjectReader,
    call: &CommandCall<UpdateVolumeInput>,
) -> Result<UpdateVolumeSettlement, ProjectCommandError> {
    store.update_volume(&call.envelope, &call.input).await
}

pub(crate) async fn delete_volume(
    store: &PostgresProjectReader,
    call: &CommandCall<DeleteVolumeInput>,
) -> Result<DeleteVolumeSettlement, ProjectCommandError> {
    store.delete_volume(&call.envelope, &call.input).await
}

pub(crate) async fn create_chapter(
    store: &PostgresProjectReader,
    call: &CommandCall<CreateChapterInput>,
) -> Result<CreateChapterSettlement, ProjectCommandError> {
    store.create_chapter(&call.envelope, &call.input).await
}

pub(crate) async fn update_chapter(
    store: &PostgresProjectReader,
    call: &CommandCall<UpdateChapterInput>,
) -> Result<UpdateChapterSettlement, ProjectCommandError> {
    store.update_chapter(&call.envelope, &call.input).await
}

pub(crate) async fn delete_chapter(
    store: &PostgresProjectReader,
    call: &CommandCall<DeleteChapterInput>,
) -> Result<DeleteChapterSettlement, ProjectCommandError> {
    store.delete_chapter(&call.envelope, &call.input).await
}

async fn take_over_project_writer(
    store: &PostgresProjectReader,
    call: &CommandCall<TakeOverProjectWriterInput>,
) -> Result<TakeOverProjectWriterSettlement, ProjectCommandError> {
    store
        .take_over_project_writer(&call.envelope, &call.input)
        .await
}

/// The applied effect and settled authority of one structure command; panics on any other outcome.
pub(crate) fn applied<A: Clone + Debug, N: Debug, C: Debug, R: Debug>(
    settlement: &StructureSettlement<A, N, C, R>,
) -> (A, StructureAuthority) {
    match &settlement.outcome {
        TransitionOutcome::Applied(applied) => match &applied.authority {
            StructureAuthorityEvidence::Settled(authority) => {
                (applied.effect.clone(), authority.clone())
            }
            StructureAuthorityEvidence::BeforeAuthorityHistoryFloor => {
                panic!("an applied structure command must write Structural Authority Settlement")
            }
        },
        other => panic!("the structure command must apply, got {other:?}"),
    }
}

const MISSING_VOLUME: &str = "018f0000-0000-7001-8000-00000000ffff";

async fn stores() -> (PostgresProjectReader, Client) {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(connection);
    (PostgresProjectReader::new(runtime_url), admin)
}

/// Counts the Receipt, Author Action, Activity, Commit, and Snapshot rows of one Receipt.
async fn settlement_rows(admin: &Client, receipt_id: &str) -> [i64; 5] {
    let row = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.domain_receipts WHERE receipt_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.author_action_entries
                      WHERE receipt_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE receipt_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE receipt_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.project_snapshots AS snapshot
                       JOIN storyos.project_activity_event_payloads AS payload
                         ON (payload.owner_user_id, payload.project_id,
                             payload.project_activity_position) =
                            (snapshot.owner_user_id, snapshot.project_id,
                             snapshot.project_activity_position)
                      WHERE payload.receipt_id = $1::text::uuid)",
            &[&receipt_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4].map(|index| row.get(index))
}

/// The public route identity of one project command kind.
struct Route {
    kind: &'static str,
    method: &'static str,
    path: &'static str,
    schema: &'static str,
}

const CREATE_VOLUME: Route = Route {
    kind: "createVolume",
    method: storyos_contracts::CREATE_VOLUME_METHOD,
    path: storyos_contracts::CREATE_VOLUME_PATH,
    schema: storyos_contracts::CREATE_VOLUME_REQUEST_SCHEMA_ID,
};
const UPDATE_VOLUME: Route = Route {
    kind: "updateVolume",
    method: storyos_contracts::UPDATE_VOLUME_METHOD,
    path: storyos_contracts::UPDATE_VOLUME_PATH,
    schema: storyos_contracts::UPDATE_VOLUME_REQUEST_SCHEMA_ID,
};
const DELETE_VOLUME: Route = Route {
    kind: "deleteVolume",
    method: storyos_contracts::DELETE_VOLUME_METHOD,
    path: storyos_contracts::DELETE_VOLUME_PATH,
    schema: storyos_contracts::DELETE_VOLUME_REQUEST_SCHEMA_ID,
};
const CREATE_CHAPTER: Route = Route {
    kind: "createChapter",
    method: storyos_contracts::CREATE_CHAPTER_METHOD,
    path: storyos_contracts::CREATE_CHAPTER_PATH,
    schema: storyos_contracts::CREATE_CHAPTER_REQUEST_SCHEMA_ID,
};
const UPDATE_CHAPTER: Route = Route {
    kind: "updateChapter",
    method: storyos_contracts::UPDATE_CHAPTER_METHOD,
    path: storyos_contracts::UPDATE_CHAPTER_PATH,
    schema: storyos_contracts::UPDATE_CHAPTER_REQUEST_SCHEMA_ID,
};
const UPDATE_PROJECT_ASSISTANCE: Route = Route {
    kind: "updateProjectAssistance",
    method: storyos_contracts::UPDATE_PROJECT_ASSISTANCE_METHOD,
    path: storyos_contracts::UPDATE_PROJECT_ASSISTANCE_PATH,
    schema: storyos_contracts::UPDATE_PROJECT_ASSISTANCE_REQUEST_SCHEMA_ID,
};
const TAKE_OVER_PROJECT_WRITER: Route = Route {
    kind: "takeOverProjectWriter",
    method: storyos_contracts::TAKE_OVER_PROJECT_WRITER_METHOD,
    path: storyos_contracts::TAKE_OVER_PROJECT_WRITER_PATH,
    schema: storyos_contracts::TAKE_OVER_PROJECT_WRITER_REQUEST_SCHEMA_ID,
};
const SET_CURRENT_CHAPTER: Route = Route {
    kind: "setCurrentChapter",
    method: storyos_contracts::SET_CURRENT_CHAPTER_METHOD,
    path: storyos_contracts::SET_CURRENT_CHAPTER_PATH,
    schema: storyos_contracts::SET_CURRENT_CHAPTER_REQUEST_SCHEMA_ID,
};
const DELETE_CHAPTER: Route = Route {
    kind: "deleteChapter",
    method: storyos_contracts::DELETE_CHAPTER_METHOD,
    path: storyos_contracts::DELETE_CHAPTER_PATH,
    schema: storyos_contracts::DELETE_CHAPTER_REQUEST_SCHEMA_ID,
};
const UPDATE_PROJECT: Route = Route {
    kind: "updateProject",
    method: storyos_contracts::UPDATE_PROJECT_METHOD,
    path: storyos_contracts::UPDATE_PROJECT_PATH,
    schema: storyos_contracts::UPDATE_PROJECT_REQUEST_SCHEMA_ID,
};
const ARCHIVE_PROJECT: Route = Route {
    kind: "archiveProject",
    method: storyos_contracts::ARCHIVE_PROJECT_METHOD,
    path: storyos_contracts::ARCHIVE_PROJECT_PATH,
    schema: storyos_contracts::ARCHIVE_PROJECT_REQUEST_SCHEMA_ID,
};
const REOPEN_WITHDRAWN_PROPOSAL: Route = Route {
    kind: "reopenWithdrawnProposal",
    method: storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_METHOD,
    path: storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_PATH,
    schema: storyos_contracts::REOPEN_WITHDRAWN_PROPOSAL_REQUEST_SCHEMA_ID,
};
const REJECT_PROPOSAL_OPERATIONS: Route = Route {
    kind: "rejectProposalOperations",
    method: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_METHOD,
    path: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_PATH,
    schema: storyos_contracts::REJECT_PROPOSAL_OPERATIONS_REQUEST_SCHEMA_ID,
};
const REPLAN_PROPOSAL: Route = Route {
    kind: "replanProposal",
    method: storyos_contracts::REPLAN_PROPOSAL_METHOD,
    path: storyos_contracts::REPLAN_PROPOSAL_PATH,
    schema: storyos_contracts::REPLAN_PROPOSAL_REQUEST_SCHEMA_ID,
};
const REOPEN_REJECTED_OPERATIONS: Route = Route {
    kind: "reopenRejectedOperations",
    method: storyos_contracts::REOPEN_REJECTED_OPERATIONS_METHOD,
    path: storyos_contracts::REOPEN_REJECTED_OPERATIONS_PATH,
    schema: storyos_contracts::REOPEN_REJECTED_OPERATIONS_REQUEST_SCHEMA_ID,
};
const WITHDRAW_PROPOSAL: Route = Route {
    kind: "withdrawProposal",
    method: storyos_contracts::WITHDRAW_PROPOSAL_METHOD,
    path: storyos_contracts::WITHDRAW_PROPOSAL_PATH,
    schema: storyos_contracts::WITHDRAW_PROPOSAL_REQUEST_SCHEMA_ID,
};
const CLOSE_EDITOR_FLOW_DRAFT: Route = Route {
    kind: "closeEditorFlowDraft",
    method: "POST",
    path: storyos_contracts::CLOSE_EDITOR_FLOW_DRAFT_PATH,
    schema: storyos_contracts::CLOSE_EDITOR_FLOW_DRAFT_REQUEST_SCHEMA_ID,
};

const EXPAND_REFUSED_EDIT_DRAFT: Route = Route {
    kind: "expandRefusedEditDraftToProposal",
    method: "POST",
    path: storyos_contracts::EXPAND_REFUSED_EDIT_DRAFT_PATH,
    schema: storyos_contracts::EXPAND_REFUSED_EDIT_DRAFT_REQUEST_SCHEMA_ID,
};
const COMPLETE_READY_PARTIAL_PROPOSAL: Route = Route {
    kind: "completeReadyPartialProposal",
    method: storyos_contracts::COMPLETE_READY_PARTIAL_PROPOSAL_METHOD,
    path: storyos_contracts::COMPLETE_READY_PARTIAL_PROPOSAL_PATH,
    schema: storyos_contracts::COMPLETE_READY_PARTIAL_PROPOSAL_REQUEST_SCHEMA_ID,
};
const CONTINUE_PROPOSAL_GENERATION: Route = Route {
    kind: "continueProposalGeneration",
    method: storyos_contracts::CONTINUE_PROPOSAL_GENERATION_METHOD,
    path: storyos_contracts::CONTINUE_PROPOSAL_GENERATION_PATH,
    schema: storyos_contracts::CONTINUE_PROPOSAL_GENERATION_REQUEST_SCHEMA_ID,
};

/// Issues one Command Challenge for `route` and binds `input` to it.
async fn issued<I>(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    route: &Route,
    input: I,
) -> CommandCall<I> {
    let suffix = format!("{suffix:04x}");
    let issue = IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: scope.clone(),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: "storyos.web-client.release-1.v3".to_owned(),
            security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: route.method.to_owned(),
            route_template: route.path.to_owned(),
            command_schema: route.schema.to_owned(),
            command_kind: route.kind.to_owned(),
            canonical_command_digest: format!("sha256:{}:{suffix}", route.kind),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        },
        nonce: format!("opaque-nonce-{suffix}"),
        nonce_digest: format!("sha256:nonce-{suffix}"),
    };
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    command_call(issue.binding, &issue.nonce_digest, &suffix, b"{}", input)
}

/// Settles one call, replays it with new request identities, and requires an equal settlement.
///
/// Returns the stored Receipt result kind and the authority rows of the first settlement.
async fn replayed_outcome<I: Clone, A, N, C, R, P, Z>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<I>,
    settle: impl AsyncFn(
        &PostgresProjectReader,
        &CommandCall<I>,
    ) -> Result<ProjectCommandSettlement<A, N, C, R, P, Z>, ProjectCommandError>,
) -> (String, [i64; 5])
where
    A: Debug + PartialEq,
    N: ReasonCode + Debug + PartialEq,
    C: ReasonCode + Debug + PartialEq,
    R: ReasonCode + Debug + PartialEq,
    P: Debug + PartialEq,
    Z: Debug + PartialEq,
{
    let first = settle(store, call).await.unwrap();
    let mut retry = call.clone();
    retry.envelope.ids = AuthorCommandAdmissionIds {
        command_id: Uuid::now_v7().to_string(),
        author_command_admission_id: Uuid::now_v7().to_string(),
        receipt_id: Uuid::now_v7().to_string(),
    };
    assert_eq!(settle(store, &retry).await.unwrap(), first);
    let result_kind = admin
        .query_one(
            "SELECT result_kind FROM storyos.domain_receipts WHERE receipt_id = $1::text::uuid",
            &[&first.ids.receipt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    (
        result_kind,
        settlement_rows(admin, &first.ids.receipt_id).await,
    )
}

async fn new_volume(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    expected_tree_revision: u64,
) -> VolumeId {
    let input = CreateVolumeInput {
        title: format!("Volume {suffix:04x}"),
        expected_tree_revision,
    };
    let call = issued(store, scope, suffix, &CREATE_VOLUME, input).await;
    VolumeId::new(
        applied(&create_volume(store, &call).await.unwrap())
            .0
            .volume_id,
    )
}

async fn new_chapter(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    volume_id: &VolumeId,
    expected_tree_revision: u64,
) -> ChapterId {
    let input = CreateChapterInput {
        volume_id: volume_id.as_ref().to_owned(),
        title: format!("Chapter {suffix:04x}"),
        placement: CreateChapterPlacement::Append,
        expected_tree_revision,
    };
    let call = issued(store, scope, suffix, &CREATE_CHAPTER, input).await;
    ChapterId::new(
        applied(&create_chapter(store, &call).await.unwrap())
            .0
            .chapter_id,
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_outcome_replays_its_first_settlement_and_writes_only_its_profile_records() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let missing_volume = VolumeId::new(MISSING_VOLUME);
    let missing_chapter = ChapterId::new(MISSING_VOLUME);
    let mut observed = Vec::new();

    let scope = seed_project(&store, "5d00").await;
    for (suffix, title, expected_tree_revision) in [
        (0x5d01, "Volume", 1),
        (0x5d02, "Volume", 1),
        (0x5d03, "", 2),
    ] {
        let input = CreateVolumeInput {
            title: title.to_owned(),
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &CREATE_VOLUME, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, create_volume).await;
        observed.push((CREATE_VOLUME.kind, outcome));
    }

    let scope = seed_project(&store, "5d10").await;
    let volume = new_volume(&store, &scope, 0x5d11, 1).await;
    for (suffix, volume_id, expected_tree_revision) in [
        (0x5d12, &volume, 2),
        (0x5d13, &volume, 3),
        (0x5d14, &volume, 2),
        (0x5d15, &missing_volume, 3),
    ] {
        let input = UpdateVolumeInput {
            volume_id: volume_id.clone(),
            title: "Renamed".to_owned(),
            order: 1,
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &UPDATE_VOLUME, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, update_volume).await;
        observed.push((UPDATE_VOLUME.kind, outcome));
    }

    let scope = seed_project(&store, "5d20").await;
    let volume = new_volume(&store, &scope, 0x5d21, 1).await;
    for (suffix, volume_id, expected_tree_revision) in [
        (0x5d22, &volume, 2),
        (0x5d23, &volume, 3),
        (0x5d24, &volume, 2),
        (0x5d25, &missing_volume, 3),
    ] {
        let input = DeleteVolumeInput {
            volume_id: volume_id.clone(),
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &DELETE_VOLUME, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, delete_volume).await;
        observed.push((DELETE_VOLUME.kind, outcome));
    }

    let scope = seed_project(&store, "5d30").await;
    let volume = new_volume(&store, &scope, 0x5d31, 1).await;
    for (suffix, volume_id, expected_tree_revision) in [
        (0x5d32, &volume, 2),
        (0x5d33, &volume, 2),
        (0x5d34, &missing_volume, 3),
    ] {
        let input = CreateChapterInput {
            volume_id: volume_id.as_ref().to_owned(),
            title: "Chapter".to_owned(),
            placement: CreateChapterPlacement::Append,
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &CREATE_CHAPTER, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, create_chapter).await;
        observed.push((CREATE_CHAPTER.kind, outcome));
    }

    let scope = seed_project(&store, "5d40").await;
    let volume = new_volume(&store, &scope, 0x5d41, 1).await;
    let chapter = new_chapter(&store, &scope, 0x5d42, &volume, 2).await;
    for (suffix, chapter_id, expected_tree_revision) in [
        (0x5d43, &chapter, 3),
        (0x5d44, &chapter, 4),
        (0x5d45, &chapter, 3),
        (0x5d46, &missing_chapter, 4),
    ] {
        let input = UpdateChapterInput {
            chapter_id: chapter_id.clone(),
            title: "Renamed".to_owned(),
            order: 1,
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &UPDATE_CHAPTER, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, update_chapter).await;
        observed.push((UPDATE_CHAPTER.kind, outcome));
    }

    let scope = seed_project(&store, "5d50").await;
    let volume = new_volume(&store, &scope, 0x5d51, 1).await;
    new_chapter(&store, &scope, 0x5d52, &volume, 2).await;
    let chapter = new_chapter(&store, &scope, 0x5d53, &volume, 3).await;
    for (suffix, chapter_id, expected_tree_revision) in [
        (0x5d54, &chapter, 4),
        (0x5d55, &chapter, 5),
        (0x5d56, &chapter, 4),
        (0x5d57, &missing_chapter, 5),
    ] {
        let input = DeleteChapterInput {
            chapter_id: chapter_id.clone(),
            expected_tree_revision,
        };
        let call = issued(&store, &scope, suffix, &DELETE_CHAPTER, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, delete_chapter).await;
        observed.push((DELETE_CHAPTER.kind, outcome));
    }

    let scope = seed_project(&store, "5d60").await;
    for (suffix, title, expected_revision) in [
        (0x5d61, "Renamed", 1),
        (0x5d62, "Renamed", 2),
        (0x5d63, "Other", 1),
    ] {
        let input = UpdateProjectInput {
            title: title.to_owned(),
            expected_revision,
        };
        let call = issued(&store, &scope, suffix, &UPDATE_PROJECT, input).await;
        let outcome = replayed_outcome(
            &store,
            &admin,
            &call,
            async |store: &PostgresProjectReader, call: &CommandCall<UpdateProjectInput>| {
                store.update_project(&call.envelope, &call.input).await
            },
        )
        .await;
        observed.push((UPDATE_PROJECT.kind, outcome));
    }

    let scope = seed_project(&store, "5d70").await;
    for (suffix, expected_revision) in [(0x5d71, 2), (0x5d72, 1), (0x5d73, 2)] {
        let input = ArchiveProjectInput { expected_revision };
        let call = issued(&store, &scope, suffix, &ARCHIVE_PROJECT, input).await;
        let outcome = replayed_outcome(
            &store,
            &admin,
            &call,
            async |store: &PostgresProjectReader, call: &CommandCall<ArchiveProjectInput>| {
                store.archive_project(&call.envelope, &call.input).await
            },
        )
        .await;
        observed.push((ARCHIVE_PROJECT.kind, outcome));
    }

    let (scope, chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(&store, /*base*/ 0x5d80).await;
    for (suffix, chapter_id, expected_current_chapter_id) in [
        (0x5d89, &chapter_b, &chapter_a),
        (0x5d8a, &chapter_b, &chapter_b),
        (0x5d8b, &chapter_a, &chapter_a),
        (0x5d8c, &MISSING_VOLUME.to_owned(), &chapter_b),
    ] {
        let input = SetCurrentChapterInput {
            editor_session_id: EditorSessionId::new(editor_session_id.clone()),
            chapter_id: chapter_id.clone(),
            expected_current_chapter_id: expected_current_chapter_id.clone(),
            expected_target_revision_id: revision_b.clone(),
        };
        let call = issued(&store, &scope, suffix, &SET_CURRENT_CHAPTER, input).await;
        let outcome = replayed_outcome(
            &store,
            &admin,
            &call,
            async |store: &PostgresProjectReader, call: &CommandCall<SetCurrentChapterInput>| {
                store.set_current_chapter(&call.envelope, &call.input).await
            },
        )
        .await;
        observed.push((SET_CURRENT_CHAPTER.kind, outcome));
    }

    let scope = seed_project(&store, "5d90").await;
    for (suffix, availability, expected_revision) in [
        (0x5d91, AssistanceAvailability::Available, 0),
        (0x5d92, AssistanceAvailability::Available, 1),
        (0x5d93, AssistanceAvailability::Unavailable, 0),
        (0x5d94, AssistanceAvailability::Unavailable, 1),
    ] {
        let input = UpdateProjectAssistanceInput {
            availability,
            expected_revision,
        };
        let call = issued(&store, &scope, suffix, &UPDATE_PROJECT_ASSISTANCE, input).await;
        let outcome = replayed_outcome(
            &store,
            &admin,
            &call,
            async |store: &PostgresProjectReader,
                   call: &CommandCall<UpdateProjectAssistanceInput>| {
                store
                    .update_project_assistance(&call.envelope, &call.input)
                    .await
            },
        )
        .await;
        observed.push((UPDATE_PROJECT_ASSISTANCE.kind, outcome));
    }

    let call = take_over_project_writer_call(&store, /*base*/ 0x5da0).await;
    let outcome = replayed_outcome(&store, &admin, &call, take_over_project_writer).await;
    observed.push((TAKE_OVER_PROJECT_WRITER.kind, outcome));

    let (scope, withdrawn) = withdrawn_proposal(&store, &admin, /*base*/ 0x5db0).await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x5db9,
        &REOPEN_WITHDRAWN_PROPOSAL,
        withdrawn.clone(),
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, reopen_withdrawn_proposal).await;
    observed.push((REOPEN_WITHDRAWN_PROPOSAL.kind, outcome));
    let reopened_revision_id: String = admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.proposal_heads
              WHERE proposal_id = $1::text::uuid",
            &[&withdrawn.proposal_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    for (suffix, proposal_revision_id) in [
        (0x5dba, reopened_revision_id),
        (0x5dbb, withdrawn.proposal_revision_id.clone()),
    ] {
        let input = ReopenWithdrawnProposalInput {
            proposal_revision_id,
            ..withdrawn.clone()
        };
        let call = issued(&store, &scope, suffix, &REOPEN_WITHDRAWN_PROPOSAL, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, reopen_withdrawn_proposal).await;
        observed.push((REOPEN_WITHDRAWN_PROPOSAL.kind, outcome));
    }
    let (scope, withdrawn) = withdrawn_proposal(&store, &admin, /*base*/ 0x5dc0).await;
    move_head_away(
        &admin,
        &scope,
        &withdrawn.expected_authoritative_revision_id,
    )
    .await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x5dc9,
        &REOPEN_WITHDRAWN_PROPOSAL,
        withdrawn,
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, reopen_withdrawn_proposal).await;
    observed.push((REOPEN_WITHDRAWN_PROPOSAL.kind, outcome));

    let mut rejection_records = Vec::new();
    let (scope, pending) = pending_proposal(&store, &admin, /*base*/ 0x7b00).await;
    let stale = RejectProposalOperationsInput {
        proposal_revision_id: Uuid::now_v7().to_string(),
        ..pending.clone()
    };
    let (changed_scope, changed) = pending_proposal(&store, &admin, /*base*/ 0x7b10).await;
    move_head_away(
        &admin,
        &changed_scope,
        &changed.expected_authoritative_revision_id,
    )
    .await;
    for (scope, suffix, input) in [
        (&scope, 0x7b09, pending.clone()),
        (&scope, 0x7b0a, pending),
        (&scope, 0x7b0b, stale),
        (&changed_scope, 0x7b19, changed),
    ] {
        let call = issued(&store, scope, suffix, &REJECT_PROPOSAL_OPERATIONS, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, reject_proposal_operations).await;
        observed.push((REJECT_PROPOSAL_OPERATIONS.kind, outcome));
        rejection_records.push(rejection_records_of(&admin, &call).await);
    }

    // The second exact call names a Proposal Revision that the first call superseded.
    let (scope, conflicted) = conflicted_proposal(&store, &admin, /*base*/ 0x7200).await;
    for suffix in [0x7209, 0x720a] {
        let call = issued(&store, &scope, suffix, &REPLAN_PROPOSAL, conflicted.clone()).await;
        let outcome = replayed_outcome(&store, &admin, &call, replan_proposal).await;
        observed.push((REPLAN_PROPOSAL.kind, outcome));
    }
    let (scope, conflicted) = conflicted_proposal(&store, &admin, /*base*/ 0x7210).await;
    move_head_away(
        &admin,
        &scope,
        &conflicted.expected_authoritative_revision_id,
    )
    .await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x7219,
        &REPLAN_PROPOSAL,
        conflicted,
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, replan_proposal).await;
    observed.push((REPLAN_PROPOSAL.kind, outcome));
    let (scope, rejected) = rejected_operation(&store, &admin, /*base*/ 0x7220).await;
    for suffix in [0x7229, 0x722a] {
        let call = issued(
            &store,
            &scope,
            suffix,
            &REOPEN_REJECTED_OPERATIONS,
            rejected.clone(),
        )
        .await;
        let outcome = replayed_outcome(&store, &admin, &call, reopen_rejected_operations).await;
        observed.push((REOPEN_REJECTED_OPERATIONS.kind, outcome));
    }
    let (scope, rejected) = rejected_operation(&store, &admin, /*base*/ 0x7230).await;
    move_head_away(&admin, &scope, &rejected.expected_authoritative_revision_id).await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x7239,
        &REOPEN_REJECTED_OPERATIONS,
        rejected,
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, reopen_rejected_operations).await;
    observed.push((REOPEN_REJECTED_OPERATIONS.kind, outcome));

    let (scope, open) = withdrawable_proposal(&store, &admin, /*base*/ 0x7440).await;
    let mut withdrawal_receipts = Vec::new();
    for (suffix, proposal_revision_id) in [
        (0x7449, open.proposal_revision_id.clone()),
        (0x744a, open.proposal_revision_id.clone()),
        (0x744b, Uuid::now_v7().to_string()),
    ] {
        let input = WithdrawProposalInput {
            proposal_revision_id,
            ..open.clone()
        };
        let call = issued(&store, &scope, suffix, &WITHDRAW_PROPOSAL, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, withdraw_proposal).await;
        observed.push((WITHDRAW_PROPOSAL.kind, outcome));
        withdrawal_receipts.push(call.envelope.ids.receipt_id);
    }
    let (scope, open) = withdrawable_proposal(&store, &admin, /*base*/ 0x7450).await;
    move_head_away(&admin, &scope, &open.expected_authoritative_revision_id).await;
    let call = issued(
        &store,
        &scope,
        /*suffix*/ 0x7459,
        &WITHDRAW_PROPOSAL,
        open,
    )
    .await;
    let outcome = replayed_outcome(&store, &admin, &call, withdraw_proposal).await;
    observed.push((WITHDRAW_PROPOSAL.kind, outcome));
    withdrawal_receipts.push(call.envelope.ids.receipt_id);
    let mut withdrawal_records = Vec::new();
    for receipt_id in &withdrawal_receipts {
        let records: i64 = admin
            .query_one(
                "SELECT count(*) FROM storyos.proposal_withdrawals
                  WHERE withdrawal_receipt_id = $1::text::uuid",
                &[receipt_id],
            )
            .await
            .unwrap()
            .get(/*idx*/ 0);
        withdrawal_records.push(records);
    }
    let (scope, open) = refused_edit_draft(&store, &admin, /*base*/ 0x7d00, "retained").await;
    let changed = CloseEditorFlowDraftInput {
        source_current_draft_revision_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let (archived_scope, archived) =
        refused_edit_draft(&store, &admin, /*base*/ 0x7d10, "archived").await;
    for (scope, suffix, input) in [
        (&scope, 0x7d09, open.clone()),
        (&scope, 0x7d0a, open),
        (&scope, 0x7d0b, changed),
        (&archived_scope, 0x7d19, archived),
    ] {
        let call = discard_call(&store, scope, suffix, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, close_editor_flow_draft).await;
        observed.push((CLOSE_EDITOR_FLOW_DRAFT.kind, outcome));
    }
    let (scope, open) = refused_edit_expansion(&store, &admin, /*base*/ 0x8e00, "retained").await;
    let changed = ExpandRefusedEditDraftToProposalInput {
        source_current_draft_revision_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let missing_target_ref = Uuid::now_v7().to_string();
    let missing_target = ExpandRefusedEditDraftToProposalInput {
        target_ref: missing_target_ref.clone(),
        anchor: OpenInlineProposalAnchor {
            manuscript_block_id: missing_target_ref,
            ..open.anchor.clone()
        },
        ..open.clone()
    };
    let (archived_scope, archived) =
        refused_edit_expansion(&store, &admin, /*base*/ 0x8e10, "archived").await;
    let mut expansion_records = Vec::new();
    for (scope, suffix, input) in [
        (&scope, 0x8e09, open.clone()),
        (&scope, 0x8e0a, changed),
        (&scope, 0x8e0b, missing_target),
        (&archived_scope, 0x8e19, archived),
    ] {
        let call = expansion_call(&store, scope, suffix, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, expand_refused_edit_draft).await;
        observed.push((EXPAND_REFUSED_EDIT_DRAFT.kind, outcome));
        expansion_records.push(expansion_records_of(&admin, &call).await);
    }

    let (scope, generation) = ready_partial_proposal(&store, &admin, /*base*/ 0x7500).await;
    for (suffix, expected_authoritative_revision_id) in [
        (0x750a, &generation.other_chapter_revision_id),
        (
            0x750b,
            &generation.complete.expected_authoritative_revision_id,
        ),
        (
            0x750c,
            &generation.complete.expected_authoritative_revision_id,
        ),
    ] {
        let input = CompleteReadyPartialProposalInput {
            expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
            ..generation.complete.clone()
        };
        let call = issued(
            &store,
            &scope,
            suffix,
            &COMPLETE_READY_PARTIAL_PROPOSAL,
            input,
        )
        .await;
        let outcome =
            replayed_outcome(&store, &admin, &call, complete_ready_partial_proposal).await;
        observed.push((COMPLETE_READY_PARTIAL_PROPOSAL.kind, outcome));
    }
    for (suffix, expected_authoritative_revision_id) in [
        (0x750d, &generation.other_chapter_revision_id),
        (
            0x750e,
            &generation.complete.expected_authoritative_revision_id,
        ),
        (
            0x750f,
            &generation.complete.expected_authoritative_revision_id,
        ),
    ] {
        let input = ContinueProposalGenerationInput {
            expected_generation_state: "ready".to_owned(),
            expected_authoritative_revision_id: expected_authoritative_revision_id.clone(),
            ..generation.continuation.clone()
        };
        let call = issued(&store, &scope, suffix, &CONTINUE_PROPOSAL_GENERATION, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, continue_proposal_generation).await;
        observed.push((CONTINUE_PROPOSAL_GENERATION.kind, outcome));
    }

    // Receipt, Author Action, Activity, Commit, and Snapshot rows of each outcome.
    let structural_applied = ("authoritative_applied", [1, 1, 1, 1, 1]);
    let chapter_selection_applied = ("authoritative_applied", [1, 1, 1, 0, 1]);
    let activity_applied = ("authoritative_applied", [1, 0, 1, 0, 0]);
    let proposal_revised = ("proposal_revised", [1, 1, 0, 0, 0]);
    let proposal_closure_changed = ("proposal_closure_changed", [1, 1, 0, 0, 0]);
    let operations_resolved = ("proposal_operations_resolved", [1, 1, 0, 0, 0]);
    let generation_completed = ("proposal_generation_completed", [1, 1, 0, 0, 0]);
    let generation_started = ("proposal_generation_started", [1, 1, 0, 0, 0]);
    let draft_closure_changed = ("draft_closure_changed", [1, 1, 0, 0, 0]);
    let proposal_created_from_draft = ("proposal_created_from_draft", [1, 1, 0, 0, 0]);
    let no_effect = ("no_effect", [1, 0, 0, 0, 0]);
    let writer_takeover = ("no_effect", [1, 0, 1, 0, 1]);
    let conflicted = ("conflicted", [1, 0, 0, 0, 0]);
    let refused = ("refused", [1, 0, 0, 0, 0]);
    assert_eq!(
        observed
            .iter()
            .map(|(kind, (result_kind, rows))| (*kind, (result_kind.as_str(), *rows)))
            .collect::<Vec<_>>(),
        vec![
            ("createVolume", structural_applied),
            ("createVolume", conflicted),
            ("createVolume", refused),
            ("updateVolume", structural_applied),
            ("updateVolume", no_effect),
            ("updateVolume", conflicted),
            ("updateVolume", refused),
            ("deleteVolume", structural_applied),
            ("deleteVolume", no_effect),
            ("deleteVolume", conflicted),
            ("deleteVolume", refused),
            ("createChapter", structural_applied),
            ("createChapter", conflicted),
            ("createChapter", refused),
            ("updateChapter", structural_applied),
            ("updateChapter", no_effect),
            ("updateChapter", conflicted),
            ("updateChapter", refused),
            ("deleteChapter", structural_applied),
            ("deleteChapter", no_effect),
            ("deleteChapter", conflicted),
            ("deleteChapter", refused),
            ("updateProject", activity_applied),
            ("updateProject", no_effect),
            ("updateProject", conflicted),
            ("archiveProject", conflicted),
            ("archiveProject", activity_applied),
            ("archiveProject", no_effect),
            ("setCurrentChapter", chapter_selection_applied),
            ("setCurrentChapter", no_effect),
            ("setCurrentChapter", conflicted),
            ("setCurrentChapter", refused),
            ("updateProjectAssistance", activity_applied),
            ("updateProjectAssistance", no_effect),
            ("updateProjectAssistance", conflicted),
            ("updateProjectAssistance", activity_applied),
            ("takeOverProjectWriter", writer_takeover),
            ("reopenWithdrawnProposal", proposal_revised),
            ("reopenWithdrawnProposal", no_effect),
            ("reopenWithdrawnProposal", refused),
            ("reopenWithdrawnProposal", conflicted),
            ("rejectProposalOperations", operations_resolved),
            ("rejectProposalOperations", refused),
            ("rejectProposalOperations", refused),
            ("rejectProposalOperations", conflicted),
            ("replanProposal", proposal_revised),
            ("replanProposal", refused),
            ("replanProposal", conflicted),
            ("reopenRejectedOperations", proposal_revised),
            ("reopenRejectedOperations", refused),
            ("reopenRejectedOperations", conflicted),
            ("withdrawProposal", proposal_closure_changed),
            ("withdrawProposal", no_effect),
            ("withdrawProposal", refused),
            ("withdrawProposal", conflicted),
            ("closeEditorFlowDraft", draft_closure_changed),
            ("closeEditorFlowDraft", refused),
            ("closeEditorFlowDraft", conflicted),
            ("closeEditorFlowDraft", refused),
            (
                "expandRefusedEditDraftToProposal",
                proposal_created_from_draft
            ),
            ("expandRefusedEditDraftToProposal", conflicted),
            ("expandRefusedEditDraftToProposal", refused),
            ("expandRefusedEditDraftToProposal", refused),
            ("completeReadyPartialProposal", conflicted),
            ("completeReadyPartialProposal", generation_completed),
            ("completeReadyPartialProposal", refused),
            ("continueProposalGeneration", conflicted),
            ("continueProposalGeneration", generation_started),
            ("continueProposalGeneration", refused),
        ]
    );
    assert_eq!(rejection_records, vec![1; 4]);
    assert_eq!(withdrawal_records, vec![1, 0, 0, 0]);
    assert_eq!(expansion_records, vec![[1, 1, 1], [0; 3], [0; 3], [0; 3]]);
}

/// A new Project with Chapters A and B, Chapter A current, and one writer Editor Session.
///
/// Returns the Scope, both Chapters, the head of Chapter B, and the Editor Session.
async fn two_chapter_writer(
    store: &PostgresProjectReader,
    base: u16,
) -> (ProjectScope, String, String, String, String) {
    let suffix = |offset: u16| format!("{:04x}", base + offset);
    let (scope, _volume_id, chapter_a, chapter_b) = seed_two_chapters(
        store,
        "018f0000-0000-7001-8000-000000000001",
        &suffix(/*offset*/ 0),
        &suffix(/*offset*/ 1),
        &suffix(/*offset*/ 2),
        &suffix(/*offset*/ 3),
    )
    .await;
    let editor_session_id = open_session(store, &scope, &suffix(/*offset*/ 4)).await;
    let OpenChapter::Found(opened) =
        open_chapter(store, &scope, &ChapterId::new(chapter_b.clone()))
            .await
            .unwrap()
    else {
        panic!("Chapter B must open");
    };
    let revision_b = opened.chapter.revision_id.as_ref().to_owned();
    (scope, chapter_a, chapter_b, revision_b, editor_session_id)
}

/// One applicable writer takeover by a second Editor Session in a new Project.
async fn take_over_project_writer_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<TakeOverProjectWriterInput> {
    let (scope, ..) = two_chapter_writer(store, base).await;
    let observer = open_session(store, &scope, &format!("{:04x}", base + 5)).await;
    let input = TakeOverProjectWriterInput {
        editor_session_id: EditorSessionId::new(observer),
        observed_writer_generation: 1,
        editor_contract_revision: storyos_contracts::EDITOR_CONTRACT_REVISION.to_owned(),
    };
    issued(store, &scope, base + 9, &TAKE_OVER_PROJECT_WRITER, input).await
}

/// One applicable first Update Project Assistance in a new Project.
async fn update_project_assistance_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateProjectAssistanceInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = UpdateProjectAssistanceInput {
        availability: AssistanceAvailability::Available,
        expected_revision: 0,
    };
    issued(store, &scope, base + 9, &UPDATE_PROJECT_ASSISTANCE, input).await
}

/// One applicable Set Current Chapter from Chapter A to Chapter B in a new Project.
async fn set_current_chapter_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<SetCurrentChapterInput> {
    let (scope, chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let input = SetCurrentChapterInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        chapter_id: chapter_b,
        expected_current_chapter_id: chapter_a,
        expected_target_revision_id: revision_b,
    };
    issued(store, &scope, base + 9, &SET_CURRENT_CHAPTER, input).await
}

async fn reopen_withdrawn_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<ReopenWithdrawnProposalInput>,
) -> Result<ReopenWithdrawnProposalSettlement, ProjectCommandError> {
    store
        .reopen_withdrawn_proposal(&call.envelope, &call.input)
        .await
}

/// A new Project with a writer Editor Session and one withdrawn Proposal on Chapter B.
///
/// Returns the Scope and the input that reopens the Proposal. The fixture skips the foreign
/// keys of the AgentRun, the Manuscript Block, and the withdrawal Receipt.
async fn withdrawn_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, ReopenWithdrawnProposalInput) {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let input = ReopenWithdrawnProposalInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        proposal_id: Uuid::now_v7().to_string(),
        proposal_revision_id: Uuid::now_v7().to_string(),
        withdrawal_event_id: Uuid::now_v7().to_string(),
        expected_authoritative_revision_id: revision_b,
    };
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'block_edit', '{chapter}',
                     '{block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready', 'valid',
                     'withdrawn', 'Candidate', '{base_revision}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'resolved', 'Candidate');
             INSERT INTO storyos.proposal_withdrawals
               (owner_user_id, project_id, withdrawal_event_id, proposal_id,
                proposal_revision_id, withdrawal_reason, withdrawal_receipt_id)
             VALUES ('{owner}', '{project}', '{event}', '{proposal}', '{revision}',
                     'current_producer_withdrew', '{receipt}');
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = input.proposal_id,
            chapter = chapter_b,
            block = Uuid::now_v7(),
            run = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = input.proposal_revision_id,
            base_revision = input.expected_authoritative_revision_id,
            operation = Uuid::now_v7(),
            event = input.withdrawal_event_id,
            receipt = Uuid::now_v7(),
        ))
        .await
        .unwrap();
    (scope, input)
}

/// One applicable Reopen Withdrawn Proposal in a new Project.
async fn reopen_withdrawn_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ReopenWithdrawnProposalInput> {
    let (scope, input) = withdrawn_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &REOPEN_WITHDRAWN_PROPOSAL, input).await
}

/// Moves the Chapter head away from `revision`, so that a command that expects it conflicts.
async fn move_head_away(admin: &Client, scope: &ProjectScope, revision: &str) {
    run_without_foreign_keys(
        admin,
        &format!(
            "UPDATE storyos.authoritative_heads AS head
                SET current_revision_id = other.current_revision_id
               FROM storyos.authoritative_heads AS other
              WHERE (head.owner_user_id, head.project_id) = (other.owner_user_id, other.project_id)
                AND head.current_revision_id = '{revision}'
                AND other.current_revision_id <> '{revision}'
                AND head.project_id = '{project}'",
            project = scope.project_id.as_ref(),
        ),
    )
    .await;
}

async fn reject_proposal_operations(
    store: &PostgresProjectReader,
    call: &CommandCall<RejectProposalOperationsInput>,
) -> Result<RejectProposalOperationsSettlement, ProjectCommandError> {
    store
        .reject_proposal_operations(&call.envelope, &call.input)
        .await
}

/// A new Project with a writer Editor Session and one open Proposal with one pending Operation.
///
/// Returns the Scope and the input that rejects the Operation. The fixture skips the foreign
/// keys of the AgentRun and the Manuscript Block.
async fn pending_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, RejectProposalOperationsInput) {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let input = RejectProposalOperationsInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        proposal_id: Uuid::now_v7().to_string(),
        proposal_revision_id: Uuid::now_v7().to_string(),
        selected_pending_operation_ids: vec![Uuid::now_v7().to_string()],
        expected_authoritative_revision_id: revision_b,
        rejection_note: RejectionNote::Present {
            text: "Too formal".to_owned(),
        },
    };
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'block_edit', '{chapter}',
                     '{block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready', 'valid',
                     'open', 'Candidate', '{base_revision}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'unresolved', 'Candidate');
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = input.proposal_id,
            chapter = chapter_b,
            block = Uuid::now_v7(),
            run = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = input.proposal_revision_id,
            base_revision = input.expected_authoritative_revision_id,
            operation = input.selected_pending_operation_ids[0],
        ))
        .await
        .unwrap();
    (scope, input)
}

/// One applicable Reject Proposal Operations in a new Project.
async fn reject_proposal_operations_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<RejectProposalOperationsInput> {
    let (scope, input) = pending_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &REJECT_PROPOSAL_OPERATIONS, input).await
}

/// The rejection records of the Receipt of one call.
async fn rejection_records_of<I>(admin: &Client, call: &CommandCall<I>) -> i64 {
    admin
        .query_one(
            "SELECT count(*) FROM storyos.proposal_rejection_receipts
              WHERE rejection_receipt_id = $1::text::uuid",
            &[&call.envelope.ids.receipt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0)
}

async fn replan_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<ReplanProposalInput>,
) -> Result<ReplanProposalSettlement, ProjectCommandError> {
    store.replan_proposal(&call.envelope, &call.input).await
}

async fn reopen_rejected_operations(
    store: &PostgresProjectReader,
    call: &CommandCall<ReopenRejectedOperationsInput>,
) -> Result<ReopenRejectedOperationsSettlement, ProjectCommandError> {
    store
        .reopen_rejected_operations(&call.envelope, &call.input)
        .await
}

/// The identities of one open Proposal on Chapter B with one Proposal Operation.
struct OpenProposal {
    editor_session_id: String,
    proposal_id: String,
    revision_id: String,
    operation_id: String,
    chapter_head: String,
}

/// A new Project with a writer Editor Session and one open Proposal on Chapter B.
///
/// The fixture skips the foreign keys of the AgentRun and the Manuscript Block.
async fn open_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
    validation: &str,
    resolution: &str,
) -> (ProjectScope, OpenProposal) {
    let (scope, _chapter_a, chapter_b, chapter_head, editor_session_id) =
        two_chapter_writer(store, base).await;
    let proposal = OpenProposal {
        editor_session_id,
        proposal_id: Uuid::now_v7().to_string(),
        revision_id: Uuid::now_v7().to_string(),
        operation_id: Uuid::now_v7().to_string(),
        chapter_head,
    };
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'block_edit', '{chapter_b}',
                     '{block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready', '{validation}',
                     'open', 'Candidate', '{chapter_head}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     '{resolution}', 'resolved', 'Candidate')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = proposal.proposal_id,
            block = Uuid::now_v7(),
            run = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = proposal.revision_id,
            operation = proposal.operation_id,
            chapter_head = proposal.chapter_head,
        ),
    )
    .await;
    (scope, proposal)
}

/// Runs fixture statements in one transaction without their foreign keys.
async fn run_without_foreign_keys(admin: &Client, statements: &str) {
    admin
        .batch_execute(&format!(
            "BEGIN; SET LOCAL session_replication_role = replica; {statements}; COMMIT;"
        ))
        .await
        .unwrap();
}

/// A conflicted open Proposal and the input that replans it.
async fn conflicted_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, ReplanProposalInput) {
    let (scope, proposal) = open_proposal(store, admin, base, "conflicted", "pending").await;
    let conflict_id = Uuid::now_v7().to_string();
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposal_validation_conditions
               (owner_user_id, project_id, proposal_id, proposal_revision_id,
                acceptance_receipt_id, validation, conflict_id, condition_kind)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', '{receipt}',
                     'conflicted', '{conflict_id}', 'proposal_conflict')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = proposal.proposal_id,
            revision = proposal.revision_id,
            receipt = Uuid::now_v7(),
        ),
    )
    .await;
    let input = ReplanProposalInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id),
        proposal_id: proposal.proposal_id,
        conflicted_proposal_revision_id: proposal.revision_id.clone(),
        expected_current_proposal_head: proposal.revision_id,
        expected_authoritative_revision_id: proposal.chapter_head,
        replacement_operation_id: proposal.operation_id,
        source_condition: storyos_contracts::ReplanSourceCondition::ProposalConflict {
            proposal_conflict_ref: conflict_id,
        },
    };
    (scope, input)
}

/// An open Proposal with one rejected Proposal Operation and the input that reopens it.
async fn rejected_operation(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, ReopenRejectedOperationsInput) {
    let (scope, proposal) = open_proposal(store, admin, base, "valid", "rejected").await;
    let rejection_event_id = Uuid::now_v7().to_string();
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.proposal_operation_resolutions
               (owner_user_id, project_id, resolution_event_id, proposal_id,
                proposal_revision_id, operation_id, prior_resolution, resulting_resolution,
                rejection_receipt_id, author_action_sequence)
             VALUES ('{owner}', '{project}', '{rejection_event_id}', '{proposal}',
                     '{revision}', '{operation}', 'pending', 'rejected', '{receipt}', 1)",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            proposal = proposal.proposal_id,
            revision = proposal.revision_id,
            operation = proposal.operation_id,
            receipt = Uuid::now_v7(),
        ),
    )
    .await;
    let input = ReopenRejectedOperationsInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id),
        proposal_id: proposal.proposal_id,
        proposal_revision_id: proposal.revision_id,
        selected_rejected_operation_id: proposal.operation_id,
        rejection_event_id,
        expected_authoritative_revision_id: proposal.chapter_head,
    };
    (scope, input)
}

/// One applicable Replan Proposal in a new Project.
async fn replan_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ReplanProposalInput> {
    let (scope, input) = conflicted_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &REPLAN_PROPOSAL, input).await
}

/// One applicable Reopen Rejected Operations in a new Project.
async fn reopen_rejected_operations_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ReopenRejectedOperationsInput> {
    let (scope, input) = rejected_operation(store, admin, base).await;
    issued(store, &scope, base + 9, &REOPEN_REJECTED_OPERATIONS, input).await
}

async fn withdraw_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<WithdrawProposalInput>,
) -> Result<WithdrawProposalSettlement, ProjectCommandError> {
    store.withdraw_proposal(&call.envelope, &call.input).await
}

/// A new Project with one open Proposal and the input that withdraws it.
async fn withdrawable_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, WithdrawProposalInput) {
    let (scope, proposal) = open_proposal(store, admin, base, "valid", "pending").await;
    let input = WithdrawProposalInput {
        editor_session_id: EditorSessionId::new(proposal.editor_session_id),
        proposal_id: proposal.proposal_id,
        proposal_revision_id: proposal.revision_id,
        expected_authoritative_revision_id: proposal.chapter_head,
        withdrawal_note: WithdrawalNote::Present {
            text: "Not this one".to_owned(),
        },
    };
    (scope, input)
}

/// One applicable author Withdrawal in a new Project.
async fn withdraw_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<WithdrawProposalInput> {
    let (scope, input) = withdrawable_proposal(store, admin, base).await;
    issued(store, &scope, base + 9, &WITHDRAW_PROPOSAL, input).await
}

async fn close_editor_flow_draft(
    store: &PostgresProjectReader,
    call: &CommandCall<CloseEditorFlowDraftInput>,
) -> Result<CloseEditorFlowDraftSettlement, ProjectCommandError> {
    store
        .close_editor_flow_draft(&call.envelope, &call.input)
        .await
}

/// A new Project with a writer Editor Session and one open Refused Edit Draft in `retention`.
///
/// Returns the Scope and the input that discards the Draft. The fixture skips the creation
/// Receipt and lifecycle event of the Draft.
async fn refused_edit_draft(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
    retention: &str,
) -> (ProjectScope, CloseEditorFlowDraftInput) {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let payload = serde_json::json!({
        "schema_revision": "storyos.refused-edit-payload.v1",
        "chapter_id": chapter_b,
        "expected_authoritative_revision_id": revision_b,
        "expected_proposal_head_revision_ids": [],
        "target_refs": [],
        "author_edit_units": [],
        "undo_group_id": Uuid::now_v7().to_string(),
        "completed_intent_record_id": Uuid::now_v7().to_string(),
        "local_intent_sequence": "1",
    });
    let input = CloseEditorFlowDraftInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        writer_generation: 1,
        draft_id: Uuid::now_v7().to_string(),
        source_current_draft_revision_id: Uuid::now_v7().to_string(),
        source_draft_payload_digest: storyos_core::hex_sha256(
            storyos_core::canonical_json(&payload).as_bytes(),
        ),
        source_reopen_event_id: None,
    };
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.draft_artifact_revisions
               (owner_user_id, project_id, draft_id, revision_id, payload, payload_digest)
             VALUES ('{owner}', '{project}', '{draft}', '{revision}', '{payload}'::jsonb,
                     '{digest}');
             INSERT INTO storyos.draft_artifacts
               (owner_user_id, project_id, draft_id, current_revision_id, retention_state)
             VALUES ('{owner}', '{project}', '{draft}', '{revision}', '{retention}');
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            draft = input.draft_id,
            revision = input.source_current_draft_revision_id,
            digest = input.source_draft_payload_digest,
            payload = payload,
        ))
        .await
        .unwrap();
    (scope, input)
}

/// Issues one Draft Discard call whose canonical bytes are the command body of `input`.
async fn discard_call(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    input: CloseEditorFlowDraftInput,
) -> CommandCall<CloseEditorFlowDraftInput> {
    let body = serde_json::json!({
        "close_editor_flow_draft_input": {
            "draft_id": input.draft_id,
            "draft_kind": "refused_edit",
            "source_current_draft_revision_id": input.source_current_draft_revision_id,
            "source_draft_payload_digest": input.source_draft_payload_digest,
            "expected_closure": "open",
            "close_reason": "abandoned",
            "editor_session_id": input.editor_session_id.as_ref(),
            "writer_generation": input.writer_generation.to_string(),
        }
    });
    let mut call = issued(store, scope, suffix, &CLOSE_EDITOR_FLOW_DRAFT, input).await;
    call.envelope.canonical_command_bytes = body.to_string().into_bytes();
    call
}

/// One applicable Draft Discard in a new Project.
async fn close_editor_flow_draft_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<CloseEditorFlowDraftInput> {
    let (scope, input) = refused_edit_draft(store, admin, base, "retained").await;
    discard_call(store, &scope, base + 9, input).await
}

async fn expand_refused_edit_draft(
    store: &PostgresProjectReader,
    call: &CommandCall<ExpandRefusedEditDraftToProposalInput>,
) -> Result<ExpandRefusedEditDraftSettlement, ProjectCommandError> {
    store
        .expand_refused_edit_draft(&call.envelope, &call.input)
        .await
}

/// A new Project with a writer Editor Session and one text Block in Chapter B. One open Refused
/// Edit Draft in `retention` replaces a slice of that Block.
///
/// Returns the Scope and the input that expands the Draft. The fixture skips the creation
/// Receipt and lifecycle event of the Draft and the Block history of the Chapter.
async fn refused_edit_expansion(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
    retention: &str,
) -> (ProjectScope, ExpandRefusedEditDraftToProposalInput) {
    let (scope, _chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let block_id = Uuid::now_v7().to_string();
    let block_text = "Guard the narrator voice in this passage.";
    let manuscript = serde_json::json!({
        "format": "storyos.manuscript-payload.v1",
        "schema_version": 1,
        "coordinate_version": 1,
        "blocks": [{"manuscript_block_id": block_id, "block_kind": "paragraph", "text": block_text}],
    });
    let payload = serde_json::json!({
        "schema_revision": "storyos.refused-edit-payload.v1",
        "chapter_id": chapter_b,
        "expected_authoritative_revision_id": revision_b,
        "expected_proposal_head_revision_ids": [],
        "target_refs": [block_id],
        "author_edit_units": [{
            "normalized_primitives": [{
                "kind": "replace_structured_selection",
                "replacement": [{"block_kind": "paragraph", "text": "A steadier voice."}],
            }],
            "selection_snapshot": {
                "coordinate_profile": PROSEMIRROR_TOKEN_UTF16_V1,
                "from": 10,
                "to": 24,
            },
        }],
        "undo_group_id": Uuid::now_v7().to_string(),
        "completed_intent_record_id": Uuid::now_v7().to_string(),
        "local_intent_sequence": "1",
    });
    let input = ExpandRefusedEditDraftToProposalInput {
        editor_session_id: EditorSessionId::new(editor_session_id),
        writer_generation: 1,
        draft_id: Uuid::now_v7().to_string(),
        source_current_draft_revision_id: Uuid::now_v7().to_string(),
        source_draft_payload_digest: storyos_core::hex_sha256(
            storyos_core::canonical_json(&payload).as_bytes(),
        ),
        source_reopen_event_id: None,
        chapter_id: chapter_b.clone(),
        target_ref: block_id.clone(),
        expected_target_revision_id: revision_b.clone(),
        anchor: OpenInlineProposalAnchor {
            manuscript_block_id: block_id.clone(),
            base_authoritative_revision_id: revision_b.clone(),
            manuscript_schema_version: 1,
            coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
            from: 10,
            to: 24,
            boundary_profile: EXCLUSIVE_AUTHORITATIVE_EDGES_V1.to_owned(),
            base_slice_digest: proposal_anchor_base_slice_digest(
                &block_id,
                "paragraph",
                /*manuscript_schema_version*/ 1,
                PROSEMIRROR_TOKEN_UTF16_V1,
                /*from*/ 10,
                /*to*/ 24,
                &block_text[10..24],
            ),
        },
    };
    run_without_foreign_keys(
        admin,
        &format!(
            "INSERT INTO storyos.manuscript_blocks
               (owner_user_id, project_id, manuscript_block_id, manuscript_object_id, block_kind)
             VALUES ('{owner}', '{project}', '{block}', '{chapter}', 'paragraph');
             DELETE FROM storyos.manuscript_revision_members
              WHERE owner_user_id = '{owner}' AND project_id = '{project}'
                AND manuscript_object_id = '{chapter}' AND revision_id = '{head}';
             INSERT INTO storyos.manuscript_revision_members
               (owner_user_id, project_id, manuscript_object_id, revision_id,
                manuscript_block_id, block_order)
             VALUES ('{owner}', '{project}', '{chapter}', '{head}', '{block}', 1);
             UPDATE storyos.authoritative_payloads AS payload
                SET canonical_bytes = convert_to('{manuscript}', 'UTF8')
               FROM storyos.authoritative_revisions AS revision
              WHERE (revision.owner_user_id, revision.project_id, revision.manuscript_object_id,
                     revision.revision_id) = ('{owner}', '{project}', '{chapter}', '{head}')
                AND (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (revision.owner_user_id, revision.project_id, revision.payload_id);
             INSERT INTO storyos.draft_artifact_revisions
               (owner_user_id, project_id, draft_id, revision_id, payload, payload_digest)
             VALUES ('{owner}', '{project}', '{draft}', '{revision}', '{payload}'::jsonb,
                     '{digest}');
             INSERT INTO storyos.draft_artifacts
               (owner_user_id, project_id, draft_id, current_revision_id, retention_state)
             VALUES ('{owner}', '{project}', '{draft}', '{revision}', '{retention}')",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            block = block_id,
            chapter = chapter_b,
            head = revision_b,
            draft = input.draft_id,
            revision = input.source_current_draft_revision_id,
            digest = input.source_draft_payload_digest,
        ),
    )
    .await;
    (scope, input)
}

/// Issues one Draft expansion call whose canonical bytes carry the source Draft binding of `input`.
async fn expansion_call(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    input: ExpandRefusedEditDraftToProposalInput,
) -> CommandCall<ExpandRefusedEditDraftToProposalInput> {
    let body = serde_json::json!({
        "expand_refused_edit_draft_to_proposal_input": {
            "draft_id": input.draft_id,
            "source_current_draft_revision_id": input.source_current_draft_revision_id,
            "source_draft_payload_digest": input.source_draft_payload_digest,
            "chapter_id": input.chapter_id,
            "target_refs": [input.target_ref],
            "expected_target_revisions": [input.expected_target_revision_id],
            "editor_session_id": input.editor_session_id.as_ref(),
            "writer_generation": input.writer_generation.to_string(),
        }
    });
    let mut call = issued(store, scope, suffix, &EXPAND_REFUSED_EDIT_DRAFT, input).await;
    call.envelope.canonical_command_bytes = body.to_string().into_bytes();
    call
}

/// One applicable Draft expansion in a new Project.
async fn expand_refused_edit_draft_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ExpandRefusedEditDraftToProposalInput> {
    let (scope, input) = refused_edit_expansion(store, admin, base, "retained").await;
    expansion_call(store, &scope, base + 9, input).await
}

/// The Proposals, superseding close events, and closed source Drafts of the Receipt of one call.
async fn expansion_records_of(
    admin: &Client,
    call: &CommandCall<ExpandRefusedEditDraftToProposalInput>,
) -> [i64; 3] {
    let row = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.proposals AS proposal
                       JOIN storyos.domain_receipts AS receipt
                         ON receipt.result_payload->>'proposal_id' = proposal.proposal_id::text
                      WHERE receipt.receipt_id = $1::text::uuid
                        AND proposal.source_draft_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.draft_close_events
                      WHERE receipt_id = $1::text::uuid AND close_reason = 'superseded'),
                    (SELECT count(*) FROM storyos.draft_artifacts AS draft
                       JOIN storyos.draft_close_events AS event
                         ON event.event_id = draft.close_event_id
                      WHERE event.receipt_id = $1::text::uuid AND draft.closure = 'closed')",
            &[&call.envelope.ids.receipt_id, &call.input.draft_id],
        )
        .await
        .unwrap();
    [0, 1, 2].map(|index| row.get(index))
}

async fn complete_ready_partial_proposal(
    store: &PostgresProjectReader,
    call: &CommandCall<CompleteReadyPartialProposalInput>,
) -> Result<CompleteReadyPartialProposalSettlement, ProjectCommandError> {
    store
        .complete_ready_partial_proposal(&call.envelope, &call.input)
        .await
}

async fn continue_proposal_generation(
    store: &PostgresProjectReader,
    call: &CommandCall<ContinueProposalGenerationInput>,
) -> Result<ContinueProposalGenerationSettlement, ProjectCommandError> {
    store
        .continue_proposal_generation(&call.envelope, &call.input)
        .await
}

/// The applicable completion and continuation of one ready-partial Proposal Generation.
struct GenerationFixture {
    complete: CompleteReadyPartialProposalInput,
    continuation: ContinueProposalGenerationInput,
    /// The head of Chapter A, which is not the head of the Proposal Chapter B.
    other_chapter_revision_id: String,
}

/// A new Project with a writer Editor Session and one open ready-partial Proposal on Chapter B.
///
/// The Proposal Generation belongs to a paused AgentRun. The fixture skips the foreign keys of
/// the AgentRun, the Manuscript Block, and the AgentRun Receipt.
async fn ready_partial_proposal(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (ProjectScope, GenerationFixture) {
    let (scope, chapter_a, chapter_b, revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let candidate_text = "Candidate";
    let proposal_id = Uuid::now_v7().to_string();
    let revision_id = Uuid::now_v7().to_string();
    let generation_id = Uuid::now_v7().to_string();
    let operation_id = Uuid::now_v7().to_string();
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             INSERT INTO storyos.agent_runs
               (owner_user_id, project_id, run_id, project_agent_id, conversation_id,
                memory_settings_revision, grant_id, project_model_use_binding_revision,
                chapter_id, author_message, status, receipt_id)
             VALUES ('{owner}', '{project}', '{run}', '{agent}', '{conversation}',
                     '{memory}', '{grant}', '{binding}', '{chapter}', 'Continue',
                     'paused', '{run_receipt}');
             INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id)
             VALUES ('{owner}', '{project}', '{proposal}', 'block_edit', '{chapter}',
                     '{block}', '{run}', '{decision}');
             INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}', 'ready_partial',
                     'valid', 'open', '{candidate_text}', '{base_revision}');
             INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{revision}');
             INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             VALUES ('{owner}', '{project}', '{proposal}', '{operation}', '{block}',
                     'pending', 'resolved', '{candidate_text}');
             INSERT INTO storyos.proposal_generations
               (owner_user_id, project_id, generation_id, proposal_id, last_applied_stream_seq,
                run_id)
             VALUES ('{owner}', '{project}', '{generation}', '{proposal}', 3, '{run}');
             INSERT INTO storyos.proposal_generation_heads
               (owner_user_id, project_id, proposal_id, generation_id)
             VALUES ('{owner}', '{project}', '{proposal}', '{generation}');
             COMMIT;",
            owner = scope.owner_user_id.as_ref(),
            project = scope.project_id.as_ref(),
            run = Uuid::now_v7(),
            agent = Uuid::now_v7(),
            conversation = Uuid::now_v7(),
            memory = Uuid::now_v7(),
            grant = Uuid::now_v7(),
            binding = Uuid::now_v7(),
            chapter = chapter_b,
            run_receipt = Uuid::now_v7(),
            proposal = proposal_id,
            block = Uuid::now_v7(),
            decision = Uuid::now_v7(),
            revision = revision_id,
            base_revision = revision_b,
            operation = operation_id,
            generation = generation_id,
        ))
        .await
        .unwrap();
    let other_chapter_revision_id = admin
        .query_one(
            "SELECT current_revision_id::text FROM storyos.authoritative_heads
              WHERE manuscript_object_id = $1::text::uuid",
            &[&chapter_a],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let editor_session_id = EditorSessionId::new(editor_session_id);
    let expected_candidate_digest = storyos_core::hex_sha256(candidate_text.as_bytes());
    let fixture = GenerationFixture {
        complete: CompleteReadyPartialProposalInput {
            editor_session_id: editor_session_id.clone(),
            proposal_id: proposal_id.clone(),
            proposal_revision_id: revision_id.clone(),
            generation_id: generation_id.clone(),
            expected_candidate_digest: expected_candidate_digest.clone(),
            last_applied_stream_seq: 3,
            expected_authoritative_revision_id: revision_b.clone(),
        },
        continuation: ContinueProposalGenerationInput {
            editor_session_id,
            proposal_id,
            proposal_revision_id: revision_id,
            prior_generation_id: generation_id,
            expected_generation_state: "ready_partial".to_owned(),
            expected_candidate_digest,
            selected_pending_operation_ids: vec![operation_id],
            expected_authoritative_revision_id: revision_b,
        },
        other_chapter_revision_id,
    };
    (scope, fixture)
}

/// One applicable Complete Ready Partial Proposal in a new Project.
async fn complete_ready_partial_proposal_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<CompleteReadyPartialProposalInput> {
    let (scope, fixture) = ready_partial_proposal(store, admin, base).await;
    issued(
        store,
        &scope,
        base + 9,
        &COMPLETE_READY_PARTIAL_PROPOSAL,
        fixture.complete,
    )
    .await
}

/// One applicable Continue Proposal Generation in a new Project.
async fn continue_proposal_generation_call(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> CommandCall<ContinueProposalGenerationInput> {
    let (scope, fixture) = ready_partial_proposal(store, admin, base).await;
    issued(
        store,
        &scope,
        base + 9,
        &CONTINUE_PROPOSAL_GENERATION,
        fixture.continuation,
    )
    .await
}

/// One applicable Create Volume in a new Project.
async fn create_volume_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<CreateVolumeInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = CreateVolumeInput {
        title: "Volume".to_owned(),
        expected_tree_revision: 1,
    };
    issued(store, &scope, base + 9, &CREATE_VOLUME, input).await
}

/// One applicable Update Volume in a new Project with one Volume.
async fn update_volume_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateVolumeInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, 1).await;
    let input = UpdateVolumeInput {
        volume_id,
        title: "Renamed".to_owned(),
        order: 1,
        expected_tree_revision: 2,
    };
    issued(store, &scope, base + 9, &UPDATE_VOLUME, input).await
}

/// One applicable Delete Volume in a new Project with one empty Volume.
async fn delete_volume_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<DeleteVolumeInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, 1).await;
    let input = DeleteVolumeInput {
        volume_id,
        expected_tree_revision: 2,
    };
    issued(store, &scope, base + 9, &DELETE_VOLUME, input).await
}

/// One applicable Create Chapter in a new Project with one Volume.
async fn create_chapter_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<CreateChapterInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, 1).await;
    let input = CreateChapterInput {
        volume_id: volume_id.as_ref().to_owned(),
        title: "Chapter".to_owned(),
        placement: CreateChapterPlacement::Append,
        expected_tree_revision: 2,
    };
    issued(store, &scope, base + 9, &CREATE_CHAPTER, input).await
}

/// One applicable Update Chapter in a new Project with one Volume and one Chapter.
async fn update_chapter_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateChapterInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, 1).await;
    let chapter_id = new_chapter(store, &scope, base + 2, &volume_id, 2).await;
    let input = UpdateChapterInput {
        chapter_id,
        title: "Renamed".to_owned(),
        order: 1,
        expected_tree_revision: 3,
    };
    issued(store, &scope, base + 9, &UPDATE_CHAPTER, input).await
}

/// One applicable Delete Chapter in a new Project with one Volume and one Chapter.
async fn delete_chapter_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<DeleteChapterInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let volume_id = new_volume(store, &scope, base + 1, 1).await;
    let chapter_id = new_chapter(store, &scope, base + 2, &volume_id, 2).await;
    let input = DeleteChapterInput {
        chapter_id,
        expected_tree_revision: 3,
    };
    issued(store, &scope, base + 9, &DELETE_CHAPTER, input).await
}

/// One applicable Update Project in a new Project.
async fn update_project_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<UpdateProjectInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = UpdateProjectInput {
        title: "Renamed".to_owned(),
        expected_revision: 1,
    };
    issued(store, &scope, base + 9, &UPDATE_PROJECT, input).await
}

/// One applicable Archive Project in a new Project.
async fn archive_project_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<ArchiveProjectInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    let input = ArchiveProjectInput {
        expected_revision: 1,
    };
    issued(store, &scope, base + 9, &ARCHIVE_PROJECT, input).await
}

fn with_new_request_ids<I: Clone>(call: &CommandCall<I>) -> CommandCall<I> {
    let mut retry = call.clone();
    retry.envelope.ids = AuthorCommandAdmissionIds {
        command_id: Uuid::now_v7().to_string(),
        author_command_admission_id: Uuid::now_v7().to_string(),
        receipt_id: Uuid::now_v7().to_string(),
    };
    retry
}

/// Marks the Command Challenge consumed and the Command Idempotency Fence in progress, then retries.
///
/// Returns whether the retry is a binding conflict and the rows of the retry Receipt.
async fn in_progress_retry<C: ProjectCommand>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
) -> (bool, [i64; 5]) {
    admin
        .batch_execute(&format!(
            "UPDATE storyos.project_command_challenges SET consumed_at = clock_timestamp()
              WHERE idempotency_key = '{key}';
             UPDATE storyos.command_idempotency SET outcome_kind = 'in_progress'
              WHERE idempotency_key = '{key}';",
            key = call.envelope.challenge_binding.idempotency_key,
        ))
        .await
        .unwrap();
    let retry = settle_project_command(store, &call.envelope, &call.input).await;
    (
        matches!(retry, Err(ProjectCommandError::BindingConflict)),
        settlement_rows(admin, &call.envelope.ids.receipt_id).await,
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_in_progress_exact_retry_conflicts_and_writes_no_row() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        in_progress_retry(
            &store,
            &admin,
            &create_volume_call(&store, /*base*/ 0x5e00).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &update_volume_call(&store, /*base*/ 0x5e10).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &delete_volume_call(&store, /*base*/ 0x5e20).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &create_chapter_call(&store, /*base*/ 0x5e30).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &update_chapter_call(&store, /*base*/ 0x5e40).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &delete_chapter_call(&store, /*base*/ 0x5e50).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &update_project_call(&store, /*base*/ 0x5e60).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &archive_project_call(&store, /*base*/ 0x5e70).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &set_current_chapter_call(&store, /*base*/ 0x5e80).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &update_project_assistance_call(&store, /*base*/ 0x5e90).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &take_over_project_writer_call(&store, /*base*/ 0x5ea0).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x5eb0).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b20).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7240).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7250).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7470).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &close_editor_flow_draft_call(&store, &admin, /*base*/ 0x7d20).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x7510).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x7520).await,
        )
        .await,
        in_progress_retry(
            &store,
            &admin,
            &expand_refused_edit_draft_call(&store, &admin, /*base*/ 0x8e20).await,
        )
        .await,
    ];
    assert_eq!(observed, vec![(true, [0; 5]); 20]);
}

#[derive(Clone, Copy, Debug)]
enum FailurePoint {
    Classify,
    Apply,
    AfterAuthority,
}

/// The failure that `Failing` injects, named by its step.
#[derive(Debug)]
struct Injected(&'static str);

impl std::fmt::Display for Injected {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "injected {} failure", self.0)
    }
}

impl std::error::Error for Injected {}

/// A project command that fails after one step has written its rows.
struct Failing<C> {
    command: C,
    at: FailurePoint,
}

impl<C: ProjectCommand> ProjectCommand for Failing<C> {
    const SPEC: CommandSpec = C::SPEC;
    type Profile = C::Profile;
    type Response = C::Response;
    type ZeroEffect = C::ZeroEffect;
    type Applied = C::Applied;
    type Plan = C::Plan;
    type Effect = C::Effect;
    type NoEffect = C::NoEffect;
    type Conflict = C::Conflict;
    type Refusal = C::Refusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let classified = self.command.classify(client, envelope, project).await?;
        match self.at {
            FailurePoint::Classify => Err(unavailable(Injected("classify"))),
            FailurePoint::Apply | FailurePoint::AfterAuthority => Ok(Classification {
                outcome: classified.outcome,
                admission: classified.admission,
                heads: classified.heads,
                zero_receipt: classified.zero_receipt,
            }),
        }
    }

    fn applied_receipt_payload(&self, applied: &Self::Applied, plan: &Self::Plan) -> String {
        self.command.applied_receipt_payload(applied, plan)
    }

    fn applied_receipt_refs(&self, applied: &Self::Applied, plan: &Self::Plan) -> ReceiptRefs {
        self.command.applied_receipt_refs(applied, plan)
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        sequences: &ProfileSequences<Self>,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> Result<ProfileWrite<Self>, ProjectCommandError> {
        let write = self
            .command
            .apply(client, envelope, project, sequences, plan, applied)
            .await?;
        match self.at {
            FailurePoint::Apply => Err(unavailable(Injected("apply"))),
            FailurePoint::Classify | FailurePoint::AfterAuthority => Ok(write),
        }
    }

    fn apply_after_authority(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        applied: &ProfileApplied<Self>,
    ) -> impl std::future::Future<Output = Result<(), ProjectCommandError>> + Send {
        // The applied value need not be `Sync`, so only the inner future holds it.
        let written = self
            .command
            .apply_after_authority(client, envelope, applied);
        async move {
            written.await?;
            match self.at {
                FailurePoint::AfterAuthority => Err(unavailable(Injected("after authority"))),
                FailurePoint::Classify | FailurePoint::Apply => Ok(()),
            }
        }
    }

    fn decode(&self, replay: &CommandReplay) -> Result<Self::Effect, ReplayFault> {
        self.command.decode(replay)
    }

    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        self.command.zero_authority_rows(&inner_outcome(outcome))
    }

    async fn write_zero_authority_effect(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        outcome: &ZeroOutcome<'_, Self>,
    ) -> Result<Self::ZeroEffect, ProjectCommandError> {
        self.command
            .write_zero_authority_effect(client, envelope, &inner_outcome(outcome))
            .await?;
        Err(unavailable(Injected("zero-authority write")))
    }

    async fn write_zero_authority_activity(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        activity: &ActivitySequences,
    ) -> Result<ZeroAuthorityWrite<Self::ZeroEffect>, ProjectCommandError> {
        self.command
            .write_zero_authority_activity(client, envelope, project, activity)
            .await?;
        Err(unavailable(Injected("zero-authority write")))
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<Self::ZeroEffect>, ReplayFault> {
        self.command.decode_zero_authority_effect(replay)
    }
}

/// The zero-authority outcome of the wrapped command.
fn inner_outcome<'a, C: ProjectCommand>(
    outcome: &ZeroOutcome<'a, Failing<C>>,
) -> ZeroOutcome<'a, C> {
    match outcome {
        ZeroOutcome::NoEffect(reason) => ZeroOutcome::NoEffect(*reason),
        ZeroOutcome::Conflicted(reason) => ZeroOutcome::Conflicted(*reason),
        ZeroOutcome::Refused(reason) => ZeroOutcome::Refused(*reason),
    }
}

/// Fails the call at each failure point, and then settles it for real.
///
/// Returns, for each failure, the step of the injected failure and the rows of its Receipt, then
/// the Receipt result kind of the real settlement.
async fn failed_then_settled<C: ProjectCommand + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
) -> (Vec<(Option<&'static str>, [i64; 5])>, ReceiptResult) {
    let mut failures = Vec::new();
    for at in [
        FailurePoint::Classify,
        FailurePoint::Apply,
        FailurePoint::AfterAuthority,
    ] {
        let failing = Failing {
            command: call.input.clone(),
            at,
        };
        let injected = match settle_project_command(store, &call.envelope, &failing).await {
            Err(ProjectCommandError::Unavailable(source)) => source
                .downcast_ref::<Injected>()
                .map(|Injected(step)| *step),
            Err(
                ProjectCommandError::BindingConflict
                | ProjectCommandError::HistoricalAcknowledgementUnavailable
                | ProjectCommandError::InvalidChallenge
                | ProjectCommandError::MissingProject
                | ProjectCommandError::WriterIneligible,
            )
            | Ok(_) => None,
        };
        failures.push((
            injected,
            settlement_rows(admin, &call.envelope.ids.receipt_id).await,
        ));
    }
    let Ok(settled) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the unused Command Challenge must still settle");
    };
    (failures, settled.outcome.receipt_result())
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_failing_step_rolls_back_every_row_and_keeps_the_challenge_unused() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        failed_then_settled(
            &store,
            &admin,
            &create_volume_call(&store, /*base*/ 0x5f00).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_volume_call(&store, /*base*/ 0x5f10).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &delete_volume_call(&store, /*base*/ 0x5f20).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &create_chapter_call(&store, /*base*/ 0x5f30).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_chapter_call(&store, /*base*/ 0x5f40).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &delete_chapter_call(&store, /*base*/ 0x5f50).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_project_call(&store, /*base*/ 0x5f60).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &archive_project_call(&store, /*base*/ 0x5f70).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &set_current_chapter_call(&store, /*base*/ 0x5f80).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &update_project_assistance_call(&store, /*base*/ 0x5f90).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &take_over_project_writer_call(&store, /*base*/ 0x5fa0).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x5fb0).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b30).await,
        )
        .await,
        failed_then_settled(&store, &admin, &{
            let mut stale = reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b40).await;
            stale.input.proposal_revision_id = Uuid::now_v7().to_string();
            stale
        })
        .await,
        failed_then_settled(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7260).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7270).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7480).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &close_editor_flow_draft_call(&store, &admin, /*base*/ 0x7d30).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x7530).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x7540).await,
        )
        .await,
        failed_then_settled(
            &store,
            &admin,
            &expand_refused_edit_draft_call(&store, &admin, /*base*/ 0x8e30).await,
        )
        .await,
    ];
    let rolled_back = |result| {
        let after_classify = match result {
            ReceiptResult::AuthoritativeApplied => ["apply", "after authority"],
            ReceiptResult::NoEffect | ReceiptResult::Conflicted | ReceiptResult::Refused => {
                ["zero-authority write"; 2]
            }
        };
        let failures = std::iter::once("classify")
            .chain(after_classify)
            .map(|step| (Some(step), [0; 5]))
            .collect::<Vec<_>>();
        (failures, result)
    };
    let mut expected = vec![rolled_back(ReceiptResult::AuthoritativeApplied); 10];
    expected.push(rolled_back(ReceiptResult::NoEffect));
    expected.extend(vec![rolled_back(ReceiptResult::AuthoritativeApplied); 2]);
    expected.push(rolled_back(ReceiptResult::Refused));
    expected.extend(vec![rolled_back(ReceiptResult::AuthoritativeApplied); 7]);
    assert_eq!(observed, expected);
}

/// The replay error of one exact retry.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ReplayError {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    InvalidChallenge,
    MissingProject,
    WriterIneligible,
    Unavailable,
}

/// Settles the call, then replays it with pre-capture and then with damaged acknowledgement evidence.
async fn evidence_replays<C: ProjectCommand + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
) -> (ReceiptResult, [ReplayError; 2]) {
    let Ok(settled) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the project command must settle");
    };
    let key = &call.envelope.challenge_binding.idempotency_key;
    let mut errors = Vec::new();
    for evidence in [
        "acknowledgement_format = NULL, response_project = NULL, response_assistance = NULL",
        "acknowledgement_format = 'command_response_project.v1',
         response_project = '{\"broken\":true}'::jsonb, response_assistance = NULL",
    ] {
        admin
            .batch_execute(&format!(
                "UPDATE storyos.command_idempotency SET {evidence}
                  WHERE idempotency_key = '{key}'"
            ))
            .await
            .unwrap();
        let retry = with_new_request_ids(call);
        let Err(error) = settle_project_command(store, &retry.envelope, &retry.input).await else {
            panic!("a replay without complete evidence must fail");
        };
        errors.push(error.into());
    }
    let errors: [ReplayError; 2] = errors.try_into().unwrap();
    (settled.outcome.receipt_result(), errors)
}

impl From<ProjectCommandError> for ReplayError {
    fn from(error: ProjectCommandError) -> Self {
        match error {
            ProjectCommandError::BindingConflict => Self::BindingConflict,
            ProjectCommandError::HistoricalAcknowledgementUnavailable => {
                Self::HistoricalAcknowledgementUnavailable
            }
            ProjectCommandError::InvalidChallenge => Self::InvalidChallenge,
            ProjectCommandError::MissingProject => Self::MissingProject,
            ProjectCommandError::WriterIneligible => Self::WriterIneligible,
            ProjectCommandError::Unavailable(_) => Self::Unavailable,
        }
    }
}

/// The effect row of one applied Proposal decision and its preserved Proposal State Axes.
struct PreservedEffect {
    table: &'static str,
    receipt_column: &'static str,
    preserved_columns: &'static [&'static str],
}

/// Settles the call and clears one preserved value, which the all-or-none check refuses. Then
/// replays the call once without its preserved values and once without its effect row.
async fn effect_replays<C: ProjectCommand + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    effect: PreservedEffect,
) -> (Option<SqlState>, [ReplayError; 2]) {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the Proposal decision must settle");
    };
    let PreservedEffect {
        table,
        receipt_column,
        preserved_columns,
    } = effect;
    let filter = format!(
        "WHERE {receipt_column} = '{}'",
        call.envelope.ids.receipt_id
    );
    let cleared = preserved_columns
        .iter()
        .map(|column| format!("{column} = NULL"))
        .collect::<Vec<_>>()
        .join(", ");
    let partial = admin
        .batch_execute(&format!(
            "UPDATE storyos.{table} SET {} = NULL {filter}",
            preserved_columns[0]
        ))
        .await
        .err()
        .and_then(|error| error.code().cloned());
    let mut errors = Vec::new();
    for statement in [
        format!("UPDATE storyos.{table} SET {cleared} {filter}"),
        format!("DELETE FROM storyos.{table} {filter}"),
    ] {
        run_without_foreign_keys(admin, &statement).await;
        let retry = with_new_request_ids(call);
        let Err(error) = settle_project_command(store, &retry.envelope, &retry.input).await else {
            panic!("a replay without its preserved values must fail");
        };
        errors.push(error.into());
    }
    (partial, errors.try_into().unwrap())
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_replay_separates_pre_capture_from_damaged_evidence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        evidence_replays(
            &store,
            &admin,
            &create_volume_call(&store, /*base*/ 0x6a00).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &update_volume_call(&store, /*base*/ 0x6a10).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &delete_volume_call(&store, /*base*/ 0x6a20).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &create_chapter_call(&store, /*base*/ 0x6a30).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &update_chapter_call(&store, /*base*/ 0x6a40).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &delete_chapter_call(&store, /*base*/ 0x6a50).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &update_project_call(&store, /*base*/ 0x6a60).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &archive_project_call(&store, /*base*/ 0x6a70).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &set_current_chapter_call(&store, /*base*/ 0x6a80).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &update_project_assistance_call(&store, /*base*/ 0x6a90).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x6ad0).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7b50).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7280).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7290).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7490).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x7550).await,
        )
        .await,
        evidence_replays(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x7560).await,
        )
        .await,
    ];
    let separated = (
        ReceiptResult::AuthoritativeApplied,
        [
            ReplayError::HistoricalAcknowledgementUnavailable,
            ReplayError::Unavailable,
        ],
    );
    assert_eq!(observed, vec![separated; 17]);
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_proposal_decision_replay_separates_pre_capture_from_damaged_effect_rows() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        effect_replays(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x7c00).await,
            PreservedEffect {
                table: "proposal_rejection_receipts",
                receipt_column: "rejection_receipt_id",
                preserved_columns: &[
                    "preserved_generation",
                    "preserved_validation",
                    "preserved_closure",
                ],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x7c10).await,
            PreservedEffect {
                table: "proposal_withdrawals",
                receipt_column: "withdrawal_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_validation"],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x7c20).await,
            PreservedEffect {
                table: "proposal_replans",
                receipt_column: "replan_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_closure"],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x7c30).await,
            PreservedEffect {
                table: "proposal_withdrawal_reopenings",
                receipt_column: "reopen_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_operation_resolution"],
            },
        )
        .await,
        effect_replays(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x7c40).await,
            PreservedEffect {
                table: "proposal_operation_reopenings",
                receipt_column: "reopen_receipt_id",
                preserved_columns: &["preserved_generation", "preserved_closure"],
            },
        )
        .await,
    ];
    let separated = (
        Some(SqlState::CHECK_VIOLATION),
        [
            ReplayError::HistoricalAcknowledgementUnavailable,
            ReplayError::Unavailable,
        ],
    );
    assert_eq!(observed, vec![separated; 5]);
}

/// Settles the call, damages its records with `damage` that takes the Receipt identity, and
/// replays it.
async fn damaged_replay<C: ProjectCommand + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    damage: fn(&str) -> String,
) -> ReplayError {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the project command must settle");
    };
    run_without_foreign_keys(admin, &damage(&call.envelope.ids.receipt_id)).await;
    let retry = with_new_request_ids(call);
    let Err(error) = settle_project_command(store, &retry.envelope, &retry.input).await else {
        panic!("a replay with damaged records must fail");
    };
    error.into()
}

fn without_transition_record(receipt_id: &str) -> String {
    format!("DELETE FROM storyos.proposal_generation_transitions WHERE receipt_id = '{receipt_id}'")
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_generation_decision_without_its_transition_record_is_damaged_evidence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = [
        damaged_replay(
            &store,
            &admin,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x7570).await,
            without_transition_record,
        )
        .await,
        damaged_replay(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x7580).await,
            without_transition_record,
        )
        .await,
    ];
    assert_eq!(
        observed,
        [ReplayError::Unavailable, ReplayError::Unavailable]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_multi_operation_rejection_replays_the_resolution_of_its_first_selected_operation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut call = reject_proposal_operations_call(&store, &admin, /*base*/ 0x7c50).await;
    let first_operation = call.input.selected_pending_operation_ids[0].clone();
    let later_operation = Uuid::now_v7().to_string();
    run_without_foreign_keys(
        &admin,
        &format!(
            "INSERT INTO storyos.proposal_operations
               (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                resolution, reservation_state, candidate_text)
             SELECT owner_user_id, project_id, proposal_id, '{later_operation}',
                    gen_random_uuid(), resolution, reservation_state, candidate_text
               FROM storyos.proposal_operations WHERE operation_id = '{first_operation}'"
        ),
    )
    .await;
    call.input.selected_pending_operation_ids = vec![later_operation, first_operation];
    let (result_kind, _rows) =
        replayed_outcome(&store, &admin, &call, reject_proposal_operations).await;
    assert_eq!(result_kind, "proposal_operations_resolved");
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_command_without_a_response_record_replays_without_it_and_damaged_activity_is_a_fault() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = take_over_project_writer_call(&store, /*base*/ 0x6aa0).await;
    let first = take_over_project_writer(&store, &call).await.unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET acknowledgement_format = NULL, response_project = NULL
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let pre_capture = take_over_project_writer(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.project_activity_event_payloads
                SET payload = jsonb_set(payload, '{{prior_writer_generation}}', '\"damaged\"')
              WHERE receipt_id = '{}'",
            first.ids.receipt_id
        ))
        .await
        .unwrap();
    let damaged = take_over_project_writer(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET canonical_command_digest = 'sha256:takeOverProjectWriter:damaged'
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let other_digest = take_over_project_writer(&store, &with_new_request_ids(&call)).await;
    assert_eq!(pre_capture.unwrap(), first);
    assert!(matches!(damaged, Err(ProjectCommandError::Unavailable(_))));
    assert!(matches!(
        other_digest,
        Err(ProjectCommandError::BindingConflict)
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_assistance_replay_separates_a_project_only_record_from_damaged_assistance() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = update_project_assistance_call(&store, /*base*/ 0x6ab0).await;
    store
        .update_project_assistance(&call.envelope, &call.input)
        .await
        .unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    let mut errors = Vec::new();
    for evidence in [
        "acknowledgement_format = 'command_response_project.v1', response_assistance = NULL",
        "acknowledgement_format = 'command_response_project_assistance.v1',
         response_assistance = '{}'::jsonb",
    ] {
        admin
            .batch_execute(&format!(
                "UPDATE storyos.command_idempotency SET {evidence}
                  WHERE idempotency_key = '{key}'"
            ))
            .await
            .unwrap();
        let retry = with_new_request_ids(&call);
        errors.push(
            match store
                .update_project_assistance(&retry.envelope, &retry.input)
                .await
            {
                Err(ProjectCommandError::HistoricalAcknowledgementUnavailable) => {
                    ReplayError::HistoricalAcknowledgementUnavailable
                }
                Err(ProjectCommandError::Unavailable(_)) => ReplayError::Unavailable,
                other => panic!("the replay must fail on assistance evidence, got {other:?}"),
            },
        );
    }
    assert_eq!(
        errors,
        vec![
            ReplayError::HistoricalAcknowledgementUnavailable,
            ReplayError::Unavailable,
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_exact_retry_ignores_a_later_canonical_snapshot_at_the_same_position() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, _admin) = stores().await;
    let switch = set_current_chapter_call(&store, /*base*/ 0x6ac0).await;
    let scope = switch.envelope.project_scope.clone();
    let switched = store
        .set_current_chapter(&switch.envelope, &switch.input)
        .await
        .unwrap();
    open_session(&store, &scope, "6acb").await;
    let rename_input = UpdateProjectInput {
        title: "Renamed".to_owned(),
        expected_revision: 1,
    };
    let rename = issued(
        &store,
        &scope,
        /*suffix*/ 0x6acc,
        &UPDATE_PROJECT,
        rename_input,
    )
    .await;
    let renamed = store
        .update_project(&rename.envelope, &rename.input)
        .await
        .unwrap();
    open_session(&store, &scope, "6acd").await;
    let switch_retry = with_new_request_ids(&switch);
    let rename_retry = with_new_request_ids(&rename);
    assert_eq!(
        (
            store
                .set_current_chapter(&switch_retry.envelope, &switch_retry.input)
                .await
                .unwrap(),
            store
                .update_project(&rename_retry.envelope, &rename_retry.input)
                .await
                .unwrap(),
        ),
        (switched, renamed)
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_discard_replays_without_a_response_record_and_a_damaged_close_event_is_a_fault() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = close_editor_flow_draft_call(&store, &admin, /*base*/ 0x7d40).await;
    let first = close_editor_flow_draft(&store, &call).await.unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET acknowledgement_format = NULL, response_project = NULL
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let pre_capture = close_editor_flow_draft(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             UPDATE storyos.draft_close_events
                SET author_action_sequence = author_action_sequence + 1000
              WHERE receipt_id = '{}';
             COMMIT;",
            first.ids.receipt_id
        ))
        .await
        .unwrap();
    let damaged = close_editor_flow_draft(&store, &with_new_request_ids(&call)).await;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET canonical_command_digest = 'sha256:closeEditorFlowDraft:damaged'
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let other_digest = close_editor_flow_draft(&store, &with_new_request_ids(&call)).await;
    assert_eq!(pre_capture.unwrap(), first);
    assert!(matches!(damaged, Err(ProjectCommandError::Unavailable(_))));
    assert!(matches!(
        other_digest,
        Err(ProjectCommandError::BindingConflict)
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_discard_refuses_before_admission_and_requires_the_client_writer_generation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, open) = refused_edit_draft(&store, &admin, /*base*/ 0x7d50, "retained").await;
    let stale_writer = CloseEditorFlowDraftInput {
        writer_generation: 2,
        ..open.clone()
    };
    let missing = CloseEditorFlowDraftInput {
        draft_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let (damaged_scope, damaged) =
        refused_edit_draft(&store, &admin, /*base*/ 0x7d60, "retained").await;
    admin
        .batch_execute(&format!(
            "BEGIN;
             SET LOCAL session_replication_role = replica;
             UPDATE storyos.draft_artifact_revisions SET payload_digest = repeat('0', 64)
              WHERE draft_id = '{}';
             COMMIT;",
            damaged.draft_id
        ))
        .await
        .unwrap();
    let mut observed = Vec::new();
    for (scope, suffix, input) in [
        (&scope, 0x7d59, stale_writer),
        (&scope, 0x7d5a, missing),
        (&damaged_scope, 0x7d69, damaged),
    ] {
        let call = discard_call(&store, scope, suffix, input).await;
        let refused = close_editor_flow_draft(&store, &call).await;
        observed.push((
            match refused {
                Err(error) => ReplayError::from(error),
                Ok(settled) => panic!("the Discard must refuse before Admission, got {settled:?}"),
            },
            settlement_rows(&admin, &call.envelope.ids.receipt_id).await,
        ));
    }
    let open_call = discard_call(&store, &scope, /*suffix*/ 0x7d5b, open).await;
    let settled = close_editor_flow_draft(&store, &open_call).await.unwrap();
    assert_eq!(
        (observed, settled.outcome.receipt_result()),
        (
            vec![
                (ReplayError::WriterIneligible, [0; 5]),
                (ReplayError::MissingProject, [0; 5]),
                (ReplayError::BindingConflict, [0; 5]),
            ],
            ReceiptResult::AuthoritativeApplied,
        )
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_expansion_replays_from_its_proposal_revision_and_damaged_effect_rows_are_faults() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = expand_refused_edit_draft_call(&store, &admin, /*base*/ 0x8e40).await;
    let first = expand_refused_edit_draft(&store, &call).await.unwrap();
    let key = &call.envelope.challenge_binding.idempotency_key;
    let receipt_id = &first.ids.receipt_id;
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET acknowledgement_format = NULL, response_project = NULL
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let pre_capture = expand_refused_edit_draft(&store, &with_new_request_ids(&call)).await;
    let mut damaged = Vec::new();
    for statement in [
        format!(
            "UPDATE storyos.draft_close_events
                SET author_action_sequence = author_action_sequence + 1000
              WHERE receipt_id = '{receipt_id}'"
        ),
        format!(
            "DELETE FROM storyos.proposal_revisions AS revision
               USING storyos.domain_receipts AS receipt
              WHERE receipt.receipt_id = '{receipt_id}'
                AND revision.revision_id = receipt.proposal_revision_ids[1]"
        ),
        format!("DELETE FROM storyos.author_action_entries WHERE receipt_id = '{receipt_id}'"),
        format!("DELETE FROM storyos.draft_close_events WHERE receipt_id = '{receipt_id}'"),
    ] {
        run_without_foreign_keys(&admin, &statement).await;
        let replayed = expand_refused_edit_draft(&store, &with_new_request_ids(&call)).await;
        damaged.push(match replayed {
            Err(error) => ReplayError::from(error),
            Ok(settled) => panic!("a damaged expansion must not replay, got {settled:?}"),
        });
    }
    admin
        .batch_execute(&format!(
            "UPDATE storyos.command_idempotency
                SET canonical_command_digest = 'sha256:expandRefusedEditDraftToProposal:damaged'
              WHERE idempotency_key = '{key}'"
        ))
        .await
        .unwrap();
    let other_digest = expand_refused_edit_draft(&store, &with_new_request_ids(&call)).await;
    assert_eq!(pre_capture.unwrap(), first);
    assert_eq!(damaged, vec![ReplayError::Unavailable; 4]);
    assert!(matches!(
        other_digest,
        Err(ProjectCommandError::BindingConflict)
    ));
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_expansion_refuses_before_admission_and_requires_the_client_writer_generation() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, open) = refused_edit_expansion(&store, &admin, /*base*/ 0x8e50, "retained").await;
    let stale_writer = ExpandRefusedEditDraftToProposalInput {
        writer_generation: 2,
        ..open.clone()
    };
    let missing = ExpandRefusedEditDraftToProposalInput {
        draft_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let (damaged_scope, damaged) =
        refused_edit_expansion(&store, &admin, /*base*/ 0x8e60, "retained").await;
    run_without_foreign_keys(
        &admin,
        &format!(
            "UPDATE storyos.draft_artifact_revisions
                SET payload = jsonb_set(payload, '{{local_intent_sequence}}', '\"2\"')
              WHERE draft_id = '{}'",
            damaged.draft_id
        ),
    )
    .await;
    let mut observed = Vec::new();
    for (scope, suffix, input) in [
        (&scope, 0x8e59, stale_writer),
        (&scope, 0x8e5a, missing),
        (&damaged_scope, 0x8e69, damaged),
    ] {
        let call = expansion_call(&store, scope, suffix, input).await;
        let refused = expand_refused_edit_draft(&store, &call).await;
        observed.push((
            match refused {
                Err(error) => ReplayError::from(error),
                Ok(settled) => {
                    panic!("the expansion must refuse before Admission, got {settled:?}")
                }
            },
            settlement_rows(&admin, &call.envelope.ids.receipt_id).await,
        ));
    }
    let open_call = expansion_call(&store, &scope, /*suffix*/ 0x8e5b, open).await;
    let settled = expand_refused_edit_draft(&store, &open_call).await.unwrap();
    assert_eq!(
        (observed, settled.outcome.receipt_result()),
        (
            vec![
                (ReplayError::WriterIneligible, [0; 5]),
                (ReplayError::MissingProject, [0; 5]),
                (ReplayError::BindingConflict, [0; 5]),
            ],
            ReceiptResult::AuthoritativeApplied,
        )
    );
}

fn changed_admission_payload(receipt_id: &str) -> String {
    format!(
        "UPDATE storyos.author_command_admissions AS admission
            SET command_payload = admission.command_payload || '{{\"damaged\": true}}'::jsonb
           FROM storyos.domain_receipts AS receipt
          WHERE receipt.receipt_id = '{receipt_id}'
            AND admission.author_command_admission_id = receipt.author_command_admission_id"
    )
}

fn changed_draft_reference(receipt_id: &str) -> String {
    format!(
        "UPDATE storyos.domain_receipts SET draft_artifact_refs = ARRAY['{}']
          WHERE receipt_id = '{receipt_id}'",
        Uuid::now_v7()
    )
}

fn compensation_action(receipt_id: &str) -> String {
    format!(
        "UPDATE storyos.author_action_entries
            SET disposition = 'compensation', compensated_source_sequence = author_action_sequence
          WHERE receipt_id = '{receipt_id}'"
    )
}

fn zero_authority_action(receipt_id: &str) -> String {
    format!(
        "INSERT INTO storyos.author_action_entries
           (owner_user_id, project_id, author_action_sequence, disposition, receipt_id,
            receipt_result_kind)
         SELECT owner_user_id, project_id, 1000, 'forward', receipt_id, 'draft_closure_changed'
           FROM storyos.domain_receipts WHERE receipt_id = '{receipt_id}'"
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_draft_replay_binds_its_admission_and_draft_and_requires_its_author_action_shape() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut observed = Vec::new();
    for (base, damage) in [
        (0x9a00, changed_admission_payload as fn(&str) -> String),
        (0x9a10, changed_draft_reference),
        (0x9a20, compensation_action),
    ] {
        observed.push(
            damaged_replay(
                &store,
                &admin,
                &close_editor_flow_draft_call(&store, &admin, base).await,
                damage,
            )
            .await,
        );
        observed.push(
            damaged_replay(
                &store,
                &admin,
                &expand_refused_edit_draft_call(&store, &admin, base + 0x100).await,
                damage,
            )
            .await,
        );
    }
    let (scope, tombstoned) =
        refused_edit_draft(&store, &admin, /*base*/ 0x9a30, "tombstoned").await;
    let refused_discard = discard_call(&store, &scope, /*suffix*/ 0x9a39, tombstoned).await;
    observed.push(damaged_replay(&store, &admin, &refused_discard, zero_authority_action).await);
    let (scope, tombstoned) =
        refused_edit_expansion(&store, &admin, /*base*/ 0x9b30, "tombstoned").await;
    let refused_expansion = expansion_call(&store, &scope, /*suffix*/ 0x9b39, tombstoned).await;
    observed.push(damaged_replay(&store, &admin, &refused_expansion, zero_authority_action).await);
    assert_eq!(
        observed,
        vec![
            ReplayError::BindingConflict,
            ReplayError::BindingConflict,
            ReplayError::BindingConflict,
            ReplayError::BindingConflict,
            ReplayError::Unavailable,
            ReplayError::Unavailable,
            ReplayError::Unavailable,
            ReplayError::Unavailable,
        ]
    );
}

/// Settles the call and replays it once with `field` removed from its applied Receipt payload.
///
/// The Receipt payload checks refuse such a row. The helper drops them for the replay, and then
/// restores the payload and the checks.
async fn replay_without_receipt_payload_field<C: ProjectCommand + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
    field: &str,
) -> ReplayError {
    let Ok(_) = settle_project_command(store, &call.envelope, &call.input).await else {
        panic!("the project command must settle");
    };
    let receipt_id = &call.envelope.ids.receipt_id;
    let payload: String = admin
        .query_one(
            "SELECT result_payload::text FROM storyos.domain_receipts
              WHERE receipt_id = $1::text::uuid",
            &[receipt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let checks = admin
        .query(
            "SELECT conname::text, pg_get_constraintdef(oid) FROM pg_constraint
              WHERE conrelid = 'storyos.domain_receipts'::regclass AND contype = 'c'
                AND pg_get_constraintdef(oid) LIKE '%result_payload%'",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get::<_, String>(0), row.get::<_, String>(1)))
        .collect::<Vec<_>>();
    let dropped = checks
        .iter()
        .map(|(name, _)| format!("ALTER TABLE storyos.domain_receipts DROP CONSTRAINT {name};"))
        .collect::<String>();
    run_without_foreign_keys(
        admin,
        &format!(
            "{dropped}
             UPDATE storyos.domain_receipts SET result_payload = result_payload - '{field}'
              WHERE receipt_id = '{receipt_id}'"
        ),
    )
    .await;
    let retry = with_new_request_ids(call);
    let replayed = settle_project_command(store, &retry.envelope, &retry.input).await;
    let restored = checks
        .iter()
        .map(|(name, check)| {
            format!("ALTER TABLE storyos.domain_receipts ADD CONSTRAINT {name} {check};")
        })
        .collect::<String>();
    run_without_foreign_keys(
        admin,
        &format!(
            "UPDATE storyos.domain_receipts SET result_payload = '{payload}'::jsonb
              WHERE receipt_id = '{receipt_id}';
             {restored}"
        ),
    )
    .await;
    let Err(error) = replayed else {
        panic!("a replay without its applied payload field must fail");
    };
    error.into()
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_applied_replay_requires_the_payload_field_of_its_receipt() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        replay_without_receipt_payload_field(
            &store,
            &admin,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x9c00).await,
            "rejection_reason",
        )
        .await,
        replay_without_receipt_payload_field(
            &store,
            &admin,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x9c10).await,
            "transition",
        )
        .await,
        replay_without_receipt_payload_field(
            &store,
            &admin,
            &replan_proposal_call(&store, &admin, /*base*/ 0x9c20).await,
            "transition",
        )
        .await,
        replay_without_receipt_payload_field(
            &store,
            &admin,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x9c30).await,
            "transition",
        )
        .await,
        replay_without_receipt_payload_field(
            &store,
            &admin,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x9c40).await,
            "transition",
        )
        .await,
        replay_without_receipt_payload_field(
            &store,
            &admin,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x9c50).await,
            "transition",
        )
        .await,
        replay_without_receipt_payload_field(
            &store,
            &admin,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x9c60).await,
            "transition",
        )
        .await,
        replay_without_receipt_payload_field(
            &store,
            &admin,
            &close_editor_flow_draft_call(&store, &admin, /*base*/ 0x9c70).await,
            "reason",
        )
        .await,
        replay_without_receipt_payload_field(
            &store,
            &admin,
            &expand_refused_edit_draft_call(&store, &admin, /*base*/ 0x9c80).await,
            "reason",
        )
        .await,
    ];
    assert_eq!(observed, vec![ReplayError::Unavailable; 9]);
}
