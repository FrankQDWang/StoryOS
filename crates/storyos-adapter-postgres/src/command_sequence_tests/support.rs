use std::fmt::Debug;

use storyos_application::{
    AuthorCommandAdmissionIds, CreateProjectChallengeBinding, CreateProjectCommand,
    EditorClientBinding, IssueCreateProjectChallenge, ProjectCommandChallengeBinding,
    ProjectCommandEnvelope, ProjectCommandError, ProjectCommandSettlement, ProjectId, ProjectScope,
    RefusableCommandError, StructureAuthority, StructureAuthorityEvidence, StructureSettlement,
    UserId, create_project, issue_create_project_challenge, issue_project_command_challenge,
};
use storyos_application::{
    ChallengeRateClass, ChapterId, EditorSessionId, IssueProjectCommandChallenge, OpenChapter,
    UndoLatestAuthorActionInput, UndoLatestAuthorActionSettlement, open_chapter,
};
use storyos_core::{ReasonCode, TransitionOutcome};
use tokio_postgres::{Client, NoTls};
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::set_current_chapter_authority_tests::{open_session, seed_two_chapters};

use super::undo::UNDO_LATEST_AUTHOR_ACTION;

/// The sequence error of a settlement that a contract row expects.
pub(crate) trait SequenceError {
    /// Panics on a refusal before Admission.
    fn sequence(self) -> ProjectCommandError;
}

impl SequenceError for ProjectCommandError {
    fn sequence(self) -> Self {
        self
    }
}

impl<R: Debug> SequenceError for RefusableCommandError<R> {
    fn sequence(self) -> ProjectCommandError {
        match self {
            Self::Command(error) => error,
            Self::RefusedBeforeAdmission(refusal) => {
                panic!("the command refused before its Admission: {refusal:?}")
            }
        }
    }
}

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

/// The applied effect and settled authority of one structure command. Panics on any other outcome.
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

pub(super) async fn stores() -> (PostgresProjectReader, Client) {
    let runtime_url = std::env::var("STORYOS_TEST_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let admin_url = std::env::var("STORYOS_TEST_ADMIN_DATABASE_URL")
        .expect("run through scripts/verify-project-scope.sh");
    let (admin, connection) = tokio_postgres::connect(&admin_url, NoTls).await.unwrap();
    tokio::spawn(connection);
    (PostgresProjectReader::new(runtime_url), admin)
}

/// Counts the Receipt, Author Action, Activity, Commit, and Snapshot rows of one Receipt.
///
/// The Activity count includes both Project Activity tables.
pub(super) async fn settlement_rows(admin: &Client, receipt_id: &str) -> [i64; 5] {
    let row = admin
        .query_one(
            "WITH activity AS (
               SELECT owner_user_id, project_id, project_activity_position
                 FROM storyos.project_activity_event_payloads
                WHERE receipt_id = $1::text::uuid
               UNION ALL
               SELECT owner_user_id, project_id, project_activity_position
                 FROM storyos.project_activity_events
                WHERE receipt_id = $1::text::uuid
             )
             SELECT (SELECT count(*) FROM storyos.domain_receipts WHERE receipt_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.author_action_entries
                      WHERE receipt_id = $1::text::uuid),
                    (SELECT count(*) FROM activity),
                    (SELECT count(*) FROM storyos.authoritative_commits
                      WHERE receipt_id = $1::text::uuid),
                    (SELECT count(*) FROM storyos.project_snapshots AS snapshot
                       JOIN activity
                         ON (activity.owner_user_id, activity.project_id,
                             activity.project_activity_position) =
                            (snapshot.owner_user_id, snapshot.project_id,
                             snapshot.project_activity_position))",
            &[&receipt_id],
        )
        .await
        .unwrap();
    [0, 1, 2, 3, 4].map(|index| row.get(index))
}

const CLIENT_CONTRACT_REVISION: &str = "storyos.web-client.release-1.v3";
const SECURITY_POLICY_REVISION: &str = "storyos.web-security-policy.release-1.v1";

