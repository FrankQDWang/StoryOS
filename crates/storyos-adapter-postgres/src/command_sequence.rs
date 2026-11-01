//! The fixed Author Command Admission, Core Transition, and Command Acknowledgement sequence
//! of the project commands (ADR 0041, ADR 0076).

use storyos_application::{
    DiagnosticField, DiagnosticId, ProjectCommandEnvelope, ProjectCommandError,
    ProjectCommandSettlement,
};
use storyos_core::TransitionOutcome;
use tokio_postgres::Client;

use crate::PostgresProjectReader;

mod action_only;
mod activity_only;
mod admission;
mod admit;
mod applied_variant;
mod authoritative_revision;
mod chapter_selection;
mod contention;
mod profile;
mod records;
mod response;
mod settle;
mod settlement_shape;
mod skeleton;
mod structural;
mod structural_records;
mod surface;
mod zero_authority;
use crate::command_replay::{CommandReplay, EffectKey, ReplayFault, read_command_replay};
pub(crate) use action_only::{ActionOnly, ActionSequence};
pub(crate) use activity_only::{ActivityOnly, ActivitySequences, ActivityWrite};
use admission::insert_admission;
pub(crate) use admit::{AdmitCommand, AdmitSpec, admit_project_command};
pub(crate) use applied_variant::AppliedVariant;
pub(crate) use authoritative_revision::{
    ActionDisposition, AuthoritativeRevision, RevisionBase, RevisionMembers, RevisionSequences,
    RevisionWrite, write_revision,
};
pub(crate) use chapter_selection::{ChapterSelection, ChapterSelectionWrite};
pub(crate) use contention::CommandError;
pub(crate) use profile::{
    PersistFacts, PersistInput, SelectsRecords, SettlementProfile, SingleRecordSet,
};
pub(crate) use records::{CommitCounters, allocate_commit_counters, insert_author_action};
use records::{ReceiptRecord, insert_activity_payload, insert_receipt, lock_project};
pub(crate) use response::CommandResponse;
pub(crate) use settle::{
    AdmittedCommand, AdmittedRefusal, admit_and_settle, replay_settled as replay_settled_command,
    settle_admitted_command,
};
pub(crate) use settlement_shape::{AppliedRecord, SettlementShape};
use skeleton::{
    ChallengeUse, FirstUse, consume_challenge, record_ids, record_result, replay_problem,
};
pub(crate) use structural::{
    CurrentChapterChange, Structural, StructureIdentity, StructureWrite, WriterBase,
};
pub(crate) use surface::{
    AppliedReceipt, Classification, Classified, CommandSpec, CommandWrite, Conflict, NoEffect,
    ProfileSequences, ProjectCommand, Refusal, SettledCommand, ZeroEffect,
};
pub(crate) use zero_authority::{
    NoZeroAuthority, ZeroAuthority, ZeroAuthorityRows, ZeroAuthorityWrite, ZeroOutcome, ZeroReceipt,
};

/// The action class and Editor Session binding that the Author Command Admission records.
pub(crate) enum Admission {
    /// A command without an Editor Session.
    Project(ProjectActionClass),
    /// A command of the current writer Editor Session. The insert requires its writer generation.
    ExplicitEditorCommand(EditorAdmission),
    /// A writer takeover. The insert requires the observed writer generation of another session.
    WriterTakeover(TakeoverAdmission),
    /// A direct editor action of the current writer, for example an Author Edit. The insert
    /// requires the client writer generation, the expected Revision and head of the Chapter, and an
    /// active Project.
    DirectEditorAction(DirectEditorAdmission),
}

/// The Author Edit columns of a direct editor action Admission.
pub(crate) struct DirectEditorAdmission {
    pub(crate) editor_session_id: String,
    pub(crate) writer_generation: u64,
    pub(crate) chapter_object_id: String,
    pub(crate) expected_authoritative_revision_id: String,
    pub(crate) expected_proposal_head_revision_ids: Vec<String>,
    pub(crate) target_refs: Vec<String>,
    pub(crate) observed_ownership_partition: String,
    pub(crate) editor_contract_revision: String,
    pub(crate) undo_group_id: String,
    pub(crate) completed_intent_record_id: String,
    pub(crate) local_intent_sequence: u64,
}

