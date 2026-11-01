//! The fixed Author Command Admission, Core Transition, and Command Acknowledgement sequence
//! of the project commands (ADR 0041, ADR 0076).

use std::future::Future;
use std::pin::Pin;

use storyos_application::{
    DiagnosticField, DiagnosticId, ProjectCommandEnvelope, ProjectCommandError,
    ProjectCommandSettlement, ProjectScope,
};
use storyos_core::{ReasonCode, ReceiptResult, TransitionOutcome};
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
mod legacy;
mod records;
mod response;
mod settle;
mod settlement_shape;
mod skeleton;
mod structural;
mod structural_records;
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
pub(crate) use legacy::{LegacyProjectCommand, LegacySpec};
pub(crate) use records::{CommitCounters, allocate_commit_counters, insert_author_action};
use records::{ReceiptRecord, insert_activity_payload, insert_receipt, lock_project};
pub(crate) use response::{
    CommandResponse, NoResponse, ProjectAssistanceResponse, ProjectResponse, ResponseRecord,
};
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
pub(crate) use zero_authority::{NoZeroAuthority, ZeroAuthority};

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

/// The declared facts of one command that the sequence reads (ADR 0080). `E` is the settlement
/// error of the command.
pub(crate) struct CommandSpec<E = ProjectCommandError> {
    pub(crate) kind: storyos_contracts::CommandKind,
    /// The closed set of applied variants. A command of one variant names only it.
    pub(crate) applied: &'static [AppliedVariant],
    pub(crate) activity_kind: &'static str,
    pub(crate) replay_effect: ReplayEffect,
    pub(crate) response: CommandResponse,
    /// The refusal of a serialization failure, a unique violation, or a deadlock of the first-use
    /// transaction, including its commit. `None` keeps such a failure a store fault.
    pub(crate) contention_refusal: Option<fn() -> E>,
    /// The Domain Receipt payload of an applied outcome that the default `applied_receipt` gives.
    pub(crate) receipt_payload: &'static str,
}

/// The Project row that the sequence locks before a command loads its own facts.
pub(crate) struct LockedProject {
    pub(crate) tree_revision: u64,
    pub(crate) current_chapter_id: Option<String>,
}

/// The authority records that an applied project command allocates and writes (ADR 0076).
///
/// A command names one profile. The sequence allocates the profile sequences through the plan
/// (`SelectsRecords`) before the Domain Receipt, and calls `persist` after the command effect rows.
/// On an exact retry, it calls `replay` instead. Implementations never run transaction control
/// and never write the Admission, Receipt, settlement link, or idempotency rows.
pub(crate) trait SettlementProfile {
    type Sequences: Send + Sync;
    /// The applied writes that a command of this profile returns.
    type Write<E: Send>: Send;
    /// The applied value of the settlement, equal on first delivery and replay.
    type Applied<E: Send>: Send + AppliedRecord<Effect = E>;
    /// The settlement facts besides the envelope that `persist` reads.
    type Input: PersistInput + Send;

    /// The Authoritative Commit identities that the Domain Receipt binds.
    fn commit_ids(sequences: &Self::Sequences) -> Vec<String>;

    /// The Authoritative Revisions that the applied outcome creates. The Domain Receipt binds
    /// them and gives them as its resulting heads.
    fn revision_ids(_sequences: &Self::Sequences) -> Vec<String> {
        Vec::new()
    }

    fn persist<E: Send>(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        input: Self::Input,
        sequences: Self::Sequences,
        write: Self::Write<E>,
    ) -> impl Future<Output = Result<Self::Applied<E>, ProjectCommandError>> + Send;

    /// Checks the authority evidence of the profile, and only then runs `decode`, so a command
    /// decode never hides damaged authority evidence.
    fn replay<E: Send>(
        decode: impl FnOnce() -> Result<E, ReplayFault>,
        replay: &CommandReplay,
    ) -> Result<Self::Applied<E>, ReplayFault>;
}

/// The settlement facts of one applied outcome besides the envelope.
pub(crate) struct PersistFacts<'a> {
    pub(crate) project: &'a LockedProject,
    pub(crate) activity_kind: &'static str,
    pub(crate) variant: AppliedVariant,
}

