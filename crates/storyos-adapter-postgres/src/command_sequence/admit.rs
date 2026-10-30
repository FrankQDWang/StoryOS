//! The admit step: a first use that commits the Admission and the work rows (ADR 0076).
//! The Command Idempotency Fence stays `in_progress` for a later settlement.

use std::future::Future;

use storyos_application::{
    AdmittedProjectCommand, DiagnosticField as _, DiagnosticId, Project, ProjectCommandEnvelope,
    ProjectCommandError,
};
use tokio_postgres::Client;

use super::admission::insert_admission;
use super::records::lock_project;
use super::response::read_response_project;
use super::skeleton::{ChallengeUse, FirstUse, consume_challenge, record_result, replay_problem};
use super::{
    Admission, CommandError, LockedProject, ProjectResponse, RateLimitedChallenge, ReplayEffect,
    ResponseRecord, unavailable,
};
use crate::PostgresProjectReader;
use crate::command_replay::{
    CommandReplay, EffectKey, EffectRecord, ReplayFault, read_command_replay, read_effect,
    read_snapshot, response_project,
};
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, encode_command_response_project,
};

pub(crate) struct AdmitSpec {
    pub(crate) kind: &'static str,
    pub(crate) rate_limited: RateLimitedChallenge,
    /// The query of the work rows. It takes the owner, Project, and Admission identities as `$1`,
    /// `$2`, and `$3`. Its optional row holds one JSON object text.
    pub(crate) work_query: &'static str,
}

/// The first admission that every path of the admit step returns.
pub(crate) type Admitted<C> = AdmittedProjectCommand<
    <C as AdmitCommand>::Work,
    <<C as AdmitCommand>::Response as ResponseRecord>::Response,
>;

/// One project command whose first use commits its Admission and work rows and leaves the
/// Command Idempotency Fence `in_progress`. A later step settles the fence.
///
/// Implementations never run transaction control and never write the Admission or fence rows.
/// `admit_project_command` writes them in one fixed order.
pub(crate) trait AdmitCommand: Sync {
    const SPEC: AdmitSpec;
    /// The admit error. A command that refuses before its Admission declares
    /// `RefusableCommandError` with its refusal type.
    type Error: CommandError;
    type Response: AdmittedResponse;
    /// The facts that the command loads under the Project lock.
    type Facts: Send;
    /// The admitted work, equal on first use and on each exact retry.
    type Work: Send;

    /// Loads the command facts. A refusal before Admission rolls back the transaction.
    fn load_facts(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> impl Future<Output = Result<Self::Facts, Self::Error>> + Send;

    /// The Admission form that the command records for its facts.
    fn admission(&self, facts: &Self::Facts) -> Admission;

    /// Writes the work rows after the Admission.
    fn write_work(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        facts: Self::Facts,
    ) -> impl Future<Output = Result<Self::Work, ProjectCommandError>> + Send;

    /// Decodes the admitted work from the row of the work query. A missing row is damaged
    /// evidence.
    fn decode_work(&self, work: &EffectRecord) -> Result<Self::Work, ReplayFault>;

    /// Accepts the Domain Receipt of the later settlement. A result kind or reason that the
    /// settlement does not write is a binding conflict.
    fn check_settlement(&self, replay: &CommandReplay) -> Result<(), ReplayFault>;
}

/// The acknowledgement record that the admit step records on the fence, which stays
/// `in_progress`. The later settlement keeps the record.
pub(crate) trait AdmittedResponse: ResponseRecord {
    fn record(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        command_kind: &'static str,
    ) -> impl Future<Output = Result<Self::Response, ProjectCommandError>> + Send;

    /// Decodes the record from the fence columns `acknowledgement_format` and
    /// `response_project`.
    fn in_progress(
        format: Option<&str>,
        payload: Option<&str>,
    ) -> Result<Self::Response, ReplayFault>;
}

impl AdmittedResponse for ProjectResponse {
    async fn record(
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        command_kind: &'static str,
    ) -> Result<Project, ProjectCommandError> {
        let project = read_response_project(client, envelope).await?;
        let scope = &envelope.project_scope;
        client
            .execute(
                "UPDATE storyos.command_idempotency
                    SET acknowledgement_format = $3, response_project = $4::text::jsonb
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND command_kind = $5 AND idempotency_key = $6::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &COMMAND_RESPONSE_PROJECT_FORMAT,
                    &encode_command_response_project(&project),
                    &command_kind,
                    &envelope.challenge_binding.idempotency_key,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(project)
    }

    fn in_progress(format: Option<&str>, payload: Option<&str>) -> Result<Project, ReplayFault> {
        response_project(format, payload)
    }
}

#[tracing::instrument(skip_all, fields(
    command_kind = C::SPEC.kind.diagnostic(),
    project_id = envelope.project_scope.project_id.diagnostic(),
    command_id = tracing::field::Empty,
    author_command_admission_id = tracing::field::Empty,
    correlation_id = DiagnosticId(&envelope.correlation_id).diagnostic(),
    outcome = tracing::field::Empty,
))]
pub(crate) async fn admit_project_command<C: AdmitCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<Admitted<C>, C::Error> {
    let admitted = async {
        match consume_challenge(store, envelope, &Admit(command)).await? {
            ChallengeUse::Committed(admitted) => {
                tracing::Span::current().record("outcome", "admitted");
                Ok(admitted)
            }
            ChallengeUse::ExactRetrySettled(receipt_id) => read_command_replay(
                store,
                &envelope.challenge_binding,
                &receipt_id,
                &envelope.canonical_command_bytes,
                &ReplayEffect::Query(C::SPEC.work_query),
                EffectKey::Admission,
            )
            .await
            .and_then(|replay| {
                command.check_settlement(&replay)?;
                Ok(AdmittedProjectCommand {
                    command_id: replay.ids.command_id.clone(),
                    author_command_admission_id: replay.ids.author_command_admission_id.clone(),
                    work: command.decode_work(replay.effect())?,
                    response: C::Response::replay(&replay)?,
                })
            })
            .map_err(|fault| replay_problem(fault).into()),
            ChallengeUse::ExactRetryInProgress => {
                tracing::Span::current().record("outcome", "replayed");
                read_in_progress(store, envelope, command)
                    .await
                    .map_err(|fault| replay_problem(fault).into())
            }
        }
    }
    .await;
    record_result(&admitted);
    admitted
}