/// The action class of an Admission without an Editor Session. Each class writes the same columns.
pub(crate) enum ProjectActionClass {
    ExplicitProjectCommand,
    AgentRunStart,
    AgentRunControl,
}

pub(crate) struct TakeoverAdmission {
    pub(crate) editor_session_id: String,
    pub(crate) observed_writer_generation: u64,
    pub(crate) editor_contract_revision: String,
}

pub(crate) struct EditorAdmission {
    pub(crate) editor_session_id: String,
    pub(crate) chapter_object_id: Option<String>,
    pub(crate) expected_authoritative_revision_id: Option<String>,
    /// The Manuscript Block references that the command targets, in command order.
    pub(crate) target_refs: Vec<String>,
}

/// The head arrays that the Domain Receipt of every outcome records.
#[derive(Default)]
pub(crate) struct ReceiptHeads {
    pub(crate) expected: Vec<String>,
    pub(crate) prior: Vec<String>,
    pub(crate) resulting: Vec<String>,
}

/// The Proposal Revision and condition references that one Domain Receipt records.
#[derive(Default)]
pub(crate) struct ReceiptRefs {
    pub(crate) proposal_revision_ids: Vec<String>,
    pub(crate) condition_refs: Vec<String>,
}

/// The effect rows of the command that an exact retry reads in the replay transaction.
pub(crate) enum ReplayEffect {
    NoQuery,
    /// One query that takes the owner, Project, and Receipt identities as `$1`, `$2`, and `$3`.
    /// Its optional row holds one JSON object text.
    Query(&'static str),
}

/// The Project row that the sequence locks before a command loads its own facts.
pub(crate) struct LockedProject {
    pub(crate) tree_revision: u64,
    pub(crate) current_chapter_id: Option<String>,
}

#[tracing::instrument(skip_all, fields(
    command_kind = C::SPEC.kind.diagnostic(),
    project_id = envelope.project_scope.project_id.diagnostic(),
    command_id = tracing::field::Empty,
    author_command_admission_id = tracing::field::Empty,
    correlation_id = DiagnosticId(&envelope.correlation_id).diagnostic(),
    outcome = tracing::field::Empty,
))]
pub(crate) async fn settle_project_command<C: ProjectCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledCommand<C>, C::Error> {
    const {
        assert!(
            AppliedVariant::all_name_kind(C::SPEC.applied, C::SPEC.kind),
            "a Forward applied variant names the Forward command of the command kind"
        );
    }
    let settled = async {
        match consume_challenge(store, envelope, &Settle(command)).await? {
            ChallengeUse::Committed(settled) => Ok(settled),
            ChallengeUse::ExactRetrySettled(receipt_id) => {
                replay_receipt(store, envelope, command, &receipt_id).await
            }
            ChallengeUse::ExactRetryInProgress => Err(ProjectCommandError::BindingConflict.into()),
        }
    }
    .await;
    record_result(&settled);
    settled
}

/// The first use of a command that settles in its Challenge transaction.
struct Settle<'c, C>(&'c C);

impl<C: ProjectCommand> FirstUse for Settle<'_, C> {
    type Output = SettledCommand<C>;
    type Error = C::Error;
    const REFUSAL_ON_CONTENTION: Option<fn() -> C::Error> = C::SPEC.contention_refusal;

    async fn write(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
    ) -> Result<SettledCommand<C>, C::Error> {
        record_ids(
            &envelope.ids.command_id,
            &envelope.ids.author_command_admission_id,
        );
        let command = self.0;
        let project = lock_project(client, envelope).await?;
        let Classification {
            outcome,
            admission,
            heads,
            zero_receipt: zero,
        } = command.classify(client, envelope, &project).await?;
        if !insert_admission(client, envelope, C::SPEC.kind, &admission).await? {
            return Err(command.diagnose_missing_admission(client, envelope).await);
        }
        settle_classified(client, envelope, command, &project, outcome, heads, zero).await
    }
}

