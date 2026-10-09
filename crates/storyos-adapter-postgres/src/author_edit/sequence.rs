//! `applyAuthorEdit` through the admit step and the settle step of the command sequence
//! (ADR 0044).

use storyos_application::{
    ApplyAuthorEditCommand, AuthorEditError, ProjectCommandEnvelope, ProjectCommandError,
    RefusedEditDraftIdentity,
};
use storyos_contracts::{DraftRetryReplacement, SourceDraftDisposition};
use storyos_core::{
    AuthorEditApplied, AuthorEditConflict, AuthorEditNoEffect, AuthorEditRefused, ManuscriptBlock,
    TransitionOutcome,
};
use tokio_postgres::Client;
use uuid::Uuid;

use super::profile::{
    AuthorEditProfile, AuthorEditSelector, AuthorEditWrite, RevisionEdit, project_error,
};
use super::replay::{AUTHOR_EDIT_REPLAY_EFFECT, check_replay_records, replayed_record};
use super::{AuthorEditFault, ClassifiedAuthorEdit, classify_author_edit, fault_error};
use crate::author_edit_proposal::{ProposalEditContext, append_proposal_revision_as};
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    Admission, AdmittedCommand, AdmittedRefusal, AppliedVariant, Classification, CommandError,
    CommandIsolation, CommandSpec, DirectEditorAdmission, LockedProject, MissingAdmission,
    NoResponse, ProfileSequences, ProjectCommand, RateLimitedChallenge, ReceiptHeads, ReceiptRefs,
    ReceiptTime, ReplayEffect, RevisionBase, RevisionMembers, RevisionWrite, ZeroAuthorityRows,
    ZeroOutcome, ZeroReceipt, unavailable,
};
use crate::draft_retry::{SupersedeWrite, plan_disposition, write_supersede};
use crate::undo_compensation::{AuthorEditVariant, ForwardCommand};

/// The settlement error of an Author Edit. The sequence error is the Author Edit error.
pub(crate) struct AuthorEditFailure(pub(crate) AuthorEditError);

impl From<ProjectCommandError> for AuthorEditFailure {
    fn from(error: ProjectCommandError) -> Self {
        Self(match error {
            ProjectCommandError::BindingConflict | ProjectCommandError::MissingProject => {
                AuthorEditError::BindingConflict
            }
            ProjectCommandError::InvalidChallenge => AuthorEditError::InvalidChallenge,
            ProjectCommandError::WriterIneligible => AuthorEditError::StaleWriter,
            ProjectCommandError::HistoricalAcknowledgementUnavailable => {
                AuthorEditError::Unavailable(Box::new(std::io::Error::other(
                    "the Author Edit acknowledgement is unavailable",
                )))
            }
            ProjectCommandError::Unavailable(source) => AuthorEditError::Unavailable(source),
        })
    }
}

impl From<AuthorEditError> for AuthorEditFailure {
    fn from(error: AuthorEditError) -> Self {
        Self(error)
    }
}

impl CommandError for AuthorEditFailure {
    fn sequence_error(&self) -> Option<&ProjectCommandError> {
        None
    }

    fn reason_code(&self) -> &'static str {
        match self.0 {
            AuthorEditError::BindingConflict => "binding_conflict",
            AuthorEditError::InvalidChallenge => "invalid_challenge",
            AuthorEditError::StaleWriter => "stale_writer",
            AuthorEditError::AdmissionExpired => "admission_expired",
            AuthorEditError::Unavailable(_) => "unavailable",
        }
    }
}

/// The facts that the acknowledgement of every Author Edit outcome records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthorEditRecord {
    pub(crate) source_draft_disposition: Option<SourceDraftDisposition>,
    pub(crate) replacement_provenance: Option<DraftRetryReplacement>,
    pub(crate) completed_intent_record_id: String,
    pub(crate) local_intent_sequence: u64,
}