/// The Create Project Challenge of `owner` whose Project, digest, and nonce derive from
/// `project_suffix`. It is not issued.
pub(crate) fn create_project_issue(
    owner: &str,
    idempotency_key: &str,
    project_suffix: &str,
) -> IssueCreateProjectChallenge {
    let digest = format!("sha256:storyos.command.createProject.jcs.v1:{project_suffix:0<64}");
    IssueCreateProjectChallenge {
        binding: CreateProjectChallengeBinding {
            owner_user_id: UserId::new(owner),
            prospective_project_id: ProjectId::new(format!(
                "018f0000-0000-7001-8000-00000000{project_suffix}"
            )),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT_CONTRACT_REVISION.to_owned(),
            security_policy_revision: SECURITY_POLICY_REVISION.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: "/api/v1/projects".to_owned(),
            command_schema: "storyos.command.create-project.request.v1".to_owned(),
            command_kind: "createProject".to_owned(),
            create_input_digest: format!("{project_suffix:0<64}"),
            canonical_command_digest: digest,
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: format!("sha256:nonce-{project_suffix}"),
    }
}

/// The Create Project command of an empty Project whose ids derive from `ids_suffix`.
pub(crate) fn create_project_command(
    binding: CreateProjectChallengeBinding,
    nonce_digest: &str,
    ids_suffix: &str,
) -> CreateProjectCommand {
    CreateProjectCommand {
        project_scope: ProjectScope::new(
            binding.owner_user_id.clone(),
            binding.prospective_project_id.clone(),
        ),
        client_binding: EditorClientBinding {
            binding_ref: binding.client_session_binding_digest.clone(),
            session_generation: binding.client_session_generation,
            client_contract_revision: binding.client_contract_revision.clone(),
            security_policy_revision: binding.security_policy_revision.clone(),
        },
        challenge_binding: binding,
        nonce_digest: nonce_digest.to_owned(),
        canonical_command_bytes: br#"{"title":"Empty Novel"}"#.to_vec(),
        correlation_id: format!("018f0000-0000-7001-8000-00000000{ids_suffix}"),
        title: "Empty Novel".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: format!("018f0000-0000-7001-8000-00000001{ids_suffix}"),
            author_command_admission_id: format!("018f0000-0000-7001-8000-00000002{ids_suffix}"),
            receipt_id: format!("018f0000-0000-7001-8000-00000003{ids_suffix}"),
        },
    }
}

/// Creates the empty Project of User A whose ids derive from `project_suffix`.
pub(crate) async fn seed_project(
    store: &PostgresProjectReader,
    project_suffix: &str,
) -> ProjectScope {
    seed_owned_project(
        store,
        "018f0000-0000-7001-8000-000000000001",
        project_suffix,
    )
    .await
}

/// Creates the empty Project of `owner` whose ids derive from `project_suffix`.
pub(crate) async fn seed_owned_project(
    store: &PostgresProjectReader,
    owner: &str,
    project_suffix: &str,
) -> ProjectScope {
    let issue = create_project_issue(
        owner,
        &format!("018f0000-0000-7001-8000-00000000{project_suffix}"),
        project_suffix,
    );
    let issued = issue_create_project_challenge(store, &issue).await.unwrap();
    let mut binding = issue.binding.clone();
    binding.prospective_project_id = issued.prospective_project_id.clone();
    binding.canonical_command_digest = issued.canonical_command_digest.clone();
    create_project(
        store,
        &create_project_command(binding.clone(), &issue.nonce_digest, project_suffix),
    )
    .await
    .unwrap();
    ProjectScope::new(binding.owner_user_id, binding.prospective_project_id)
}

/// The public route identity of one project command kind.
pub(crate) struct Route {
    pub(crate) kind: &'static str,
    pub(crate) method: &'static str,
    pub(crate) path: &'static str,
    pub(crate) schema: &'static str,
}

pub(crate) const CREATE_EDITOR_SESSION: Route = Route {
    kind: "createEditorSession",
    method: storyos_contracts::CREATE_EDITOR_SESSION_METHOD,
    path: storyos_contracts::CREATE_EDITOR_SESSION_PATH,
    schema: storyos_contracts::CREATE_EDITOR_SESSION_REQUEST_SCHEMA_ID,
};

/// Issues one Command Challenge for `route` and binds `input` to it.
pub(super) async fn issued<I>(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    route: &Route,
    input: I,
) -> CommandCall<I> {
    let digest = format!("sha256:{}:{suffix:04x}", route.kind);
    issued_with_bytes(store, scope, suffix, route, input, b"{}", digest).await
}

/// Issues one Command Challenge for `route` whose canonical bytes and digest are `bytes` and
/// `digest`, and binds `input` to it.
pub(super) async fn issued_with_bytes<I>(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    suffix: u16,
    route: &Route,
    input: I,
    bytes: &[u8],
    digest: String,
) -> CommandCall<I> {
    let issue = challenge_issue(scope, suffix, route, digest);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    command_call(
        issue.binding,
        &issue.nonce_digest,
        &format!("{suffix:04x}"),
        bytes,
        input,
    )
}