/// Writes the Domain Receipt, the profile or zero-authority records, and the fence of one
/// classified command whose Admission exists.
async fn settle_classified<C: ProjectCommand>(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command: &C,
    project: &LockedProject,
    classified: Classified<C>,
    heads: ReceiptHeads,
    zero_receipt: ZeroReceipt<ZeroEffect<C>>,
) -> Result<SettledCommand<C>, C::Error> {
    let (mut zero_fields, zero_refs, observed_effect) = match zero_receipt {
        ZeroReceipt::Reason => (serde_json::Map::new(), ReceiptRefs::default(), None),
        ZeroReceipt::Observed {
            fields,
            refs,
            effect,
        } => (fields, refs, Some(effect)),
    };
    let classified = classified.map_applied(|(applied, plan)| {
        let receipt = command.applied_receipt(&applied, &plan);
        (applied, plan, receipt)
    });
    let receipt = ReceiptRecord {
        result: match &classified {
            TransitionOutcome::Applied((_, _, receipt)) => receipt.variant.result_kind(),
            TransitionOutcome::NoEffect(_)
            | TransitionOutcome::Conflicted(_)
            | TransitionOutcome::Refused(_) => classified.receipt_result_kind(),
        },
        // A reason value that records no code keeps only the observed fields.
        payload: {
            if let Some(code) = classified.reason_code() {
                zero_fields.insert("reason".to_owned(), code.into());
            }
            serde_json::Value::Object(zero_fields).to_string()
        },
        command_kind: C::SPEC.kind.as_str(),
        heads,
        refs: zero_refs,
        revision_ids: Vec::new(),
    };
    // A zero-authority outcome records its reason code, and an applied variant its result kind.
    let recorded = classified.reason_code().unwrap_or(receipt.result);
    tracing::Span::current().record("outcome", recorded.diagnostic());
    let (receipt_created_at, outcome) = match classified {
        TransitionOutcome::Applied((
            applied,
            plan,
            AppliedReceipt {
                variant,
                payload,
                refs,
            },
        )) => {
            let sequences = plan
                .allocate_records(client, &envelope.project_scope)
                .await?;
            let revision_ids = C::Profile::revision_ids(&sequences);
            let mut receipt = ReceiptRecord {
                payload,
                refs,
                ..receipt
            };
            if !revision_ids.is_empty() {
                receipt.heads.resulting.clone_from(&revision_ids);
            }
            receipt.revision_ids = revision_ids;
            let receipt_created_at = insert_receipt(
                client,
                envelope,
                &receipt,
                &C::Profile::commit_ids(&sequences),
            )
            .await?;
            let CommandWrite {
                profile,
                after_authority,
            } = command
                .apply(client, envelope, project, &sequences, plan, applied)
                .await?;
            let input = PersistInput::from_facts(&PersistFacts {
                project,
                activity_kind: C::SPEC.activity_kind,
                variant,
            });
            let applied = C::Profile::persist(client, envelope, input, sequences, profile).await?;
            if let Some(after_authority) = after_authority {
                after_authority(command, client, envelope, &applied).await?;
            }
            (receipt_created_at, TransitionOutcome::Applied(applied))
        }
        TransitionOutcome::NoEffect(reason) => (
            insert_receipt(client, envelope, &receipt, &[]).await?,
            TransitionOutcome::NoEffect(reason),
        ),
        TransitionOutcome::Conflicted(reason) => (
            insert_receipt(client, envelope, &receipt, &[]).await?,
            TransitionOutcome::Conflicted(reason),
        ),
        TransitionOutcome::Refused(reason) => (
            insert_receipt(client, envelope, &receipt, &[]).await?,
            TransitionOutcome::Refused(reason),
        ),
    };
    let zero_outcome = match &outcome {
        TransitionOutcome::Applied(_) => None,
        TransitionOutcome::NoEffect(reason) => Some(ZeroOutcome::NoEffect(reason)),
        TransitionOutcome::Conflicted(reason) => Some(ZeroOutcome::Conflicted(reason)),
        TransitionOutcome::Refused(reason) => Some(ZeroOutcome::Refused(reason)),
    };
    let zero_authority_effect = match (zero_outcome, command.zero_authority()) {
        (None, _) => None,
        (Some(_), None) => observed_effect,
        (Some(zero_outcome), Some(zero)) => match zero.zero_authority_rows(&zero_outcome) {
            ZeroAuthorityRows::None => observed_effect,
            ZeroAuthorityRows::Effect => Some(
                zero.write_zero_authority_effect(client, envelope, &zero_outcome)
                    .await?,
            ),
            ZeroAuthorityRows::EffectWithActivity => {
                let activity = ActivityOnly::allocate(client, &envelope.project_scope).await?;
                let write = zero
                    .write_zero_authority_activity(client, envelope, project, &activity)
                    .await?;
                insert_activity_payload(
                    client,
                    envelope,
                    outcome.receipt_result_kind(),
                    C::SPEC.activity_kind,
                    activity.project_activity_position,
                    &activity.project_activity_event_id,
                    write.activity,
                )
                .await?;
                Some(write.effect)
            }
        },
    };
    let response = C::SPEC
        .response
        .settle(client, envelope, C::SPEC.kind.as_str())
        .await?;
    Ok(ProjectCommandSettlement {
        ids: envelope.ids.clone(),
        receipt_created_at,
        outcome,
        response,
        zero_authority_effect,
    })
}

