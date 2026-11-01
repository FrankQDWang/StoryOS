use storyos_application::{
    ActivityApplied, ProjectCommandEnvelope, ProjectCommandError, UpdateProjectInput,
    UpdateProjectSettlement,
};
use storyos_contracts::CommandKind;
use storyos_core::{ReceiptResult, UpdateProjectApplied};
use tokio_postgres::Client;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, AppliedVariant, Classification, CommandResponse, CommandSpec, CommandWrite,
    Effect, LockedProject, ProfileSequences, ProjectCommand, ReplayEffect, ZeroAuthority,
    ZeroAuthorityRows, ZeroOutcome, settle_project_command, unavailable,
};

use super::project_session::{UPDATE_PROJECT, update_project_call};
use super::rollback::failed_then_settled;
use super::support::{CommandCall, issued, replayed_outcome, stores};

/// An Update Project on the declared surface of ADR 0080. It classifies, writes, and decodes as
/// Update Project, and adds the parts that only the declared surface has: a constant Receipt
/// payload, a check after the profile records, and a zero-authority effect of a no-effect outcome.
#[derive(Clone)]
struct DeclaredRename(UpdateProjectInput);

const DECLARED_PAYLOAD: &str = r#"{"surface":"declared"}"#;

impl ProjectCommand for DeclaredRename {
    const SPEC: CommandSpec = CommandSpec {
        kind: CommandKind::UpdateProject,
        applied: &[AppliedVariant::NoAuthorAction],
        activity_kind: "project_updated",
        replay_effect: ReplayEffect::NoQuery,
        response: CommandResponse::Project,
        contention_refusal: None,
        receipt_payload: DECLARED_PAYLOAD,
    };
    type Error = ProjectCommandError;
    type Profile = ActivityOnly;
    type Plan = ();
    type Applied = UpdateProjectApplied;
    type Settlement = UpdateProjectSettlement;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let classified = ProjectCommand::classify(&self.0, client, envelope, project).await?;
        Ok(Classification {
            outcome: classified.outcome,
            admission: classified.admission,
            heads: classified.heads,
            zero_receipt: classified.zero_receipt,
        })
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        sequences: &ProfileSequences<Self>,
        plan: (),
        applied: UpdateProjectApplied,
    ) -> Result<CommandWrite<'_, Self>, ProjectCommandError> {
        let CommandWrite { profile, .. } =
            ProjectCommand::apply(&self.0, client, envelope, project, sequences, plan, applied)
                .await?;
        Ok(CommandWrite::after_authority(
            profile,
            |_rename: &Self, client, envelope, applied| {
                Box::pin(require_activity(client, envelope, applied))
            },
        ))
    }

    fn decode(&self, replay: &CommandReplay) -> Result<Effect<Self>, ReplayFault> {
        ProjectCommand::decode(&self.0, replay)
    }

    fn zero_authority(&self) -> Option<&(impl ZeroAuthority<Self> + Sync)> {
        Some(self)
    }
}

impl ZeroAuthority<Self> for DeclaredRename {
    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        match outcome {
            ZeroOutcome::NoEffect(_) => ZeroAuthorityRows::Effect,
            ZeroOutcome::Conflicted(_) | ZeroOutcome::Refused(_) => ZeroAuthorityRows::None,
        }
    }

    async fn write_zero_authority_effect(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _outcome: &ZeroOutcome<'_, Self>,
    ) -> Result<(), ProjectCommandError> {
        Ok(())
    }

    fn decode_zero_authority_effect(
        &self,
        _replay: &CommandReplay,
    ) -> Result<Option<()>, ReplayFault> {
        Ok(Some(()))
    }
}

/// Refuses the settlement unless the profile already wrote the Activity record of `applied`.
async fn require_activity(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    applied: &ActivityApplied<UpdateProjectApplied>,
) -> Result<(), ProjectCommandError> {
    let recorded: i64 = client
        .query_one(
            "SELECT count(*) FROM storyos.project_activity_event_payloads
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND project_activity_event_id = $3::text::uuid",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
                &applied.project_activity_event_id,
            ],
        )
        .await
        .map_err(unavailable)?
        .get(/*idx*/ 0);
    if recorded == 1 {
        Ok(())
    } else {
        Err(unavailable(
            "the after-authority write ran before the profile records",
        ))
    }
}

async fn settle_declared(
    store: &PostgresProjectReader,
    call: &CommandCall<DeclaredRename>,
) -> Result<UpdateProjectSettlement, ProjectCommandError> {
    settle_project_command(store, &call.envelope, &call.input).await
}

async fn declared_call(store: &PostgresProjectReader, base: u16) -> CommandCall<DeclaredRename> {
    let call = update_project_call(store, base).await;
    CommandCall {
        envelope: call.envelope,
        input: DeclaredRename(call.input),
    }
}

#[tokio::test]
#[ignore = "run through scripts/verify-project-scope.sh"]
async fn a_command_on_the_declared_surface_settles_replays_and_rolls_back_through_the_sequence() {
    let _test_guard = crate::author_edit::tests::AUTHOR_EDIT_TEST_LOCK
        .lock()
        .await;
    let (store, admin) = stores().await;
    let renamed = declared_call(&store, /*base*/ 0x2e00).await;
    let applied = replayed_outcome(&store, &admin, &renamed, settle_declared).await;
    let payload: String = admin
        .query_one(
            "SELECT result_payload::text FROM storyos.domain_receipts
              WHERE receipt_id = $1::text::uuid",
            &[&renamed.envelope.ids.receipt_id],
        )
        .await
        .unwrap()
        .get(/*idx*/ 0);
    let unchanged = issued(
        &store,
        &renamed.envelope.project_scope,
        /*suffix*/ 0x2e0a,
        &UPDATE_PROJECT,
        DeclaredRename(UpdateProjectInput {
            title: renamed.input.0.title.clone(),
            expected_revision: 2,
        }),
    )
    .await;
    let no_effect = replayed_outcome(&store, &admin, &unchanged, settle_declared).await;
    let zero_authority_effect = settle_declared(&store, &unchanged)
        .await
        .unwrap()
        .zero_authority_effect;
    let rolled_back = failed_then_settled(
        &store,
        &admin,
        &declared_call(&store, /*base*/ 0x2e10).await,
    )
    .await;

    assert_eq!(
        (
            applied,
            payload,
            no_effect,
            zero_authority_effect,
            rolled_back
        ),
        (
            ("authoritative_applied".to_owned(), [1, 0, 1, 0, 0]),
            DECLARED_PAYLOAD.replace(':', ": "),
            ("no_effect".to_owned(), [1, 0, 0, 0, 0]),
            Some(()),
            (
                vec![
                    (Some("classify"), [0; 5]),
                    (Some("apply"), [0; 5]),
                    (Some("after authority"), [0; 5]),
                ],
                ReceiptResult::AuthoritativeApplied,
            ),
        ),
    );
}
