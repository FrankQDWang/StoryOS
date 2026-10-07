use std::fmt::Debug;
use std::future::Future;

use storyos_application::{
    AdmittedProjectCommand, ArchiveExportOperation, ArchiveExportRefusal, ArchiveExportWorkStore,
    ClaimedArchiveExport, ClaimedReadableExport, CompleteArchiveExport, CompleteReadableExport,
    ExportHumanReadableManuscriptInput, ExportProjectArchiveInput,
    PROJECT_EXPORT_ARCHIVE_PATH_PROFILE, PROJECT_EXPORT_ARCHIVE_PROFILE, ProjectCommandEnvelope,
    ProjectCommandError, ReadableExportOperation, ReadableExportWorkStore, RefusableCommandError,
};
use storyos_core::{
    ExportHumanReadableManuscriptRefusal, ExportProjectArchiveRefusal, ProjectArchiveBuildRefusal,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::update_volume_tests::seed_project;

use super::support::{
    CommandCall, Route, SequenceError, issued, run_without_foreign_keys, stores,
    with_new_request_ids,
};

/// The result of one Worker settlement of an admitted export.
#[derive(Debug, PartialEq)]
enum Settled {
    Ready,
    Failed,
}

type Admitted<E> = AdmittedProjectCommand<<E as ExportInput>::Work>;
type Admit<E> = Result<Admitted<E>, RefusableCommandError<<E as ExportInput>::Refusal>>;

/// The input of one export of the admit step, with the export facts that the contract rows use.
trait ExportInput: Clone + Sized {
    type Work: Clone + Debug + PartialEq;
    type Refusal: Debug;
    const ROUTE: Route;
    /// The table of the admitted operation row.
    const OPERATIONS: &'static str;

    fn new(export_id: String) -> Self;
    fn export_id(&self) -> &str;
    /// The admitted work of a first use that pins the Snapshot of `admitted`.
    fn work(&self, admitted: &Self::Work) -> Self::Work;
    fn snapshot_id(work: &Self::Work) -> &str;
    fn is_archived(refusal: &Self::Refusal) -> bool;
    fn admit(
        &self,
        store: &PostgresProjectReader,
        envelope: &ProjectCommandEnvelope,
    ) -> impl Future<Output = Admit<Self>>;
    /// Settles the admitted export through the Worker store. The settlement clears its wakeup.
    fn settle(
        store: &PostgresProjectReader,
        envelope: &ProjectCommandEnvelope,
        admitted: &Admitted<Self>,
    ) -> impl Future<Output = Settled>;
}

/// The Worker claim of one admitted export. Both exports have the same claim fields.
macro_rules! claim {
    ($claim:ident, $envelope:expr, $admitted:expr) => {
        $claim {
            project_scope: $envelope.project_scope.clone(),
            export_id: $admitted.work.export_id.clone(),
            fence_token: 0,
            source_snapshot_id: $admitted.work.source_snapshot.snapshot_id.clone(),
            author_command_admission_id: $admitted.author_command_admission_id.clone(),
            command_id: $admitted.command_id.clone(),
            idempotency_key: $envelope.challenge_binding.idempotency_key.clone(),
        }
    };
}

impl ExportInput for ExportHumanReadableManuscriptInput {
    type Work = ReadableExportOperation;
    type Refusal = ExportHumanReadableManuscriptRefusal;
    const ROUTE: Route = Route {
        kind: "exportHumanReadableManuscript",
        method: storyos_contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_METHOD,
        path: storyos_contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_PATH,
        schema: storyos_contracts::EXPORT_HUMAN_READABLE_MANUSCRIPT_REQUEST_SCHEMA_ID,
    };
    const OPERATIONS: &'static str = "human_readable_manuscript_export_operations";

    fn new(export_id: String) -> Self {
        Self { export_id }
    }

    fn export_id(&self) -> &str {
        &self.export_id
    }

    fn work(&self, admitted: &ReadableExportOperation) -> ReadableExportOperation {
        ReadableExportOperation {
            export_id: self.export_id.clone(),
            source_snapshot: admitted.source_snapshot.clone(),
        }
    }

    fn snapshot_id(work: &ReadableExportOperation) -> &str {
        &work.source_snapshot.snapshot_id
    }

    fn is_archived(refusal: &ExportHumanReadableManuscriptRefusal) -> bool {
        matches!(
            refusal,
            ExportHumanReadableManuscriptRefusal::ArchivedProject
        )
    }

    async fn admit(
        &self,
        store: &PostgresProjectReader,
        envelope: &ProjectCommandEnvelope,
    ) -> Admit<Self> {
        store.export_human_readable_manuscript(envelope, self).await
    }

    async fn settle(
        store: &PostgresProjectReader,
        envelope: &ProjectCommandEnvelope,
        admitted: &Admitted<Self>,
    ) -> Settled {
        let claim = claim!(ClaimedReadableExport, envelope, admitted);
        match store.complete_readable_export(&claim).await.unwrap() {
            CompleteReadableExport::SettledReady => Settled::Ready,
            CompleteReadableExport::SettledFailed => Settled::Failed,
            CompleteReadableExport::AlreadySettled => panic!("the export was already settled"),
        }
    }
}

impl ExportInput for ExportProjectArchiveInput {
    type Work = ArchiveExportOperation;
    type Refusal = ArchiveExportRefusal;
    const ROUTE: Route = Route {
        kind: "exportProjectArchive",
        method: storyos_contracts::EXPORT_PROJECT_ARCHIVE_METHOD,
        path: storyos_contracts::EXPORT_PROJECT_ARCHIVE_PATH,
        schema: storyos_contracts::EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID,
    };
    const OPERATIONS: &'static str = "project_export_operations";

    fn new(export_id: String) -> Self {
        Self { export_id }
    }

    fn export_id(&self) -> &str {
        &self.export_id
    }

    fn work(&self, admitted: &ArchiveExportOperation) -> ArchiveExportOperation {
        ArchiveExportOperation {
            export_id: self.export_id.clone(),
            archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
            archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
            source_snapshot: admitted.source_snapshot.clone(),
        }
    }

    fn snapshot_id(work: &ArchiveExportOperation) -> &str {
        &work.source_snapshot.snapshot_id
    }

    fn is_archived(refusal: &ArchiveExportRefusal) -> bool {
        matches!(
            refusal,
            ArchiveExportRefusal::Lifecycle(ExportProjectArchiveRefusal::ArchivedProject)
        )
    }

    async fn admit(
        &self,
        store: &PostgresProjectReader,
        envelope: &ProjectCommandEnvelope,
    ) -> Admit<Self> {
        store.export_project_archive(envelope, self).await
    }

    async fn settle(
        store: &PostgresProjectReader,
        envelope: &ProjectCommandEnvelope,
        admitted: &Admitted<Self>,
    ) -> Settled {
        let claim = claim!(ClaimedArchiveExport, envelope, admitted);
        match store.complete_archive_export(&claim).await.unwrap() {
            CompleteArchiveExport::SettledReady => Settled::Ready,
            CompleteArchiveExport::SettledFailed => Settled::Failed,
            CompleteArchiveExport::AlreadySettled => panic!("the export was already settled"),
        }
    }
}

async fn export<E: ExportInput>(
    store: &PostgresProjectReader,
    call: &CommandCall<E>,
) -> Result<Admitted<E>, ProjectCommandError> {
    call.input
        .admit(store, &call.envelope)
        .await
        .map_err(SequenceError::sequence)
}

/// One export of a new Project.
async fn export_call<E: ExportInput>(store: &PostgresProjectReader, base: u16) -> CommandCall<E> {
    let scope = seed_project(store, &format!("{base:04x}")).await;
    issued(
        store,
        &scope,
        base + 9,
        &E::ROUTE,
        E::new(Uuid::now_v7().to_string()),
    )
    .await
}

/// An exact retry with new request and export identities, as the Server sends it.
fn retry<E: ExportInput>(call: &CommandCall<E>) -> CommandCall<E> {
    let mut retry = with_new_request_ids(call);
    retry.input = E::new(Uuid::now_v7().to_string());
    retry
}

async fn settle<E: ExportInput>(
    store: &PostgresProjectReader,
    call: &CommandCall<E>,
    admitted: &Admitted<E>,
) -> Settled {
    E::settle(store, &call.envelope, admitted).await
}

/// Counts the Admission, operation, Pinned Export Source, Receipt, and Project Activity rows of
/// one call, and reads its fence state and its unused Challenge count.
async fn admitted_rows<E: ExportInput>(
    admin: &Client,
    call: &CommandCall<E>,
) -> ([i64; 6], Option<String>) {
    let row = admin
        .query_one(
            &format!(
                "SELECT (SELECT count(*) FROM storyos.author_command_admissions
                      WHERE idempotency_key = $1::text::uuid
                        AND action_class = 'explicit_project_command'),
                    (SELECT count(*) FROM storyos.{}
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
                E::OPERATIONS
            ),
            &[
                &call.envelope.challenge_binding.idempotency_key,
                &call.input.export_id(),
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
    keeps_the_fence_open::<ExportHumanReadableManuscriptInput>(/*base*/ 0xe600).await;
    keeps_the_fence_open::<ExportProjectArchiveInput>(/*base*/ 0xc600).await;
}

async fn keeps_the_fence_open<E: ExportInput>(base: u16) {
    let (store, admin) = stores().await;
    let ready = export_call::<E>(&store, base).await;
    let ([.., activity_before, _], _) = admitted_rows(&admin, &ready).await;
    let first = export(&store, &ready).await.unwrap();
    let open = admitted_rows(&admin, &ready).await;
    let in_progress = export(&store, &retry(&ready)).await.unwrap();
    let ready_settlement = settle(&store, &ready, &first).await;
    let after_ready = export(&store, &retry(&ready)).await.unwrap();

    let refused = export_call::<E>(&store, base + 0x10).await;
    let refused_first = export(&store, &refused).await.unwrap();
    run_without_foreign_keys(
        &admin,
        &format!(
            "DELETE FROM storyos.pinned_export_sources WHERE export_id = '{}'",
            refused.input.export_id()
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
        AdmittedProjectCommand {
            command_id: ready.envelope.ids.command_id.clone(),
            author_command_admission_id: ready.envelope.ids.author_command_admission_id.clone(),
            work: ready.input.work(&first.work),
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
    assert_eq!(ready_settlement, Settled::Ready);
    assert_eq!(after_ready, first);
    assert_eq!(refused_settlement, Settled::Failed);
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
    writes_no_row::<ExportHumanReadableManuscriptInput>(/*base*/ 0xe620).await;
    writes_no_row::<ExportProjectArchiveInput>(/*base*/ 0xc620).await;
}

async fn writes_no_row<E: ExportInput>(base: u16) {
    let (store, admin) = stores().await;
    let failing = export_call::<E>(&store, base).await;
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
        failing.input.export_id(),
        Uuid::now_v7(),
    );
    run_without_foreign_keys(&admin, &conflicting).await;
    let failed = failing.input.admit(&store, &failing.envelope).await;
    run_without_foreign_keys(
        &admin,
        &format!(
            "DELETE FROM storyos.pinned_export_sources WHERE export_id = '{}'",
            failing.input.export_id()
        ),
    )
    .await;
    let failed_rows = admitted_rows(&admin, &failing).await;
    let after_failure = export(&store, &failing).await.unwrap();
    settle(&store, &failing, &after_failure).await;

    let archived = export_call::<E>(&store, base + 0x10).await;
    admin
        .execute(
            "UPDATE storyos.projects SET lifecycle_state = 'archived'
              WHERE project_id = $1::text::uuid",
            &[&archived.envelope.project_scope.project_id.as_ref()],
        )
        .await
        .unwrap();
    let refused = archived.input.admit(&store, &archived.envelope).await;
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
    assert_eq!(after_failure.work, failing.input.work(&after_failure.work));
    assert!(
        matches!(&refused, Err(RefusableCommandError::RefusedBeforeAdmission(refusal))
            if E::is_archived(refusal)),
        "an archived Project must refuse before the Admission: {refused:?}"
    );
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
    damaged_evidence::<ExportHumanReadableManuscriptInput>(/*first_base*/ 0xe640).await;
    damaged_evidence::<ExportProjectArchiveInput>(/*first_base*/ 0xc640).await;
}

async fn damaged_evidence<E: ExportInput>(first_base: u16) {
    let (store, admin) = stores().await;
    let mut faults = Vec::new();
    for (base, settled) in [(first_base, false), (first_base + 0x10, true)] {
        let call = export_call::<E>(&store, base).await;
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
            "UPDATE storyos.{}
                SET source_snapshot_id = '{}'",
            E::OPERATIONS,
            E::snapshot_id(&first.work)
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
                    "UPDATE storyos.{}
                        SET source_snapshot_id = '{}'",
                    E::OPERATIONS,
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_archive_build_refusal_writes_no_row_and_the_archive_pins_its_own_admission() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let call = export_call::<ExportProjectArchiveInput>(&store, /*base*/ 0xc660).await;
    let scope = &call.envelope.project_scope;
    // A tombstoned Draft without its revision has no valid provenance.
    let draft = format!(
        "INSERT INTO storyos.draft_artifacts
           (owner_user_id, project_id, draft_id, current_revision_id, retention_state)
         VALUES ('{}', '{}', '{}', '{}', 'tombstoned')",
        scope.owner_user_id.as_ref(),
        scope.project_id.as_ref(),
        Uuid::now_v7(),
        Uuid::now_v7(),
    );
    run_without_foreign_keys(&admin, &draft).await;
    let refused = call.input.admit(&store, &call.envelope).await;
    let ([admissions, operations, sources, receipts, _, unused], fence) =
        admitted_rows(&admin, &call).await;
    run_without_foreign_keys(
        &admin,
        &format!(
            "DELETE FROM storyos.draft_artifacts WHERE project_id = '{}'",
            scope.project_id.as_ref()
        ),
    )
    .await;
    let admitted = export(&store, &call).await.unwrap();
    let pinned: i64 = admin
        .query_one(
            "SELECT count(*)
               FROM storyos.pinned_export_sources AS source,
                    jsonb_array_elements(source.facts->'families') AS family,
                    jsonb_array_elements(family->'rows') AS family_row
              WHERE source.export_id = $1::text::uuid
                AND ((family->>'table' = 'author_command_admissions'
                      AND family_row->>'author_command_admission_id' = $2)
                  OR (family->>'table' = 'project_export_operations'
                      AND family_row->>'export_id' = $1))",
            &[
                &admitted.work.export_id,
                &admitted.author_command_admission_id,
            ],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    settle(&store, &call, &admitted).await;

    assert!(
        matches!(
            refused,
            Err(RefusableCommandError::RefusedBeforeAdmission(
                ArchiveExportRefusal::ArchiveBuild(ProjectArchiveBuildRefusal::InvalidProvenance)
            ))
        ),
        "a family without valid provenance must refuse before the Admission: {refused:?}"
    );
    assert_eq!(
        ([admissions, operations, sources, receipts, unused], fence),
        ([0, 0, 0, 0, 1], Some("pending none".to_owned()))
    );
    assert_eq!(pinned, 2);
}
