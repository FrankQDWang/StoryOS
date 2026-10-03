use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, ProjectScope, UpdateVolumeInput,
    issue_project_command_challenge,
};
use storyos_core::{
    TransitionOutcome, UpdateVolumeConflict, UpdateVolumeNoEffect, UpdateVolumeRefusal,
};
use tokio_postgres::{Client, NoTls};

use super::{
    Classified, CommandSpec, LockedProject, StructureCommand, StructureWrite,
    settle_structure_command, unavailable,
};
use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::update_volume_tests::{
    UpdateFixture, applied, apply_volume, seed_project, update_command, update_issue, update_volume,
};

const BYTES: &[u8] = br#"{"expected_tree_revision":"3","order":"2","title":"Volume B"}"#;
const DIGEST: &str = "sha256:storyos.command.updateVolume.jcs.v1:contract";
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

/// A Project with Volumes A and B at Manuscript Tree Revision 3; returns the Volume A identity.
async fn two_volumes(store: &PostgresProjectReader, suffix: u16) -> (ProjectScope, String) {
    let scope = seed_project(store, &format!("{suffix:04x}")).await;
    let volume_a = br#"{"expected_tree_revision":"1","title":"Volume A"}"#;
    let volume_b = br#"{"expected_tree_revision":"2","title":"Volume B"}"#;
    let digest = |bytes: &[u8]| {
        format!(
            "sha256:storyos.command.createVolume.jcs.v1:{}",
            crate::author_edit::sha256_hex(bytes)
        )
    };
    let volume_a_id = apply_volume(
        store,
        &scope,
        &format!("{:04x}", suffix + 1),
        "Volume A",
        volume_a,
        &digest(volume_a),
        /*expected_tree_revision*/ 1,
    )
    .await;
    apply_volume(
        store,
        &scope,
        &format!("{:04x}", suffix + 2),
        "Volume B",
        volume_b,
        &digest(volume_b),
        /*expected_tree_revision*/ 2,
    )
    .await;
    (scope, volume_a_id)
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

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn every_outcome_replays_its_first_settlement_and_writes_authority_only_when_applied() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, volume_a) = two_volumes(&store, 0x5c00).await;
    let cases = [
        ("5c03", &volume_a[..], 3),
        ("5c05", &volume_a[..], 4),
        ("5c07", &volume_a[..], 3),
        ("5c09", MISSING_VOLUME, 4),
    ];
    let mut outcomes = Vec::new();
    for (suffix, volume_id, expected_tree_revision) in cases {
        let issue = update_issue(&scope, suffix, DIGEST);
        issue_project_command_challenge(&store, &issue)
            .await
            .unwrap();
        let fixture = || UpdateFixture {
            volume_id,
            title: "Volume B",
            order: 2,
            expected_tree_revision,
            bytes: BYTES,
        };
        let first = update_volume(
            &store,
            &update_command(
                issue.binding.clone(),
                &issue.nonce_digest,
                suffix,
                fixture(),
            ),
        )
        .await
        .unwrap();
        let replay_suffix = format!("{:04x}", u16::from_str_radix(suffix, 16).unwrap() + 1);
        let replay = update_volume(
            &store,
            &update_command(
                issue.binding,
                &issue.nonce_digest,
                &replay_suffix,
                fixture(),
            ),
        )
        .await
        .unwrap();
        assert_eq!(replay, first);
        let rows = settlement_rows(&admin, &first.ids.receipt_id).await;
        outcomes.push((first.outcome.map_applied(|_| ()), rows));
    }
    assert_eq!(
        outcomes,
        vec![
            (TransitionOutcome::Applied(()), [1, 1, 1, 1, 1]),
            (
                TransitionOutcome::NoEffect(UpdateVolumeNoEffect::Unchanged),
                [1, 0, 0, 0, 0]
            ),
            (
                TransitionOutcome::Conflicted(UpdateVolumeConflict::StaleTreeRevision),
                [1, 0, 0, 0, 0]
            ),
            (
                TransitionOutcome::Refused(UpdateVolumeRefusal::InvalidVolumeJoin),
                [1, 0, 0, 0, 0]
            ),
        ]
    );
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn an_in_progress_exact_retry_conflicts_and_writes_no_row() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let (scope, volume_a) = two_volumes(&store, 0x5c10).await;
    let issue = update_issue(&scope, "5c13", DIGEST);
    issue_project_command_challenge(&store, &issue)
        .await
        .unwrap();
    admin
        .batch_execute(&format!(
            "UPDATE storyos.project_command_challenges SET consumed_at = clock_timestamp()
              WHERE idempotency_key = '{key}';
             UPDATE storyos.command_idempotency SET outcome_kind = 'in_progress'
              WHERE idempotency_key = '{key}';",
            key = issue.binding.idempotency_key,
        ))
        .await
        .unwrap();
    let call = update_command(
        issue.binding,
        &issue.nonce_digest,
        "5c13",
        UpdateFixture {
            volume_id: &volume_a,
            title: "Volume B",
            order: 2,
            expected_tree_revision: 3,
            bytes: BYTES,
        },
    );
    assert!(matches!(
        update_volume(&store, &call).await,
        Err(ProjectCommandError::BindingConflict)
    ));
    assert_eq!(
        settlement_rows(&admin, &call.envelope.ids.receipt_id).await,
        [0; 5]
    );
}

