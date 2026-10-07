use storyos_application::{
    ClaimedReadableExport, CompleteReadableExport, ExportHumanReadableManuscriptAdmission,
    ExportHumanReadableManuscriptInput, ProjectCommandError, ReadableExportOperation,
    ReadableExportWorkStore, RefusableCommandError,
};
use storyos_core::ExportHumanReadableManuscriptRefusal;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::update_volume_tests::seed_project;

use super::support::{
    CommandCall, Route, SequenceError, issued, run_without_foreign_keys, stores,
    with_new_request_ids,
};

const EXPORT_HUMAN_READABLE_MANUSCRIPT: Route = Route {
    kind: "exportHumanReadableManuscript",
    method: storyos_contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_METHOD,
    path: storyos_contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_PATH,
    schema: storyos_contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST_SCHEMA_ID,
};

async fn export(
    store: &PostgresProjectReader,
    call: &CommandCall<ExportHumanReadableManuscriptInput>,
) -> Result<ExportHumanReadableManuscriptAdmission, ProjectCommandError> {
    store
        .export_human_readable_manuscript(&call.envelope, &call.input)
        .await
        .map_err(SequenceError::sequence)
}

/// One readable export of a new Project.
async fn export_call(
    store: &PostgresProjectReader,
    base: u16,
) -> CommandCall<ExportHumanReadableManuscriptInput> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    issued(
        store,
        &scope,
        base + 9,
        &EXPORT_HUMAN_READABLE_MANUSCRIPT,
        ExportHumanReadableManuscriptInput {
            export_id: Uuid::now_v7().to_string(),
        },
    )
    .await
}

/// An exact retry with new request and export identities, as the Server sends it.
fn retry(
    call: &CommandCall<ExportHumanReadableManuscriptInput>,
) -> CommandCall<ExportHumanReadableManuscriptInput> {
    let mut retry = with_new_request_ids(call);
    retry.input.export_id = Uuid::now_v7().to_string();
    retry
}

/// Settles the admitted export through the Worker store. The settlement clears its wakeup.
async fn settle(
    store: &PostgresProjectReader,
    call: &CommandCall<ExportHumanReadableManuscriptInput>,
    admitted: &ExportHumanReadableManuscriptAdmission,
) -> CompleteReadableExport {
    store
        .complete_readable_export(&ClaimedReadableExport {
            project_scope: call.envelope.project_scope.clone(),
            export_id: admitted.work.export_id.clone(),
            fence_token: 0,
            source_snapshot_id: admitted.work.source_snapshot.snapshot_id.clone(),
            author_command_admission_id: admitted.author_command_admission_id.clone(),
            command_id: admitted.command_id.clone(),
            idempotency_key: call.envelope.challenge_binding.idempotency_key.clone(),
        })
        .await
        .unwrap()
}

