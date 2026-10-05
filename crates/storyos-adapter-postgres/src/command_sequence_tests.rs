use std::fmt::Debug;

use storyos_application::{
    ArchiveProjectInput, AuthorCommandAdmissionIds, CloseEditorFlowDraftInput,
    CloseEditorFlowDraftSettlement, CreateChapterInput, CreateChapterSettlement, CreateVolumeInput,
    CreateVolumeSettlement, DeleteChapterInput, DeleteChapterSettlement, DeleteVolumeInput,
    DeleteVolumeSettlement, EditorClientBinding, ProjectCommandChallengeBinding,
    ProjectCommandEnvelope, ProjectCommandError, ProjectCommandSettlement, ProjectScope,
    RejectProposalOperationsInput, RejectProposalOperationsSettlement, RejectionNote,
    ReopenRejectedOperationsInput, ReopenRejectedOperationsSettlement,
    ReopenWithdrawnProposalInput, ReopenWithdrawnProposalSettlement, ReplanProposalInput,
    ReplanProposalSettlement, SetCurrentChapterInput, StructureAuthority,
    StructureAuthorityEvidence, StructureSettlement, UpdateChapterInput, UpdateChapterSettlement,
    UpdateProjectAssistanceInput, UpdateProjectInput, UpdateVolumeInput, UpdateVolumeSettlement,
    WithdrawProposalInput, WithdrawProposalSettlement, WithdrawalNote,
    issue_project_command_challenge,
};
use storyos_application::{
    ChapterId, EditorSessionId, IssueProjectCommandChallenge, OpenChapter, VolumeId, open_chapter,
};
use storyos_application::{TakeOverProjectWriterInput, TakeOverProjectWriterSettlement};
use storyos_core::{
    AssistanceAvailability, CreateChapterPlacement, ReasonCode, ReceiptResult, TransitionOutcome,
};
use tokio_postgres::{Client, NoTls};
use uuid::Uuid;

use super::{
    ActivitySequences, Classification, CommandSpec, LockedProject, ProfileSequences, ProfileWrite,
    ProjectCommand, ZeroAuthorityRows, ZeroAuthorityWrite, ZeroOutcome, settle_project_command,
    unavailable,
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
    let (scope, open) = refused_edit_draft(&store, &admin, /*base*/ 0x7c00, "retained").await;
    let changed = CloseEditorFlowDraftInput {
        source_current_draft_revision_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let (archived_scope, archived) =
        refused_edit_draft(&store, &admin, /*base*/ 0x7c10, "archived").await;
    for (scope, suffix, input) in [
        (&scope, 0x7c09, open.clone()),
        (&scope, 0x7c0a, open),
        (&scope, 0x7c0b, changed),
        (&archived_scope, 0x7c19, archived),
    ] {
        let call = discard_call(&store, scope, suffix, input).await;
        let outcome = replayed_outcome(&store, &admin, &call, close_editor_flow_draft).await;
        observed.push((CLOSE_EDITOR_FLOW_DRAFT.kind, outcome));
    }

    // Receipt, Author Action, Activity, Commit, and Snapshot rows of each outcome.
    let structural_applied = ("authoritative_applied", [1, 1, 1, 1, 1]);
    let chapter_selection_applied = ("authoritative_applied", [1, 1, 1, 0, 1]);
    let activity_applied = ("authoritative_applied", [1, 0, 1, 0, 0]);
    let proposal_revised = ("proposal_revised", [1, 1, 0, 0, 0]);
    let proposal_closure_changed = ("proposal_closure_changed", [1, 1, 0, 0, 0]);
    let operations_resolved = ("proposal_operations_resolved", [1, 1, 0, 0, 0]);
    let draft_closure_changed = ("draft_closure_changed", [1, 1, 0, 0, 0]);
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
        ]
    );
    assert_eq!(rejection_records, vec![1; 4]);
    assert_eq!(withdrawal_records, vec![1, 0, 0, 0]);
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
            &close_editor_flow_draft_call(&store, &admin, /*base*/ 0x7c20).await,
        )
        .await,
    ];
    assert_eq!(observed, vec![(true, [0; 5]); 17]);
}