#[derive(Clone, Copy, Debug)]
enum FailurePoint {
    Classify,
    Apply,
}

/// Update Volume that fails after one step has written its rows.
struct Failing {
    input: UpdateVolumeInput,
    at: FailurePoint,
}

impl StructureCommand for Failing {
    const SPEC: CommandSpec = <UpdateVolumeInput as StructureCommand>::SPEC;
    type Applied = <UpdateVolumeInput as StructureCommand>::Applied;
    type Plan = <UpdateVolumeInput as StructureCommand>::Plan;
    type Effect = <UpdateVolumeInput as StructureCommand>::Effect;
    type NoEffect = <UpdateVolumeInput as StructureCommand>::NoEffect;
    type Conflict = <UpdateVolumeInput as StructureCommand>::Conflict;
    type Refusal = <UpdateVolumeInput as StructureCommand>::Refusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<(Classified<Self>, Self::Plan), ProjectCommandError> {
        let classified = self.input.classify(client, envelope, project).await?;
        match self.at {
            FailurePoint::Classify => Err(unavailable("injected classify failure")),
            FailurePoint::Apply => Ok(classified),
        }
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> Result<StructureWrite<Self::Effect>, ProjectCommandError> {
        self.input.apply(client, envelope, plan, applied).await?;
        Err(unavailable("injected apply failure"))
    }

    fn decode(replay: &CommandReplay) -> Result<Self::Effect, ReplayFault> {
        UpdateVolumeInput::decode(replay)
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_failing_step_rolls_back_every_row_and_keeps_the_challenge_unused() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    for (base, at) in [
        (0x5c20, FailurePoint::Classify),
        (0x5c30, FailurePoint::Apply),
    ] {
        let (scope, volume_a) = two_volumes(&store, base).await;
        let suffix = format!("{:04x}", base + 3);
        let issue = update_issue(&scope, &suffix, DIGEST);
        issue_project_command_challenge(&store, &issue)
            .await
            .unwrap();
        let call = update_command(
            issue.binding,
            &issue.nonce_digest,
            &suffix,
            UpdateFixture {
                volume_id: &volume_a,
                title: "Volume A Renamed",
                order: 2,
                expected_tree_revision: 3,
                bytes: BYTES,
            },
        );
        let failing = Failing {
            input: call.input.clone(),
            at,
        };
        let failed = settle_structure_command(&store, &call.envelope, &failing).await;
        assert!(
            matches!(failed, Err(ProjectCommandError::Unavailable(_))),
            "{at:?}"
        );
        assert_eq!(
            settlement_rows(&admin, &call.envelope.ids.receipt_id).await,
            [0; 5]
        );
        let (effect, authority) = applied(&update_volume(&store, &call).await.unwrap());
        assert_eq!(
            (
                effect.title,
                effect.order,
                authority.prior_manuscript_tree_revision
            ),
            ("Volume A Renamed".to_owned(), 2, 3)
        );
    }
}
