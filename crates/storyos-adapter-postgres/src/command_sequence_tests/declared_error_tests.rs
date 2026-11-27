use std::marker::PhantomData;
use storyos_contracts::CommandKind;

use crate::statement_cache::Client;
use storyos_application::AuthorEditError;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::author_edit::{AuthorEdit, AuthorEditFault, author_edit_envelope};
use crate::command_sequence::{
    CommandError, ProjectCommand, admit_and_settle, settle_project_command,
};
use crate::shared_tests::create_volume_authority::{NamedEdit, named_edit_command};

use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;
use crate::shared_tests::command_sequence_acceptance::{
    accept_proposal_call, invalid_acceptance_call,
};
use crate::shared_tests::command_sequence_agent_run::{
    cancel_agent_run_call, create_agent_run_call, pause_agent_run_call,
};
use crate::shared_tests::command_sequence_project_session::{
    update_project_assistance_call, update_project_call,
};
use crate::shared_tests::command_sequence_proposal_decision::{
    reject_proposal_operations_call, withdraw_proposal_call,
};
use crate::shared_tests::command_sequence_rollback::{
    Failing, FailurePoint, UnchallengedKind, WrappedSpec,
};
use crate::shared_tests::command_sequence_steer_agent_run::steer_agent_run_call;
use crate::shared_tests::command_sequence_structure::{
    create_chapter_call, create_volume_call, delete_chapter_call, delete_volume_call,
    set_current_chapter_call, update_chapter_call, update_volume_call,
};
use crate::shared_tests::command_sequence_support::{CommandCall, stores, two_chapter_writer};
use crate::shared_tests::command_sequence_undo::{
    Case as UndoCase, session_chapter_head, undo_call,
};