/// The applied effect of an Author Edit besides its profile records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthorEditEffect {
    pub(crate) record: AuthorEditRecord,
    /// The new Proposal Revision of a `proposal_revised` outcome.
    pub(crate) proposal_revision_id: Option<String>,
}

/// The effect of a zero-authority Author Edit outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthorEditZeroEffect {
    pub(crate) record: AuthorEditRecord,
    /// The Refused Edit Draft of a `refused_to_draft` outcome.
    pub(crate) draft: Option<RefusedEditDraftIdentity>,
    pub(crate) current_authoritative_revision_id: String,
}

/// The locked facts of an applied Author Edit that `apply` and the profile reuse.
pub(crate) struct AuthorEditPlan {
    kind: AuthorEditSelector,
    current_revision_id: String,
    successor_blocks: Option<Vec<ManuscriptBlock>>,
    proposal_context: Option<ProposalEditContext>,
    source: Option<SourceDraftDisposition>,
    supersede: Option<SupersedeWrite>,
}

impl AuthorEditPlan {
    /// The record set that the plan selects.
    pub(super) fn kind(&self) -> AuthorEditSelector {
        self.kind
    }
}

/// One Author Edit command. It has its fault point for the process-cut tests.
pub(crate) struct AuthorEdit<'a> {
    pub(crate) command: &'a ApplyAuthorEditCommand,
    pub(crate) fault: AuthorEditFault,
    /// The identities of a Refused Edit Draft that the edit can create.
    draft: RefusedEditDraftIdentity,
    /// The identity of a Proposal Revision that the edit can append.
    proposal_revision_id: String,
    /// The identity of the close event of a superseded source Draft.
    source_close_event_id: String,
}

impl<'a> AuthorEdit<'a> {
    pub(crate) fn new(command: &'a ApplyAuthorEditCommand, fault: AuthorEditFault) -> Self {
        Self {
            command,
            fault,
            draft: crate::refused_edit_draft::new_identity(),
            proposal_revision_id: Uuid::now_v7().to_string(),
            source_close_event_id: Uuid::now_v7().to_string(),
        }
    }

    fn record(
        &self,
        source: Option<SourceDraftDisposition>,
        refused_to_draft: bool,
    ) -> AuthorEditRecord {
        AuthorEditRecord {
            replacement_provenance: if refused_to_draft {
                crate::draft_retry::replacement_provenance(
                    source.as_ref(),
                    self.command.retry_source.as_ref(),
                )
            } else {
                None
            },
            source_draft_disposition: source,
            completed_intent_record_id: self.command.completed_intent_record_id.clone(),
            local_intent_sequence: self.command.local_intent_sequence,
        }
    }