/// The input of `persist` of one profile: only the settlement facts that the profile reads.
pub(crate) trait PersistInput {
    fn from_facts(facts: &PersistFacts<'_>) -> Self;
}

/// A profile that reads only the envelope.
impl PersistInput for () {
    fn from_facts(_facts: &PersistFacts<'_>) -> Self {}
}

/// A profile whose records record the Receipt result kind of the applied variant.
impl PersistInput for AppliedVariant {
    fn from_facts(facts: &PersistFacts<'_>) -> Self {
        facts.variant
    }
}

/// A settlement profile of one record set. Every plan allocates its sequences.
pub(crate) trait SingleRecordSet: SettlementProfile {
    fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<Self::Sequences, ProjectCommandError>> + Send;
}

/// Allocates the sequences of the record set of the settlement profile `P` that one applied plan
/// selects.
///
/// A profile of one record set has a blanket implementation for every plan. A profile of more
/// than one record set selects the set from the plan.
pub(crate) trait SelectsRecords<P: SettlementProfile> {
    fn allocate_records(
        &self,
        client: &Client,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<P::Sequences, ProjectCommandError>> + Send;
}

impl<P: SingleRecordSet, T> SelectsRecords<P> for T {
    fn allocate_records(
        &self,
        client: &Client,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<P::Sequences, ProjectCommandError>> + Send {
        P::allocate(client, scope)
    }
}

/// The parameters of the settlement type of one command (ADR 0080).
pub(crate) type Effect<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::Effect;
pub(crate) type NoEffect<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::NoEffect;
pub(crate) type Conflict<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::Conflict;
pub(crate) type Refusal<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::Refusal;
pub(crate) type ZeroEffect<C> = <<C as ProjectCommand>::Settlement as SettlementShape>::ZeroEffect;

/// A Core Transition Outcome whose applied value carries the locked facts that `apply` reuses.
pub(crate) type Classified<C> = TransitionOutcome<
    (<C as ProjectCommand>::Applied, <C as ProjectCommand>::Plan),
    NoEffect<C>,
    Conflict<C>,
    Refusal<C>,
>;

/// The Core Transition Outcome of one command and the facts that the sequence records with it.
pub(crate) struct Classification<C: ProjectCommand + ?Sized> {
    pub(crate) outcome: Classified<C>,
    pub(crate) admission: Admission,
    pub(crate) heads: ReceiptHeads,
    pub(crate) zero_receipt: ZeroReceipt<ZeroEffect<C>>,
}

impl<C: ProjectCommand + ?Sized> Classification<C> {
    /// An explicit project command whose Receipt has empty head arrays.
    pub(crate) fn project_command(outcome: Classified<C>) -> Self {
        Self {
            outcome,
            admission: Admission::Project(ProjectActionClass::ExplicitProjectCommand),
            heads: ReceiptHeads::default(),
            zero_receipt: ZeroReceipt::Reason,
        }
    }
}

/// The Domain Receipt payload and references of a zero-authority outcome.
pub(crate) enum ZeroReceipt<Z> {
    /// The payload `{"reason": code}` and no references.
    Reason,
    /// The payload fields that the command observed, which the sequence completes with `reason`.
    ///
    /// The settlement returns `effect`, unless the outcome writes effect rows, which give it.
    Observed {
        fields: serde_json::Map<String, serde_json::Value>,
        refs: ReceiptRefs,
        effect: Z,
    },
}

/// The scope sequences that one command's settlement profile allocates.
pub(crate) type ProfileSequences<C> =
    <<C as ProjectCommand>::Profile as SettlementProfile>::Sequences;

/// A zero-authority outcome, which a command can settle with effect rows (ADR 0076).
pub(crate) enum ZeroOutcome<'a, C: ProjectCommand + ?Sized> {
    NoEffect(&'a NoEffect<C>),
    Conflicted(&'a Conflict<C>),
    Refused(&'a Refusal<C>),
}

impl<C: ProjectCommand + ?Sized> ZeroOutcome<'_, C> {
    /// The Domain Receipt result kind of the outcome, which its reason type can give.
    pub(crate) fn receipt_result_kind(&self) -> &'static str {
        match self {
            Self::NoEffect(reason) => reason
                .receipt_result()
                .unwrap_or(ReceiptResult::NoEffect.code()),
            Self::Conflicted(reason) => reason
                .receipt_result()
                .unwrap_or(ReceiptResult::Conflicted.code()),
            Self::Refused(reason) => reason
                .receipt_result()
                .unwrap_or(ReceiptResult::Refused.code()),
        }
    }
}

/// The rows that one zero-authority outcome writes besides its Admission, Receipt, and fence.
pub(crate) enum ZeroAuthorityRows {
    None,
    /// Effect rows without a Project Activity record.
    Effect,
    /// Effect rows and one Activity record at a position that the sequence allocates.
    EffectWithActivity,
}

/// The effect and the whole Activity payload of a zero-authority outcome that writes effect rows.
pub(crate) struct ZeroAuthorityWrite<Z> {
    pub(crate) effect: Z,
    /// The complete payload, including its `kind`. The event kind is the command Activity kind.
    pub(crate) activity: serde_json::Value,
}

/// The applied writes that one command returns for its settlement profile.
pub(crate) type ProfileWrite<C> =
    <<C as ProjectCommand>::Profile as SettlementProfile>::Write<Effect<C>>;

/// The applied value that one command's settlement profile returns after its authority records.
pub(crate) type ProfileApplied<C> =
    <<C as ProjectCommand>::Profile as SettlementProfile>::Applied<Effect<C>>;

/// The Domain Receipt variant, payload, and references of one applied outcome.
pub(crate) struct AppliedReceipt {
    pub(crate) variant: AppliedVariant,
    pub(crate) payload: String,
    pub(crate) refs: ReceiptRefs,
}

/// The future of a write that runs after the profile records.
pub(crate) type AuthorityWrite<'c> =
    Pin<Box<dyn Future<Output = Result<(), ProjectCommandError>> + Send + 'c>>;

/// A write of the effect rows that refer to the authority records of the profile. The sequence
/// runs it after the profile writes those records.
pub(crate) type AfterAuthority<'c, C> = Box<
    dyn for<'a> FnOnce(
            &'a C,
            &'a Client,
            &'a ProjectCommandEnvelope,
            &'a ProfileApplied<C>,
        ) -> AuthorityWrite<'a>
        + Send
        + 'c,
>;

/// The applied writes that one command returns.
pub(crate) struct CommandWrite<'c, C: ProjectCommand + ?Sized> {
    /// The writes that the settlement profile records.
    pub(crate) profile: ProfileWrite<C>,
    pub(crate) after_authority: Option<AfterAuthority<'c, C>>,
}

impl<'c, C: ProjectCommand + ?Sized> CommandWrite<'c, C> {
    /// The applied writes of a command that runs `write` after the profile records.
    pub(crate) fn after_authority(
        profile: ProfileWrite<C>,
        write: impl for<'a> FnOnce(
            &'a C,
            &'a Client,
            &'a ProjectCommandEnvelope,
            &'a ProfileApplied<C>,
        ) -> AuthorityWrite<'a>
        + Send
        + 'c,
    ) -> Self {
        Self {
            profile,
            after_authority: Some(Box::new(write)),
        }
    }
}

/// One project command: its declared profile, its settlement type, its locked facts, its own
/// effect rows, and its applied effect decoder (ADR 0080).
///
/// Implementations never run transaction control (ADR 0031) and never write the Admission,
/// Receipt, settlement link, authority, or idempotency rows. `settle_project_command` writes
/// those rows in one fixed order.
pub(crate) trait ProjectCommand: Sync {
    const SPEC: CommandSpec<Self::Error>;
    /// The settlement error. A command that refuses before its Admission declares
    /// `RefusableCommandError` with its refusal type. The other commands declare
    /// `ProjectCommandError`.
    type Error: CommandError;
    type Profile: SettlementProfile;
    type Plan: Send + SelectsRecords<Self::Profile>;
    type Applied: Send;
    /// The settlement type that the application crate declares for the command.
    type Settlement: SettlementShape;