/// Replays the Domain Receipt `receipt_id` of one settled command.
async fn replay_receipt<C: ProjectCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
    receipt_id: &str,
) -> Result<SettledCommand<C>, C::Error> {
    read_command_replay(
        store,
        &envelope.challenge_binding,
        receipt_id,
        &envelope.canonical_command_bytes,
        &C::SPEC.replay_effect,
        EffectKey::Receipt,
    )
    .await
    .and_then(|replay| replay_command(command, &replay))
    .map_err(|fault| replay_problem(fault).into())
}

fn replay_command<C: ProjectCommand>(
    command: &C,
    replay: &CommandReplay,
) -> Result<SettledCommand<C>, ReplayFault> {
    command.check_replay_binding(replay)?;
    let outcome = replay.outcome::<NoEffect<C>, Conflict<C>, Refusal<C>>(C::SPEC.applied)?;
    let zero_authority_effect = match &outcome {
        TransitionOutcome::Applied(()) => None,
        TransitionOutcome::NoEffect(_)
        | TransitionOutcome::Conflicted(_)
        | TransitionOutcome::Refused(_) => {
            if replay.author_action_sequence.is_some() {
                return Err(ReplayFault::Unavailable(
                    "a zero-authority Receipt has an Author Action".into(),
                ));
            }
            let zero = command.zero_authority();
            zero.map_or(Ok(None), |zero| zero.decode_zero_authority_effect(replay))?
        }
    };
    let outcome = match outcome {
        TransitionOutcome::Applied(()) => {
            TransitionOutcome::Applied(C::Profile::replay(|| command.decode(replay), replay)?)
        }
        TransitionOutcome::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
        TransitionOutcome::Conflicted(reason) => TransitionOutcome::Conflicted(reason),
        TransitionOutcome::Refused(reason) => TransitionOutcome::Refused(reason),
    };
    Ok(ProjectCommandSettlement {
        zero_authority_effect,
        ids: replay.ids.clone(),
        receipt_created_at: replay.receipt_created_at.clone(),
        response: C::SPEC.response.replay(replay)?,
        outcome,
    })
}

pub(crate) fn unavailable(
    error: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> ProjectCommandError {
    ProjectCommandError::Unavailable(error.into())
}

#[cfg(test)]
#[path = "command_sequence_tests.rs"]
pub(crate) mod tests;