    /// The error of an Admission that the head query does not find, as on `main`: expired, a stale
    /// writer, or a binding conflict.
    async fn diagnose_closed_admission(&self, client: &Client) -> AuthorEditFailure {
        let command = self.command;
        let diagnosis = client
            .query_one(
                "SELECT EXISTS (
                   SELECT 1 FROM storyos.author_command_admissions
                    WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                      AND author_command_admission_id = $3::text::uuid
                      AND challenge_expires_at <= clock_timestamp()),
                        EXISTS (
                   SELECT 1 FROM storyos.author_command_admissions AS admission
                    WHERE admission.owner_user_id = $1::text::uuid
                      AND admission.project_id = $2::text::uuid
                      AND admission.author_command_admission_id = $3::text::uuid
                      AND admission.writer_generation < (
                        SELECT max(current_writer.writer_generation)
                          FROM storyos.project_writer_generations AS current_writer
                         WHERE (current_writer.owner_user_id, current_writer.project_id) =
                               (admission.owner_user_id, admission.project_id)))",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.ids.author_command_admission_id,
                ],
            )
            .await;
        AuthorEditFailure(match diagnosis {
            Ok(row) if row.get::<_, bool>(/*idx*/ 0) => AuthorEditError::AdmissionExpired,
            Ok(row) if row.get::<_, bool>(/*idx*/ 1) => AuthorEditError::StaleWriter,
            Ok(_) => AuthorEditError::BindingConflict,
            Err(error) => AuthorEditError::Unavailable(Box::new(error)),
        })
    }

    fn direct_editor_admission(&self) -> Admission {
        let command = self.command;
        Admission::DirectEditorAction(DirectEditorAdmission {
            editor_session_id: command.editor_session_id.as_ref().to_owned(),
            writer_generation: command.writer_generation,
            chapter_object_id: command.chapter_id.clone(),
            expected_authoritative_revision_id: command.expected_authoritative_revision_id.clone(),
            expected_proposal_head_revision_ids: command
                .expected_proposal_head_revision_ids
                .clone(),
            target_refs: command.target_refs.clone(),
            observed_ownership_partition: command.observed_ownership_partition.clone(),
            editor_contract_revision: command.editor_contract_revision.clone(),
            undo_group_id: command.undo_group_id.clone(),
            completed_intent_record_id: command.completed_intent_record_id.clone(),
            local_intent_sequence: command.local_intent_sequence,
        })
    }

    /// The Receipt references of a recorded source Draft disposition.
    fn source_refs(
        source: Option<&SourceDraftDisposition>,
    ) -> Result<ReceiptRefs, ProjectCommandError> {
        let mut refs = ReceiptRefs::default();
        if let Some(source) = source {
            refs.source_draft_disposition =
                Some(serde_json::to_string(source).map_err(unavailable)?);
            refs.created_at = ReceiptTime::Transaction;
            if let SourceDraftDisposition::ClosedSuperseded {
                source_draft_id,
                closure_event_ref,
                ..
            } = source
            {
                refs.draft_artifact_refs.push(source_draft_id.clone());
                refs.artifact_lifecycle_event_refs
                    .push(closure_event_ref.clone());
            }
        }
        Ok(refs)
    }
}

/// The result kind that the Receipt of one classified Author Edit records.
fn result_kind(outcome: &storyos_core::ApplyAuthorEditOutcome) -> &'static str {
    match outcome {
        TransitionOutcome::Applied(applied) => applied.result_kind(),
        TransitionOutcome::NoEffect(_)
        | TransitionOutcome::Conflicted(_)
        | TransitionOutcome::Refused(_) => outcome.receipt_result_kind(),
    }
}