    /// Locks the command facts and classifies them through Core. A refusal before Admission
    /// rolls back the transaction.
    fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> impl Future<Output = Result<Classification<Self>, Self::Error>> + Send;

    /// The error of an Admission insert that inserts no row. It runs in the transaction of the
    /// insert. The default is an invalid Command Challenge.
    fn diagnose_missing_admission(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
    ) -> impl Future<Output = Self::Error> + Send {
        async { ProjectCommandError::InvalidChallenge.into() }
    }

    /// The Domain Receipt of an applied outcome. The default gives the first applied variant, the
    /// constant payload of the `CommandSpec`, and no references.
    fn applied_receipt(&self, _applied: &Self::Applied, _plan: &Self::Plan) -> AppliedReceipt {
        AppliedReceipt {
            variant: Self::SPEC.applied[0],
            payload: Self::SPEC.receipt_payload.to_owned(),
            refs: ReceiptRefs::default(),
        }
    }

    /// Writes the effect rows of an applied outcome after its profile sequences are allocated.
    fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
        sequences: &ProfileSequences<Self>,
        plan: Self::Plan,
        applied: Self::Applied,
    ) -> impl Future<Output = Result<CommandWrite<'_, Self>, ProjectCommandError>> + Send;

    /// Checks the stored facts that bind the acknowledgement to this request. The sequence runs
    /// it before every other replay check, so a binding conflict comes before damaged evidence.
    fn check_replay_binding(&self, _replay: &CommandReplay) -> Result<(), ReplayFault> {
        Ok(())
    }