/// The reason codes that one command declares for two sequence failures before its Receipt.
#[derive(Debug, PartialEq)]
struct DeclaredErrors {
    /// The reason code of a serialization failure, and the SQLSTATE of a store fault.
    contention: (&'static str, Option<String>),
    /// The reason code of an Admission insert that writes no row.
    missing_admission: &'static str,
}

/// Settles the call once with a serialization failure after its classify step, and once with an
/// Admission insert that writes no row. Returns the command kind and both errors.
async fn declared_errors<C: ProjectCommand + Clone>(
    store: &PostgresProjectReader,
    call: &CommandCall<C>,
) -> (CommandKind, DeclaredErrors) {
    let contended = Failing {
        command: call.input.clone(),
        at: FailurePoint::ContendedClassify,
        spec: PhantomData::<WrappedSpec>,
    };
    let Err(contention) = settle_project_command(store, &call.envelope, &contended).await else {
        panic!("a contended {} must fail", C::SPEC.kind.as_str());
    };
    let unadmitted = Failing {
        command: call.input.clone(),
        at: FailurePoint::NoFailure,
        spec: PhantomData::<UnchallengedKind>,
    };
    let Err(missing_admission) = settle_project_command(store, &call.envelope, &unadmitted).await
    else {
        panic!("an unadmitted {} must fail", C::SPEC.kind.as_str());
    };
    let sqlstate = match contention.sequence_error() {
        Some(storyos_application::ProjectCommandError::Unavailable(source)) => {
            database_code(source.as_ref())
        }
        Some(_) | None => None,
    };
    (
        C::SPEC.kind,
        DeclaredErrors {
            contention: (contention.reason_code(), sqlstate),
            missing_admission: missing_admission.reason_code(),
        },
    )
}

/// The SQLSTATE of a store fault whose source is a database error.
fn database_code(source: &(dyn std::error::Error + Send + Sync + 'static)) -> Option<String> {
    source
        .downcast_ref::<tokio_postgres::Error>()
        .and_then(tokio_postgres::Error::code)
        .map(|code| code.code().to_owned())
}

/// Settles one Author Edit through its admit step and its settle step with a serialization
/// failure after its classify step. Then settles a second Author Edit of the same Editor Session
/// with an Admission insert that writes no row. Returns the command kind and both errors.
async fn author_edit_declared_errors(
    store: &PostgresProjectReader,
    admin: &Client,
    base: u16,
) -> (CommandKind, DeclaredErrors) {
    let (scope, chapter_a, _chapter_b, _revision_b, editor_session_id) =
        two_chapter_writer(store, base).await;
    let head = session_chapter_head(admin, &editor_session_id).await;
    let named = |suffix, local_intent_sequence| NamedEdit {
        editor_session_id: &editor_session_id,
        chapter_id: &chapter_a,
        expected_revision_id: &head,
        suffix,
        local_intent_sequence,
        text: "x",
        proposal_target: None,
    };
    let contended_suffix = format!("{:04x}", base + 9);
    let unadmitted_suffix = format!("{:04x}", base + 11);
    let contended = named_edit_command(store, &scope, named(&contended_suffix, 1)).await;
    let unadmitted = named_edit_command(store, &scope, named(&unadmitted_suffix, 2)).await;
    let Err(contention) = admit_and_settle(
        store,
        &author_edit_envelope(&contended),
        &Failing {
            command: AuthorEdit::new(&contended, AuthorEditFault::None),
            at: FailurePoint::ContendedClassify,
            spec: PhantomData::<WrappedSpec>,
        },
        || Ok(()),
    )
    .await
    else {
        panic!("a contended Author Edit must fail");
    };
    let Err(missing_admission) = admit_and_settle(
        store,
        &author_edit_envelope(&unadmitted),
        &Failing {
            command: AuthorEdit::new(&unadmitted, AuthorEditFault::None),
            at: FailurePoint::NoFailure,
            spec: PhantomData::<UnchallengedKind>,
        },
        || Ok(()),
    )
    .await
    else {
        panic!("an unadmitted Author Edit must fail");
    };
    let sqlstate = match &contention.0 {
        AuthorEditError::Unavailable(source) => database_code(source.as_ref()),
        AuthorEditError::BindingConflict
        | AuthorEditError::InvalidChallenge
        | AuthorEditError::AdmissionExpired => None,
    };
    (
        AuthorEdit::SPEC.kind,
        DeclaredErrors {
            contention: (contention.reason_code(), sqlstate),
            missing_admission: missing_admission.reason_code(),
        },
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_command_returns_its_declared_contention_and_missing_admission_errors() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    let (store, admin) = stores().await;
    let observed = vec![
        declared_errors(&store, &create_volume_call(&store, /*base*/ 0x9300).await).await,
        declared_errors(&store, &update_volume_call(&store, /*base*/ 0x9310).await).await,
        declared_errors(&store, &delete_volume_call(&store, /*base*/ 0x9320).await).await,
        declared_errors(&store, &create_chapter_call(&store, /*base*/ 0x9330).await).await,
        declared_errors(&store, &update_chapter_call(&store, /*base*/ 0x9340).await).await,
        declared_errors(&store, &delete_chapter_call(&store, /*base*/ 0x9350).await).await,
        declared_errors(&store, &update_project_call(&store, /*base*/ 0x9360).await).await,
        declared_errors(
            &store,
            &set_current_chapter_call(&store, /*base*/ 0x9380).await,
        )
        .await,
        declared_errors(
            &store,
            &update_project_assistance_call(&store, /*base*/ 0x9390).await,
        )
        .await,
        declared_errors(
            &store,
            &reject_proposal_operations_call(&store, &admin, /*base*/ 0x93c0).await,
        )
        .await,
        declared_errors(&store, &{
            let mut stale = reject_proposal_operations_call(&store, &admin, /*base*/ 0x93d0).await;
            stale.input.proposal_revision_id = Uuid::now_v7().to_string();
            stale
        })
        .await,
        declared_errors(
            &store,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x9400).await,
        )
        .await,
        declared_errors(
            &store,
            &pause_agent_run_call(&store, &admin, /*base*/ 0x9430).await,
        )
        .await,
        declared_errors(
            &store,
            &cancel_agent_run_call(&store, &admin, /*base*/ 0x9440).await,
        )
        .await,
        declared_errors(
            &store,
            &create_agent_run_call(&store, /*base*/ 0x9450).await,
        )
        .await,
        declared_errors(
            &store,
            &steer_agent_run_call(&store, &admin, /*base*/ 0x9460).await,
        )
        .await,
        declared_errors(
            &store,
            &accept_proposal_call(&store, &admin, /*base*/ 0x9470).await,
        )
        .await,
        declared_errors(
            &store,
            &invalid_acceptance_call(&store, &admin, /*base*/ 0x9480).await,
        )
        .await,
        declared_errors(
            &store,
            &undo_call(
                &store,
                &admin,
                UndoCase::SetCurrentChapter,
                /*base*/ 0x8c00,
            )
            .await,
        )
        .await,
        author_edit_declared_errors(&store, &admin, /*base*/ 0x8c10).await,
    ];
    let expected = [
        (CommandKind::CreateVolume, None, "invalid_challenge"),
        (CommandKind::UpdateVolume, None, "invalid_challenge"),
        (CommandKind::DeleteVolume, None, "invalid_challenge"),
        (CommandKind::CreateChapter, None, "invalid_challenge"),
        (CommandKind::UpdateChapter, None, "invalid_challenge"),
        (CommandKind::DeleteChapter, None, "invalid_challenge"),
        (CommandKind::UpdateProject, None, "invalid_challenge"),
        (CommandKind::SetCurrentChapter, None, "invalid_challenge"),
        (
            CommandKind::UpdateProjectAssistance,
            None,
            "invalid_challenge",
        ),
        (
            CommandKind::RejectProposalOperations,
            None,
            "invalid_challenge",
        ),
        (
            CommandKind::RejectProposalOperations,
            None,
            "invalid_challenge",
        ),
        (CommandKind::WithdrawProposal, None, "invalid_challenge"),
        (CommandKind::PauseAgentRun, None, "invalid_challenge"),
        (CommandKind::CancelAgentRun, None, "invalid_challenge"),
        (
            CommandKind::CreateAgentRun,
            Some("conversation_busy"),
            "invalid_challenge",
        ),
        (CommandKind::SteerAgentRun, None, "invalid_challenge"),
        (CommandKind::AcceptProposal, None, "invalid_challenge"),
        (CommandKind::AcceptProposal, None, "invalid_challenge"),
        (
            CommandKind::UndoLatestAuthorAction,
            None,
            "invalid_challenge",
        ),
        (CommandKind::ApplyAuthorEdit, None, "binding_conflict"),
    ]
    .map(|(kind, contention_refusal, missing_admission)| {
        let contention = contention_refusal.map_or_else(
            || ("unavailable", Some("40001".to_owned())),
            |refusal| (refusal, None),
        );
        (
            kind,
            DeclaredErrors {
                contention,
                missing_admission,
            },
        )
    });
    assert_eq!(observed, expected);
}