#[derive(Clone, Copy, Debug)]
enum FailurePoint {
    Classify,
    Apply,
}

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
            FailurePoint::Classify => Err(unavailable("injected classify failure")),
            FailurePoint::Apply => Ok(Classification {
                outcome: classified.outcome,
                admission: classified.admission,
                heads: classified.heads,
                zero_receipt: classified.zero_receipt,
            }),
        }
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
        self.command
            .apply(client, envelope, project, sequences, plan, applied)
            .await?;
        Err(unavailable("injected apply failure"))
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
        Err(unavailable("injected zero-authority write failure"))
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
        Err(unavailable("injected zero-authority write failure"))
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

/// Fails the call in classify and then in apply, and then settles it for real.
///
/// Returns, for each failure, whether it is a store fault and the rows of its Receipt, then the
/// Receipt result kind of the real settlement.
async fn failed_then_settled<C: ProjectCommand + Clone>(
    store: &PostgresProjectReader,
    admin: &Client,
    call: &CommandCall<C>,
) -> (Vec<(bool, [i64; 5])>, ReceiptResult) {
    let mut failures = Vec::new();
    for at in [FailurePoint::Classify, FailurePoint::Apply] {
        let failing = Failing {
            command: call.input.clone(),
            at,
        };
        let failed = settle_project_command(store, &call.envelope, &failing).await;
        failures.push((
            matches!(failed, Err(ProjectCommandError::Unavailable(_))),
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
            &close_editor_flow_draft_call(&store, &admin, /*base*/ 0x7c30).await,
        )
        .await,
    ];
    let rolled_back = |result| (vec![(true, [0; 5]), (true, [0; 5])], result);
    let mut expected = vec![rolled_back(ReceiptResult::AuthoritativeApplied); 10];
    expected.push(rolled_back(ReceiptResult::NoEffect));
    expected.push(rolled_back(ReceiptResult::AuthoritativeApplied));
    expected.push(rolled_back(ReceiptResult::AuthoritativeApplied));
    expected.push(rolled_back(ReceiptResult::Refused));
    expected.extend(vec![rolled_back(ReceiptResult::AuthoritativeApplied); 3]);
    expected.push(rolled_back(ReceiptResult::AuthoritativeApplied));
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
        errors.push(match error {
            ProjectCommandError::BindingConflict => ReplayError::BindingConflict,
            ProjectCommandError::HistoricalAcknowledgementUnavailable => {
                ReplayError::HistoricalAcknowledgementUnavailable
            }
            ProjectCommandError::InvalidChallenge => ReplayError::InvalidChallenge,
            ProjectCommandError::MissingProject => ReplayError::MissingProject,
            ProjectCommandError::WriterIneligible => ReplayError::WriterIneligible,
            ProjectCommandError::Unavailable(_) => ReplayError::Unavailable,
        });
    }
    let errors: [ReplayError; 2] = errors.try_into().unwrap();
    (settled.outcome.receipt_result(), errors)
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
    ];
    let separated = (
        ReceiptResult::AuthoritativeApplied,
        [
            ReplayError::HistoricalAcknowledgementUnavailable,
            ReplayError::Unavailable,
        ],
    );
    assert_eq!(observed, vec![separated; 15]);
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
    let call = close_editor_flow_draft_call(&store, &admin, /*base*/ 0x7c40).await;
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
    let (scope, open) = refused_edit_draft(&store, &admin, /*base*/ 0x7c50, "retained").await;
    let stale_writer = CloseEditorFlowDraftInput {
        writer_generation: 2,
        ..open.clone()
    };
    let missing = CloseEditorFlowDraftInput {
        draft_id: Uuid::now_v7().to_string(),
        ..open.clone()
    };
    let (damaged_scope, damaged) =
        refused_edit_draft(&store, &admin, /*base*/ 0x7c60, "retained").await;
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
        (&scope, 0x7c59, stale_writer),
        (&scope, 0x7c5a, missing),
        (&damaged_scope, 0x7c69, damaged),
    ] {
        let call = discard_call(&store, scope, suffix, input).await;
        let refused = close_editor_flow_draft(&store, &call).await;
        observed.push((
            match refused {
                Err(ProjectCommandError::WriterIneligible) => ReplayError::WriterIneligible,
                Err(ProjectCommandError::MissingProject) => ReplayError::MissingProject,
                Err(ProjectCommandError::BindingConflict) => ReplayError::BindingConflict,
                other => panic!("the Discard must refuse before Admission, got {other:?}"),
            },
            settlement_rows(&admin, &call.envelope.ids.receipt_id).await,
        ));
    }
    let open_call = discard_call(&store, &scope, /*suffix*/ 0x7c5b, open).await;
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