/// Counts the Admission, operation, Pinned Export Source, Receipt, and Project Activity rows of
/// one call, and reads its fence state and its unused Challenge count.
async fn admitted_rows(
    admin: &Client,
    call: &CommandCall<ExportHumanReadableManuscriptInput>,
) -> ([i64; 6], Option<String>) {
    let row = admin
        .query_one(
            "SELECT (SELECT count(*) FROM storyos.author_command_admissions
                      WHERE idempotency_key = $1::text::uuid
                        AND action_class = 'explicit_project_command'),
                    (SELECT count(*) FROM storyos.human_readable_manuscript_export_operations
                      WHERE idempotency_key = $1::text::uuid),
                    (SELECT count(*) FROM storyos.pinned_export_sources
                      WHERE export_id = $2::text::uuid),
                    (SELECT count(*) FROM storyos.domain_receipts
                      WHERE idempotency_key = $1::text::uuid),
                    (SELECT count(*) FROM storyos.project_activity_event_payloads
                      WHERE project_id = $3::text::uuid),
                    (SELECT count(*) FROM storyos.project_command_challenges
                      WHERE idempotency_key = $1::text::uuid AND consumed_at IS NULL),
                    (SELECT outcome_kind || ' ' || coalesce(acknowledgement_format, 'none')
                       FROM storyos.command_idempotency WHERE idempotency_key = $1::text::uuid)",
            &[
                &call.envelope.challenge_binding.idempotency_key,
                &call.input.export_id,
                &call.envelope.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .unwrap();
    (
        [0, 1, 2, 3, 4, 5].map(|index| row.get(index)),
        row.get(/*idx*/ 6),
    )
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn the_admit_step_keeps_the_fence_open_and_replays_the_first_admission() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let ready = export_call(&store, /*base*/ 0xd500).await;
    let ([.., activity_before, _], _) = admitted_rows(&admin, &ready).await;
    let first = export(&store, &ready).await.unwrap();
    let open = admitted_rows(&admin, &ready).await;
    let in_progress = export(&store, &retry(&ready)).await.unwrap();
    let ready_settlement = settle(&store, &ready, &first).await;
    let after_ready = export(&store, &retry(&ready)).await.unwrap();

    let refused = export_call(&store, /*base*/ 0xd510).await;
    let refused_first = export(&store, &refused).await.unwrap();
    run_without_foreign_keys(
        &admin,
        &format!(
            "DELETE FROM storyos.pinned_export_sources WHERE export_id = '{}'",
            refused.input.export_id
        ),
    )
    .await;
    let refused_settlement = settle(&store, &refused, &refused_first).await;
    let after_refusal = export(&store, &retry(&refused)).await.unwrap();
    let key = &refused.envelope.challenge_binding.idempotency_key;
    // The Receipt shape check refuses these settlements, so the row is written without it.
    let shape = admin
        .query_one(
            "SELECT pg_get_constraintdef(oid) FROM pg_constraint
              WHERE conname = 'domain_receipts_result_shape'",
            &[],
        )
        .await
        .unwrap()
        .get::<_, String>(/*idx*/ 0);
    admin
        .batch_execute(
            "ALTER TABLE storyos.domain_receipts DROP CONSTRAINT domain_receipts_result_shape",
        )
        .await
        .unwrap();
    let mut unknown = Vec::new();
    for (result_kind, reason) in [
        ("refused", "unknown_reason"),
        ("conflicted", "archived_project"),
        ("refused", "pinned_export_source_unavailable"),
    ] {
        run_without_foreign_keys(
            &admin,
            &format!(
                "UPDATE storyos.domain_receipts
                    SET result_kind = '{result_kind}',
                        result_payload = '{{\"reason\":\"{reason}\"}}'::jsonb
                  WHERE idempotency_key = '{key}'"
            ),
        )
        .await;
        unknown.push(export(&store, &retry(&refused)).await);
    }
    admin
        .batch_execute(&format!(
            "ALTER TABLE storyos.domain_receipts
               ADD CONSTRAINT domain_receipts_result_shape {shape}"
        ))
        .await
        .unwrap();
    let restored = unknown.pop().unwrap().unwrap();

    assert_eq!(
        first,
        ExportHumanReadableManuscriptAdmission {
            command_id: ready.envelope.ids.command_id.clone(),
            author_command_admission_id: ready.envelope.ids.author_command_admission_id.clone(),
            work: ReadableExportOperation {
                export_id: ready.input.export_id.clone(),
                source_snapshot: first.work.source_snapshot.clone(),
            },
            response: first.response.clone(),
        }
    );
    assert_eq!(
        open,
        (
            [1, 1, 1, 0, activity_before, 0],
            Some("in_progress command_response_project.v1".to_owned())
        )
    );
    assert_eq!(in_progress, first);
    assert_eq!(ready_settlement, CompleteReadableExport::SettledReady);
    assert_eq!(after_ready, first);
    assert_eq!(refused_settlement, CompleteReadableExport::SettledFailed);
    assert_eq!(after_refusal, refused_first);
    assert_eq!(restored, refused_first);
    assert!(
        unknown
            .iter()
            .all(|retry| matches!(retry, Err(ProjectCommandError::BindingConflict))),
        "an unknown settlement must give a binding conflict: {unknown:?}"
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_failed_or_refused_admission_writes_no_row_and_keeps_the_challenge_unused() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let failing = export_call(&store, /*base*/ 0xd520).await;
    let ([.., activity_before, _], _) = admitted_rows(&admin, &failing).await;
    // A Pinned Export Source with the export identity fails the last work row insert.
    let scope = &failing.envelope.project_scope;
    let conflicting = format!(
        "INSERT INTO storyos.pinned_export_sources
           (owner_user_id, project_id, export_id, source_snapshot_id, completeness_profile,
            facts, facts_sha256)
         VALUES ('{}', '{}', '{}', '{}', 'human_readable_manuscript', '{{}}', repeat('0', 64))",
        scope.owner_user_id.as_ref(),
        scope.project_id.as_ref(),
        failing.input.export_id,
        Uuid::now_v7(),
    );
    run_without_foreign_keys(&admin, &conflicting).await;
    let failed = store
        .export_human_readable_manuscript(&failing.envelope, &failing.input)
        .await;
    run_without_foreign_keys(
        &admin,
        &format!(
            "DELETE FROM storyos.pinned_export_sources WHERE export_id = '{}'",
            failing.input.export_id
        ),
    )
    .await;
    let failed_rows = admitted_rows(&admin, &failing).await;
    let after_failure = export(&store, &failing).await.unwrap();
    settle(&store, &failing, &after_failure).await;

    let archived = export_call(&store, /*base*/ 0xd530).await;
    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE project_id = $1::text::uuid",
            &[&archived.envelope.project_scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let refused = store
        .export_human_readable_manuscript(&archived.envelope, &archived.input)
        .await;
    let ([admissions, operations, sources, receipts, _, unused], fence) =
        admitted_rows(&admin, &archived).await;

    assert!(
        matches!(
            failed,
            Err(RefusableCommandError::Command(
                ProjectCommandError::Unavailable(_)
            ))
        ),
        "a failed work row insert must give a store fault: {failed:?}"
    );
    assert_eq!(
        failed_rows,
        (
            [0, 0, 0, 0, activity_before, 1],
            Some("pending none".to_owned())
        )
    );
    assert_eq!(after_failure.work.export_id, failing.input.export_id);
    assert!(matches!(
        refused,
        Err(RefusableCommandError::RefusedBeforeAdmission(
            ExportHumanReadableManuscriptRefusal::ArchivedProject
        ))
    ));
    assert_eq!(
        ([admissions, operations, sources, receipts, unused], fence),
        ([0, 0, 0, 0, 1], Some("pending none".to_owned()))
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn pre_capture_and_damaged_admission_evidence_give_different_errors() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let mut faults = Vec::new();
    for (base, settled) in [(0xd540_u16, false), (0xd550, true)] {
        let call = export_call(&store, base).await;
        let first = export(&store, &call).await.unwrap();
        if settled {
            settle(&store, &call, &first).await;
        }
        let key = &call.envelope.challenge_binding.idempotency_key;
        let captured = admin
            .query_one(
                "SELECT response_project::text FROM storyos.command_idempotency
                  WHERE idempotency_key = $1::text::uuid",
                &[key],
            )
            .await
            .unwrap()
            .get::<_, String>(/*idx*/ 0);
        let fence = format!(
            "UPDATE storyos.command_idempotency
                SET acknowledgement_format = 'command_response_project.v1',
                    response_project = '{captured}'::jsonb"
        );
        let operation = format!(
            "UPDATE storyos.human_readable_manuscript_export_operations
                SET source_snapshot_id = '{}'",
            first.work.source_snapshot.snapshot_id
        );
        for (damage, restore) in [
            (
                "UPDATE storyos.command_idempotency
                    SET acknowledgement_format = NULL, response_project = NULL"
                    .to_owned(),
                &fence,
            ),
            (
                "UPDATE storyos.command_idempotency
                    SET response_project = '{\"broken\":true}'::jsonb"
                    .to_owned(),
                &fence,
            ),
            (
                format!(
                    "UPDATE storyos.human_readable_manuscript_export_operations
                        SET source_snapshot_id = '{}'",
                    Uuid::now_v7()
                ),
                &operation,
            ),
        ] {
            let at_key = format!(" WHERE idempotency_key = '{key}'");
            run_without_foreign_keys(&admin, &format!("{damage}{at_key}")).await;
            faults.push(match export(&store, &retry(&call)).await {
                Err(ProjectCommandError::HistoricalAcknowledgementUnavailable) => "historical",
                Err(ProjectCommandError::Unavailable(_)) => "damaged",
                other => panic!("unexpected retry result {other:?}"),
            });
            run_without_foreign_keys(&admin, &format!("{restore}{at_key}")).await;
        }
        faults.push(match export(&store, &retry(&call)).await {
            Ok(retried) if retried == first => "restored",
            other => panic!("unexpected retry result {other:?}"),
        });
        if !settled {
            settle(&store, &call, &first).await;
        }
    }

    assert_eq!(
        faults,
        ["historical", "damaged", "damaged", "restored"].repeat(2)
    );
}