/// The first use of a command that commits its Admission and work rows.
struct Admit<'c, C>(&'c C);

impl<C: AdmitCommand> FirstUse for Admit<'_, C> {
    type Output = Admitted<C>;
    type Error = C::Error;
    const RATE_LIMITED: RateLimitedChallenge = C::SPEC.rate_limited;

    async fn write(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
    ) -> Result<Admitted<C>, C::Error> {
        let command = self.0;
        let project = lock_project(client, envelope).await?;
        let facts = command.load_facts(client, envelope, &project).await?;
        if !insert_admission(client, envelope, C::SPEC.kind, &command.admission(&facts)).await? {
            return Err(ProjectCommandError::InvalidChallenge.into());
        }
        let work = command.write_work(client, envelope, facts).await?;
        let response = C::Response::record(client, envelope, C::SPEC.kind).await?;
        Ok(AdmittedProjectCommand {
            command_id: envelope.ids.command_id.clone(),
            author_command_admission_id: envelope.ids.author_command_admission_id.clone(),
            work,
            response,
        })
    }
}

/// Reads the fence, the Admission, and the work rows of an admitted command in one read-only
/// transaction. A fence that is not `in_progress` with the command digest is a binding conflict.
async fn read_in_progress<C: AdmitCommand>(
    store: &PostgresProjectReader,
    envelope: &ProjectCommandEnvelope,
    command: &C,
) -> Result<Admitted<C>, ReplayFault> {
    let scope = &envelope.project_scope;
    let challenge = &envelope.challenge_binding;
    let (fence, work) = read_snapshot(store, scope, async |client| {
        let fence = client
            .query_opt(
                "SELECT admission.command_id::text,
                        admission.author_command_admission_id::text,
                        idempotency.acknowledgement_format,
                        idempotency.response_project::text
                   FROM storyos.command_idempotency AS idempotency
              LEFT JOIN storyos.author_command_admissions AS admission
                     ON (admission.owner_user_id, admission.project_id, admission.command_kind,
                         admission.idempotency_key) =
                        (idempotency.owner_user_id, idempotency.project_id,
                         idempotency.command_kind, idempotency.idempotency_key)
                  WHERE idempotency.owner_user_id = $1::text::uuid
                    AND idempotency.project_id = $2::text::uuid
                    AND idempotency.command_kind = $3
                    AND idempotency.idempotency_key = $4::text::uuid
                    AND idempotency.outcome_kind = 'in_progress'
                    AND idempotency.canonical_command_digest = $5",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &C::SPEC.kind,
                    &challenge.idempotency_key,
                    &challenge.canonical_command_digest,
                ],
            )
            .await
            .map_err(unavailable_fault)?;
        let work = match fence
            .as_ref()
            .and_then(|fence| fence.get::<_, Option<String>>(/*idx*/ 1))
        {
            Some(admission_id) => {
                read_effect(client, C::SPEC.work_query, scope, &admission_id).await?
            }
            None => None,
        };
        Ok((fence, work))
    })
    .await?;
    let fence = fence.ok_or(ReplayFault::BindingConflict)?;
    let (Some(command_id), Some(author_command_admission_id)) = (
        fence.get::<_, Option<String>>(/*idx*/ 0),
        fence.get::<_, Option<String>>(/*idx*/ 1),
    ) else {
        return Err(unavailable_fault("an in-progress fence has no Admission"));
    };
    Ok(AdmittedProjectCommand {
        command_id,
        author_command_admission_id,
        work: command.decode_work(&EffectRecord::parse(work)?)?,
        response: C::Response::in_progress(
            fence.get::<_, Option<&str>>(/*idx*/ 2),
            fence.get::<_, Option<&str>>(/*idx*/ 3),
        )?,
    })
}

fn unavailable_fault(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> ReplayFault {
    ReplayFault::Unavailable(error.into())
}