    /// Decodes the applied effect from the stored acknowledgement evidence.
    fn decode(&self, replay: &CommandReplay) -> Result<Effect<Self>, ReplayFault>;

    /// The zero-authority rows of the command. The default writes none.
    fn zero_authority(&self) -> Option<&(impl ZeroAuthority<Self> + Sync)> {
        None::<&NoZeroAuthority>
    }
}

pub(crate) type SettledCommand<C> = ProjectCommandSettlement<
    ProfileApplied<C>,
    NoEffect<C>,
    Conflict<C>,
    Refusal<C>,
    <<C as ProjectCommand>::Settlement as SettlementShape>::Response,
    ZeroEffect<C>,
>;

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
    const CONTENTION_REFUSAL: Option<fn() -> C::Error> = C::SPEC.contention_refusal;

    async fn write(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
    ) -> Result<SettledCommand<C>, C::Error> {
        record_ids(
            &envelope.ids.command_id,
            &envelope.ids.author_command_admission_id,
        );
        first_use(client, envelope, self.0).await
    }
}

async fn first_use<C: ProjectCommand>(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<SettledCommand<C>, C::Error> {
    let project = lock_project(client, envelope).await?;
    let Classification {
        outcome,
        admission,
        heads,
        zero_receipt,
    } = command.classify(client, envelope, &project).await?;
    if !insert_admission(client, envelope, C::SPEC.kind.as_str(), &admission).await? {
        return Err(command.diagnose_missing_admission(client, envelope).await);
    }
    settle_classified(
        client,
        envelope,
        command,
        &project,
        outcome,
        heads,
        zero_receipt,
    )
    .await
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
    let zero_authority_effect = match zero_outcome {
        None => None,
        Some(zero_outcome) => match command
            .zero_authority()
            .map_or(ZeroAuthorityRows::None, |zero| {
                zero.zero_authority_rows(&zero_outcome)
            }) {
            ZeroAuthorityRows::None => observed_effect,
            ZeroAuthorityRows::Effect => Some(
                zero_authority(command)?
                    .write_zero_authority_effect(client, envelope, &zero_outcome)
                    .await?,
            ),
            ZeroAuthorityRows::EffectWithActivity => {
                let activity = ActivityOnly::allocate(client, &envelope.project_scope).await?;
                let write = zero_authority(command)?
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
            match command.zero_authority() {
                Some(zero) => zero.decode_zero_authority_effect(replay)?,
                None => None,
            }
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

/// The zero-authority rows of a command whose outcome declares them.
fn zero_authority<C: ProjectCommand>(
    command: &C,
) -> Result<&(impl ZeroAuthority<C> + Sync), ProjectCommandError> {
    command
        .zero_authority()
        .ok_or_else(|| unavailable("the command declares no zero-authority rows"))
}

pub(crate) fn unavailable(
    error: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> ProjectCommandError {
    ProjectCommandError::Unavailable(error.into())
}

#[cfg(test)]
#[path = "command_sequence_tests.rs"]
pub(crate) mod tests;