impl ProjectCommand for AuthorEdit<'_> {
    const SPEC: CommandSpec = CommandSpec {
        kind: "applyAuthorEdit",
        applied: &[
            AppliedVariant::Forward(ForwardCommand::AuthorEdit(
                AuthorEditVariant::AuthoritativeApplied,
            )),
            AppliedVariant::Forward(ForwardCommand::AuthorEdit(
                AuthorEditVariant::ProposalRevised,
            )),
        ],
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::Diagnosed,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The `AuthoritativeRevision` profile writes the Activity record.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(AUTHOR_EDIT_REPLAY_EFFECT),
    };
    type Error = AuthorEditFailure;
    type Profile = AuthorEditProfile;
    type Response = NoResponse;
    type ZeroEffect = AuthorEditZeroEffect;
    type Applied = AuthorEditApplied;
    type Plan = AuthorEditPlan;
    type Effect = AuthorEditEffect;
    type NoEffect = AuthorEditNoEffect;
    type Conflict = AuthorEditConflict;
    type Refusal = AuthorEditRefused;

    async fn classify(
        &self,
        client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, AuthorEditFailure> {
        let command = self.command;
        let scope = &command.project_scope;
        // The Admission, its current writer, and the Chapter head, under the head lock, as on
        // `main`. A writer takeover can settle while this query waits.
        let row = client
            .query_opt(
                "SELECT head.current_revision_id::text,
                        convert_from(payload.canonical_bytes, 'UTF8')
                   FROM storyos.author_command_admissions AS admission
                   JOIN storyos.project_writer_generations AS writer
                     ON (writer.owner_user_id, writer.project_id,
                         writer.current_editor_session_id, writer.writer_generation) =
                        (admission.owner_user_id, admission.project_id,
                         admission.editor_session_id, admission.writer_generation)
                    AND writer.writer_generation = (
                      SELECT max(current_writer.writer_generation)
                        FROM storyos.project_writer_generations AS current_writer
                       WHERE (current_writer.owner_user_id, current_writer.project_id) =
                             (admission.owner_user_id, admission.project_id)
                    )
                   JOIN storyos.authoritative_heads AS head
                     ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                        (admission.owner_user_id, admission.project_id,
                         admission.chapter_object_id)
                   JOIN storyos.authoritative_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id,
                         revision.manuscript_object_id, revision.revision_id) =
                        (head.owner_user_id, head.project_id,
                         head.manuscript_object_id, head.current_revision_id)
                   JOIN storyos.authoritative_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                        (revision.owner_user_id, revision.project_id, revision.payload_id)
                  WHERE admission.owner_user_id = $1::text::uuid
                    AND admission.project_id = $2::text::uuid
                    AND admission.author_command_admission_id = $3::text::uuid
                    AND admission.challenge_expires_at > clock_timestamp()
                   AND NOT EXISTS (
                     SELECT 1 FROM storyos.author_command_admission_settlements AS settlement
                      WHERE (settlement.owner_user_id, settlement.project_id,
                             settlement.author_command_admission_id) =
                            (admission.owner_user_id, admission.project_id,
                             admission.author_command_admission_id)
                   )
                  FOR UPDATE OF head",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &command.ids.author_command_admission_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        let Some(row) = row else {
            return Err(self.diagnose_closed_admission(client).await);
        };
        let current_revision_id: String = row.get(/*idx*/ 0);
        let current_body: String = row.get(/*idx*/ 1);
        let retry_source = crate::draft_retry::load_source(client, command).await?;
        let ClassifiedAuthorEdit {
            result,
            successor_blocks,
            proposal_context,
        } = if retry_source
            .as_ref()
            .is_some_and(|source| !source.input_matches)
        {
            ClassifiedAuthorEdit {
                result: TransitionOutcome::Conflicted(AuthorEditConflict::OwnershipChanged),
                successor_blocks: None,
                proposal_context: None,
            }
        } else {
            classify_author_edit(client, command, &current_revision_id, current_body).await?
        };
        let (source, supersede) = plan_disposition(
            retry_source.map(|source| source.disposition),
            result_kind(&result),
            self.source_close_event_id.clone(),
        )?;
        let heads = ReceiptHeads {
            expected: vec![command.expected_authoritative_revision_id.clone()],
            prior: vec![current_revision_id.clone()],
            resulting: vec![current_revision_id.clone()],
        };
        let mut refs = Self::source_refs(source.as_ref())?;
        let mut fields = serde_json::Map::new();
        let mut draft = None;
        let outcome = match result {
            TransitionOutcome::Applied(applied) => {
                let kind = match &applied {
                    AuthorEditApplied::AuthoritativeApplied { .. } => AuthorEditSelector::Revision,
                    AuthorEditApplied::ProposalRevised { .. } => {
                        if proposal_context.is_none() {
                            return Err(ProjectCommandError::BindingConflict.into());
                        }
                        AuthorEditSelector::Proposal
                    }
                };
                TransitionOutcome::Applied((
                    applied,
                    AuthorEditPlan {
                        kind,
                        current_revision_id: current_revision_id.clone(),
                        successor_blocks,
                        proposal_context,
                        source: source.clone(),
                        supersede,
                    },
                ))
            }
            TransitionOutcome::NoEffect(reason) => TransitionOutcome::NoEffect(reason),
            TransitionOutcome::Conflicted(reason) => {
                fields.insert(
                    "current_authoritative_revision_id".to_owned(),
                    current_revision_id.clone().into(),
                );
                TransitionOutcome::Conflicted(reason)
            }
            TransitionOutcome::Refused(AuthorEditRefused::RefusedToDraft) => {
                let identity = self.draft.clone();
                for (key, value) in [
                    ("draft_id", &identity.draft_id),
                    ("draft_revision_id", &identity.draft_revision_id),
                    ("creation_event_id", &identity.creation_event_id),
                ] {
                    fields.insert(key.to_owned(), value.clone().into());
                }
                refs.draft_artifact_refs
                    .insert(/*index*/ 0, identity.draft_id.clone());
                refs.artifact_lifecycle_event_refs
                    .insert(/*index*/ 0, identity.creation_event_id.clone());
                draft = Some(identity);
                TransitionOutcome::Refused(AuthorEditRefused::RefusedToDraft)
            }
            TransitionOutcome::Refused(reason) => TransitionOutcome::Refused(reason),
        };
        let effect = AuthorEditZeroEffect {
            record: self.record(source, draft.is_some()),
            draft,
            current_authoritative_revision_id: current_revision_id,
        };
        Ok(Classification {
            outcome,
            admission: self.direct_editor_admission(),
            heads,
            zero_receipt: ZeroReceipt::Observed {
                fields,
                refs,
                effect,
            },
        })
    }

    async fn diagnose_missing_admission(
        &self,
        client: &Client,
        _envelope: &ProjectCommandEnvelope,
    ) -> AuthorEditFailure {
        let command = self.command;
        let stale_writer = client
            .query_one(
                "SELECT EXISTS (
                   SELECT 1 FROM storyos.editor_sessions AS session
                    WHERE session.owner_user_id = $1::text::uuid
                      AND session.project_id = $2::text::uuid
                      AND session.editor_session_id = $3::text::uuid)
                  AND NOT EXISTS (
                   SELECT 1 FROM storyos.project_writer_generations AS writer
                    WHERE writer.owner_user_id = $1::text::uuid
                      AND writer.project_id = $2::text::uuid
                      AND writer.current_editor_session_id = $3::text::uuid
                      AND writer.writer_generation = $4::text::numeric
                      AND writer.writer_generation = (
                        SELECT max(current_writer.writer_generation)
                          FROM storyos.project_writer_generations AS current_writer
                         WHERE current_writer.owner_user_id = $1::text::uuid
                           AND current_writer.project_id = $2::text::uuid))",
                &[
                    &command.project_scope.owner_user_id.as_ref(),
                    &command.project_scope.project_id.as_ref(),
                    &command.editor_session_id.as_ref(),
                    &command.writer_generation.to_string(),
                ],
            )
            .await
            .map(|row| row.get::<_, bool>(/*idx*/ 0));
        AuthorEditFailure(match stale_writer {
            Ok(true) => AuthorEditError::StaleWriter,
            Ok(false) => AuthorEditError::BindingConflict,
            Err(error) => AuthorEditError::Unavailable(Box::new(error)),
        })
    }

    fn applied_variant(
        &self,
        applied: &AuthorEditApplied,
        _plan: &AuthorEditPlan,
    ) -> AppliedVariant {
        AppliedVariant::Forward(ForwardCommand::AuthorEdit(match applied {
            AuthorEditApplied::AuthoritativeApplied { .. } => {
                AuthorEditVariant::AuthoritativeApplied
            }
            AuthorEditApplied::ProposalRevised { .. } => AuthorEditVariant::ProposalRevised,
        }))
    }

    fn applied_receipt_refs(
        &self,
        _applied: &AuthorEditApplied,
        plan: &AuthorEditPlan,
    ) -> ReceiptRefs {
        // The source disposition serializes, because classify serialized it.
        let mut refs = Self::source_refs(plan.source.as_ref()).unwrap_or_default();
        if let AuthorEditSelector::Proposal = plan.kind {
            refs.proposal_revision_ids = vec![self.proposal_revision_id.clone()];
        }
        refs
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ProfileSequences<Self>,
        plan: AuthorEditPlan,
        applied: AuthorEditApplied,
    ) -> Result<AuthorEditWrite<AuthorEditEffect>, ProjectCommandError> {
        let command = self.command;
        let record = self.record(plan.source.clone(), /*refused_to_draft*/ false);
        match applied {
            AuthorEditApplied::AuthoritativeApplied { body } => {
                let (payload, members) = match plan.successor_blocks {
                    Some(blocks) => (
                        crate::manuscript_block::persist_canonical_bytes(&blocks),
                        RevisionMembers::Blocks(blocks),
                    ),
                    None => (
                        body,
                        RevisionMembers::CopyFrom(plan.current_revision_id.clone()),
                    ),
                };
                Ok(AuthorEditWrite::Revision(Box::new(RevisionEdit {
                    revision: RevisionWrite {
                        effect: AuthorEditEffect {
                            record,
                            proposal_revision_id: None,
                        },
                        chapter_id: command.chapter_id.clone(),
                        prior_revision_id: plan.current_revision_id,
                        payload,
                        members,
                        disposition: crate::command_sequence::ActionDisposition::Forward,
                        editor_session_id: command.editor_session_id.as_ref().to_owned(),
                        writer_base: RevisionBase::Required,
                    },
                    inline: plan
                        .proposal_context
                        .filter(|context| context.kind == "inline_edit"),
                    supersede: plan.supersede,
                })))
            }
            AuthorEditApplied::ProposalRevised { candidate_text } => {
                let context = plan
                    .proposal_context
                    .ok_or(ProjectCommandError::BindingConflict)?;
                let proposal_revision_id = append_proposal_revision_as(
                    client,
                    &envelope.project_scope,
                    &context,
                    &plan.current_revision_id,
                    &candidate_text,
                    self.proposal_revision_id.clone(),
                )
                .await
                .map_err(project_error)?;
                Ok(AuthorEditWrite::Proposal {
                    effect: AuthorEditEffect {
                        record,
                        proposal_revision_id: Some(proposal_revision_id),
                    },
                    supersede: plan.supersede,
                })
            }
        }
    }

    /// Requires the request payload, the Admission facts, and the expected head of the stored
    /// Receipt. A Receipt of another request is a binding conflict.
    fn check_replay_binding(&self, replay: &CommandReplay) -> Result<(), ReplayFault> {
        let command = self.command;
        let effect = replay.effect();
        let expected: Option<Vec<String>> = effect
            .fields()
            .get("expected_heads")
            .and_then(|value| serde_json::from_value(value.clone()).ok());
        let target_refs: Option<Vec<String>> = effect
            .fields()
            .get("admission_target_refs")
            .and_then(|value| serde_json::from_value(value.clone()).ok());
        if !replay.admission_matches
            || !replay.fence_digest_matches
            || effect.text("admission_chapter_id")?.as_deref() != Some(command.chapter_id.as_str())
            || effect.text("admission_expected_revision_id")?.as_deref()
                != Some(command.expected_authoritative_revision_id.as_str())
            || target_refs.as_ref() != Some(&command.target_refs)
            || expected.as_deref()
                != Some(std::slice::from_ref(
                    &command.expected_authoritative_revision_id,
                ))
        {
            return Err(ReplayFault::BindingConflict);
        }
        check_replay_records(replay)
    }

    fn decode(&self, replay: &CommandReplay) -> Result<AuthorEditEffect, ReplayFault> {
        let effect = replay.effect();
        Ok(AuthorEditEffect {
            record: replayed_record(replay, /*refused_to_draft*/ false)?,
            proposal_revision_id: match replay.result_kind() {
                "proposal_revised" => Some(effect.required("proposal_revision_id")?),
                _ => None,
            },
        })
    }

    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        match outcome {
            ZeroOutcome::Refused(AuthorEditRefused::RefusedToDraft) => ZeroAuthorityRows::Effect,
            ZeroOutcome::NoEffect(_) | ZeroOutcome::Conflicted(_) | ZeroOutcome::Refused(_) => {
                ZeroAuthorityRows::None
            }
        }
    }

    async fn write_zero_authority_effect(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _outcome: &ZeroOutcome<'_, Self>,
    ) -> Result<AuthorEditZeroEffect, ProjectCommandError> {
        let command = self.command;
        let identity = crate::refused_edit_draft::persist(client, command, self.draft.clone())
            .await
            .map_err(project_error)?;
        // The settle step holds the lock of the source Draft, so its facts do not change.
        let retry_source = crate::draft_retry::load_source(client, command)
            .await
            .map_err(project_error)?;
        let (source, supersede) = plan_disposition(
            retry_source.map(|source| source.disposition),
            "refused_to_draft",
            self.source_close_event_id.clone(),
        )
        .map_err(project_error)?;
        if let Some(supersede) = supersede {
            write_supersede(
                client,
                &envelope.project_scope,
                &envelope.ids.receipt_id,
                &supersede,
                "refused_to_draft",
                /*action_sequence*/ None,
            )
            .await
            .map_err(project_error)?;
        }
        let current_authoritative_revision_id = client
            .query_one(
                "SELECT current_revision_id::text FROM storyos.authoritative_heads
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND manuscript_object_id = $3::text::uuid",
                &[
                    &envelope.project_scope.owner_user_id.as_ref(),
                    &envelope.project_scope.project_id.as_ref(),
                    &command.chapter_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .get(/*idx*/ 0);
        Ok(AuthorEditZeroEffect {
            record: self.record(source, /*refused_to_draft*/ true),
            draft: Some(identity),
            current_authoritative_revision_id,
        })
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<AuthorEditZeroEffect>, ReplayFault> {
        let refused_to_draft = replay.result_kind() == "refused_to_draft";
        let effect = replay.effect();
        let draft = if refused_to_draft {
            let identity = RefusedEditDraftIdentity {
                draft_id: replay.receipt_uuid("draft_id")?,
                draft_revision_id: replay.receipt_uuid("draft_revision_id")?,
                creation_event_id: replay.receipt_uuid("creation_event_id")?,
            };
            if effect.text("draft_id")?.as_ref() != Some(&identity.draft_id)
                || effect.text("draft_revision_id")?.as_ref() != Some(&identity.draft_revision_id)
                || effect.text("creation_event_id")?.as_ref() != Some(&identity.creation_event_id)
            {
                return Err(ReplayFault::Unavailable(
                    "the Refused Edit Draft does not match its Receipt".into(),
                ));
            }
            Some(identity)
        } else {
            None
        };
        Ok(Some(AuthorEditZeroEffect {
            record: replayed_record(replay, refused_to_draft)?,
            draft,
            current_authoritative_revision_id: replay
                .resulting_heads
                .first()
                .cloned()
                .ok_or_else(|| ReplayFault::Unavailable("the Receipt has no head".into()))?,
        }))
    }
}

impl AdmittedCommand for AuthorEdit<'_> {
    fn admission(&self, _envelope: &ProjectCommandEnvelope) -> Admission {
        self.direct_editor_admission()
    }

    fn admitted_refusal(refusal: AdmittedRefusal) -> AuthorEditFailure {
        AuthorEditFailure(match refusal {
            AdmittedRefusal::Expired => AuthorEditError::AdmissionExpired,
            AdmittedRefusal::StaleWriter => AuthorEditError::StaleWriter,
        })
    }

    fn before_commit(&self) -> Result<(), AuthorEditFailure> {
        if self.fault == AuthorEditFault::CoreBeforeCommit {
            return Err(AuthorEditFailure(fault_error("CFP-CORE-BEFORE-COMMIT")));
        }
        Ok(())
    }
}