/// The Command Challenge for `route` whose ids derive from `suffix`. It is not issued.
pub(super) fn challenge_issue(
    scope: &ProjectScope,
    suffix: u16,
    route: &Route,
    digest: String,
) -> IssueProjectCommandChallenge {
    command_issue(scope, &format!("{suffix:04x}"), route, &digest)
}

/// The Command Challenge for `route` whose ids derive from the four hex digits `suffix`. It is
/// not issued.
pub(crate) fn command_issue(
    scope: &ProjectScope,
    suffix: &str,
    route: &Route,
    digest: &str,
) -> IssueProjectCommandChallenge {
    IssueProjectCommandChallenge {
        binding: ProjectCommandChallengeBinding {
            project_scope: scope.clone(),
            client_session_binding_digest: "sha256:session-a".to_owned(),
            client_session_generation: 1,
            client_contract_revision: CLIENT_CONTRACT_REVISION.to_owned(),
            security_policy_revision: SECURITY_POLICY_REVISION.to_owned(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            // The Server derives the rate class from the command kind.
            challenge_rate_policy_revision: ChallengeRateClass::for_command_kind(route.kind)
                .policy_revision()
                .to_owned(),
            method: route.method.to_owned(),
            route_template: route.path.to_owned(),
            command_schema: route.schema.to_owned(),
            command_kind: route.kind.to_owned(),
            canonical_command_digest: digest.to_owned(),
            idempotency_key: format!("018f0000-0000-7001-8000-00000000{suffix}"),
        },
        nonce: format!("opaque-nonce-{suffix}"),
        nonce_digest: format!("sha256:nonce-{suffix}"),
        earlier_revision_nonces: Vec::new(),
    }
}

/// Settles one Author Undo of the Author Undo Frontier `expected_frontier`, whose ids derive from
/// `suffix`.
pub(crate) async fn undo_named(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
    editor_session_id: &str,
    expected_frontier: u64,
    expected_revision_id: &str,
    suffix: &str,
) -> UndoLatestAuthorActionSettlement {
    let input = serde_json::json!({
        "expected_author_undo_frontier_sequence": expected_frontier.to_string(),
        "expected_authoritative_revision_id": expected_revision_id,
        "editor_session_id": editor_session_id,
        "client_contract_revision": CLIENT_CONTRACT_REVISION,
        "security_policy_revision": SECURITY_POLICY_REVISION,
        "correlation_id": format!("018f0000-0000-7001-8000-00000000{suffix}"),
    });
    let body = serde_json::json!({
        "command_schema": "storyos.command.undo-latest-author-action.request.v1",
        "undo_latest_author_action_input": input,
    });
    let bytes = serde_json::to_vec(&body).unwrap();
    let digest = format!(
        "sha256:storyos.command.undoLatestAuthorAction.jcs.v1:{}",
        crate::author_edit::sha256_hex(&bytes)
    );
    let issue = command_issue(scope, suffix, &UNDO_LATEST_AUTHOR_ACTION, &digest);
    issue_project_command_challenge(store, &issue)
        .await
        .unwrap();
    let call = command_call(
        issue.binding,
        &issue.nonce_digest,
        suffix,
        &bytes,
        UndoLatestAuthorActionInput {
            editor_session_id: EditorSessionId::new(editor_session_id),
            expected_author_undo_frontier_sequence: expected_frontier,
            expected_authoritative_revision_id: expected_revision_id.to_owned(),
        },
    );
    store
        .undo_latest_author_action(&call.envelope, &call.input)
        .await
        .unwrap()
}

/// Settles one call, replays it with new request identities, and requires an equal settlement.
///
/// Returns the stored Receipt result kind and the authority rows of the first settlement.
pub(super) async fn replayed_outcome<I: Clone, A, N, C, R, P, Z>(
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

/// A new Project with Chapters A and B, Chapter A current, and one writer Editor Session.
///
/// Returns the Scope, both Chapters, the head of Chapter B, and the Editor Session.
pub(super) async fn two_chapter_writer(
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

/// Runs fixture statements in one transaction without their foreign keys.
pub(super) async fn run_without_foreign_keys(admin: &Client, statements: &str) {
    admin
        .batch_execute(&format!(
            "BEGIN; SET LOCAL session_replication_role = replica; {statements}; COMMIT;"
        ))
        .await
        .unwrap();
}

pub(super) fn with_new_request_ids<I: Clone>(call: &CommandCall<I>) -> CommandCall<I> {
    let mut retry = call.clone();
    retry.envelope.ids = AuthorCommandAdmissionIds {
        command_id: Uuid::now_v7().to_string(),
        author_command_admission_id: Uuid::now_v7().to_string(),
        receipt_id: Uuid::now_v7().to_string(),
    };
    retry
}
