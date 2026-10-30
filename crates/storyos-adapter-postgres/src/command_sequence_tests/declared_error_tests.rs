use std::marker::PhantomData;

use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_sequence::{CommandError, ProjectCommand, settle_project_command};

use super::acceptance::{accept_proposal_call, invalid_acceptance_call};
use super::agent_run::{cancel_agent_run_call, create_agent_run_call, pause_agent_run_call};
use super::project_session::{
    archive_project_call, take_over_project_writer_call, update_project_assistance_call,
    update_project_call,
};
use super::proposal_decision::{
    reject_proposal_operations_call, reopen_rejected_operations_call,
    reopen_withdrawn_proposal_call, replan_proposal_call, withdraw_proposal_call,
};
use super::proposal_generation::{
    complete_ready_partial_proposal_call, continue_proposal_generation_call,
};
use super::rollback::{Failing, FailurePoint, UnchallengedKind, WrappedSpec};
use super::steer_agent_run::steer_agent_run_call;
use super::structure::{
    create_chapter_call, create_volume_call, delete_chapter_call, delete_volume_call,
    set_current_chapter_call, update_chapter_call, update_volume_call,
};
use super::support::{CommandCall, stores};

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
) -> (&'static str, DeclaredErrors) {
    let contended = Failing {
        command: call.input.clone(),
        at: FailurePoint::ContendedClassify,
        spec: PhantomData::<WrappedSpec>,
    };
    let Err(contention) = settle_project_command(store, &call.envelope, &contended).await else {
        panic!("a contended {} must fail", C::SPEC.kind);
    };
    let unadmitted = Failing {
        command: call.input.clone(),
        at: FailurePoint::NoFailure,
        spec: PhantomData::<UnchallengedKind>,
    };
    let Err(missing_admission) = settle_project_command(store, &call.envelope, &unadmitted).await
    else {
        panic!("an unadmitted {} must fail", C::SPEC.kind);
    };
    let sqlstate = match contention.sequence_error() {
        Some(storyos_application::ProjectCommandError::Unavailable(source)) => source
            .downcast_ref::<tokio_postgres::Error>()
            .and_then(tokio_postgres::Error::code)
            .map(|code| code.code().to_owned()),
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_command_returns_its_declared_contention_and_missing_admission_errors() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let observed = vec![
        declared_errors(&store, &create_volume_call(&store, /*base*/ 0x9300).await).await,
        declared_errors(&store, &update_volume_call(&store, /*base*/ 0x9310).await).await,
        declared_errors(&store, &delete_volume_call(&store, /*base*/ 0x9320).await).await,
        declared_errors(&store, &create_chapter_call(&store, /*base*/ 0x9330).await).await,
        declared_errors(&store, &update_chapter_call(&store, /*base*/ 0x9340).await).await,
        declared_errors(&store, &delete_chapter_call(&store, /*base*/ 0x9350).await).await,
        declared_errors(&store, &update_project_call(&store, /*base*/ 0x9360).await).await,
        declared_errors(&store, &archive_project_call(&store, /*base*/ 0x9370).await).await,
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
            &take_over_project_writer_call(&store, /*base*/ 0x93a0).await,
        )
        .await,
        declared_errors(
            &store,
            &reopen_withdrawn_proposal_call(&store, &admin, /*base*/ 0x93b0).await,
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
            &replan_proposal_call(&store, &admin, /*base*/ 0x93e0).await,
        )
        .await,
        declared_errors(
            &store,
            &reopen_rejected_operations_call(&store, &admin, /*base*/ 0x93f0).await,
        )
        .await,
        declared_errors(
            &store,
            &withdraw_proposal_call(&store, &admin, /*base*/ 0x9400).await,
        )
        .await,
        declared_errors(
            &store,
            &complete_ready_partial_proposal_call(&store, &admin, /*base*/ 0x9410).await,
        )
        .await,
        declared_errors(
            &store,
            &continue_proposal_generation_call(&store, &admin, /*base*/ 0x9420).await,
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
    ];
    let expected = [
        ("createVolume", None, "invalid_challenge"),
        ("updateVolume", None, "invalid_challenge"),
        ("deleteVolume", None, "invalid_challenge"),
        ("createChapter", None, "invalid_challenge"),
        ("updateChapter", None, "invalid_challenge"),
        ("deleteChapter", None, "invalid_challenge"),
        ("updateProject", None, "invalid_challenge"),
        ("archiveProject", None, "invalid_challenge"),
        ("setCurrentChapter", None, "invalid_challenge"),
        ("updateProjectAssistance", None, "invalid_challenge"),
        ("takeOverProjectWriter", None, "binding_conflict"),
        ("reopenWithdrawnProposal", None, "invalid_challenge"),
        ("rejectProposalOperations", None, "invalid_challenge"),
        ("rejectProposalOperations", None, "invalid_challenge"),
        ("replanProposal", None, "invalid_challenge"),
        ("reopenRejectedOperations", None, "invalid_challenge"),
        ("withdrawProposal", None, "invalid_challenge"),
        ("completeReadyPartialProposal", None, "invalid_challenge"),
        ("continueProposalGeneration", None, "invalid_challenge"),
        ("pauseAgentRun", None, "invalid_challenge"),
        ("cancelAgentRun", None, "invalid_challenge"),
        (
            "createAgentRun",
            Some("conversation_busy"),
            "invalid_challenge",
        ),
        ("steerAgentRun", None, "invalid_challenge"),
        ("acceptProposal", None, "stale_writer"),
        ("acceptProposal", None, "stale_writer"),
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
