//! One replay read for the settled acknowledgement of a project command.

use std::collections::BTreeMap;

use storyos_application::{
    AuthorCommandAdmissionIds, Project, ProjectAssistanceAcknowledgement,
    ProjectCommandChallengeBinding,
};
use storyos_core::{ReasonCode, TransitionOutcome};

use crate::command_response_assistance::{
    COMMAND_RESPONSE_ASSISTANCE_FORMAT, CommandResponseAssistanceEvidence,
    read_command_response_assistance,
};
use crate::command_response_project::{
    COMMAND_RESPONSE_PROJECT_FORMAT, CommandResponseProjectEvidence, read_command_response_project,
};
use crate::command_sequence::ReplayEffect;
use crate::{PostgresProjectReader, set_challenge_scope_on_client};

/// The stored evidence of one settled command, read for an exact retry.
pub(crate) struct CommandReplay {
    pub(crate) ids: AuthorCommandAdmissionIds,
    pub(crate) receipt_created_at: String,
    pub(crate) project_activity_position: u64,
    pub(crate) project_activity_event_id: String,
    pub(crate) authority: Option<ReplayedAuthority>,
    /// The Author Action sequence text of the Receipt, also when the transition has no Commit.
    /// Only a profile that needs it parses it.
    pub(crate) author_action_sequence: Option<String>,
    /// The disposition of the Author Action of the Receipt.
    pub(crate) author_action_disposition: Option<String>,
    /// The canonical Snapshot at the Activity position of the Receipt.
    pub(crate) snapshot_id: Option<String>,
    /// The latest Manuscript Tree Revision text that an Activity payload records at or before the
    /// Receipt. Only a profile that needs it parses it.
    pub(crate) manuscript_tree_revision: Option<String>,
    /// The resulting head array of the Domain Receipt.
    pub(crate) resulting_heads: Vec<String>,
    /// Whether the Command Idempotency Fence keeps the command digest of the Receipt.
    pub(crate) fence_digest_matches: bool,
    /// Whether the Admission has the command kind, digest, and idempotency key of the Receipt and
    /// the canonical command payload of the request.
    pub(crate) admission_matches: bool,
    /// The Draft references of the Domain Receipt.
    pub(crate) draft_artifact_refs: Vec<String>,
    result_kind: String,
    receipt: JsonText,
    activity: JsonText,
    effect: JsonText,
    acknowledgement_format: Option<String>,
    response_project: Option<String>,
    response_assistance: Option<String>,
}

/// The Structural Authority Settlement records that one applied command wrote.
pub(crate) struct ReplayedAuthority {
    pub(crate) authoritative_commit_id: String,
    pub(crate) author_action_sequence: u64,
    pub(crate) snapshot_id: String,
    pub(crate) prior_manuscript_tree_revision: u64,
    pub(crate) resulting_manuscript_tree_revision: u64,
    pub(crate) resulting_revision_id: Option<String>,
}

pub(crate) enum ReplayFault {
    BindingConflict,
    HistoricalAcknowledgementUnavailable,
    Unavailable(Box<dyn std::error::Error + Send + Sync>),
}

/// The top-level fields of one stored JSON object, each as PostgreSQL `->>` text.
struct JsonText(BTreeMap<String, Option<String>>);

impl JsonText {
    fn parse(text: Option<String>) -> Result<Self, ReplayFault> {
        match text {
            Some(text) => serde_json::from_str(&text).map(Self).map_err(unavailable),
            None => Ok(Self(BTreeMap::new())),
        }
    }

    fn text(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(Option::as_deref)
    }
}

impl CommandReplay {
    /// The recorded outcome, classified from the stored Receipt result kind. `applied_result`
    /// is the result kind of the command's applied outcome.
    ///
    /// `Applied` carries no value. The command decodes its applied effect.
    pub(crate) fn outcome<N: ReasonCode, C: ReasonCode, R: ReasonCode>(
        &self,
        applied_result: &str,
    ) -> Result<TransitionOutcome<(), N, C, R>, ReplayFault> {
        if self.result_kind == applied_result {
            return Ok(TransitionOutcome::Applied(()));
        }
        self.receipt
            .text("reason")
            .and_then(|reason| {
                TransitionOutcome::from_zero_authority_codes(&self.result_kind, reason)
            })
            .ok_or(ReplayFault::BindingConflict)
    }

    pub(crate) fn receipt_text(&self, key: &str) -> Option<&str> {
        self.receipt.text(key)
    }

    pub(crate) fn activity_text(&self, key: &str) -> Result<String, ReplayFault> {
        self.activity
            .text(key)
            .map(str::to_owned)
            .ok_or(ReplayFault::BindingConflict)
    }

    pub(crate) fn activity_optional_text(&self, key: &str) -> Option<String> {
        self.activity.text(key).map(str::to_owned)
    }

    /// One field of the effect row that the command's `ReplayEffect` query reads.
    pub(crate) fn effect_text(&self, key: &str) -> Option<String> {
        self.effect.text(key).map(str::to_owned)
    }

    pub(crate) fn activity_u64(&self, key: &str) -> Result<u64, ReplayFault> {
        self.activity_text(key)?.parse().map_err(unavailable)
    }

    pub(crate) fn response_project(&self) -> Result<Project, ReplayFault> {
        match read_command_response_project(
            self.acknowledgement_format.as_deref(),
            self.response_project.as_deref(),
        ) {
            Ok(CommandResponseProjectEvidence::Captured(project)) => Ok(project),
            Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                Err(ReplayFault::HistoricalAcknowledgementUnavailable)
            }
            Err(()) => Err(unavailable(std::io::Error::other(
                "command acknowledgement evidence is damaged",
            ))),
        }
    }

    /// The Command-response Project and the assistance record of an assistance acknowledgement.
    pub(crate) fn response_project_assistance(
        &self,
    ) -> Result<ProjectAssistanceAcknowledgement, ReplayFault> {
        let damaged = || {
            unavailable(std::io::Error::other(
                "command acknowledgement evidence is damaged",
            ))
        };
        let project_format = match self.acknowledgement_format.as_deref() {
            Some(COMMAND_RESPONSE_ASSISTANCE_FORMAT) => Some(COMMAND_RESPONSE_PROJECT_FORMAT),
            other => other,
        };
        let project =
            match read_command_response_project(project_format, self.response_project.as_deref()) {
                Ok(CommandResponseProjectEvidence::Captured(project)) => project,
                Ok(CommandResponseProjectEvidence::HistoricalUnavailable) => {
                    return Err(ReplayFault::HistoricalAcknowledgementUnavailable);
                }
                Err(()) => return Err(damaged()),
            };
        let assistance = match read_command_response_assistance(
            self.acknowledgement_format.as_deref(),
            self.response_assistance.as_deref(),
        ) {
            Ok(CommandResponseAssistanceEvidence::Captured(assistance)) => assistance,
            Ok(CommandResponseAssistanceEvidence::HistoricalUnavailable) => {
                return Err(ReplayFault::HistoricalAcknowledgementUnavailable);
            }
            Err(()) => return Err(damaged()),
        };
        Ok(ProjectAssistanceAcknowledgement {
            project,
            assistance,
        })
    }
}

/// Reads the settled acknowledgement evidence of one exact retry in a read-only transaction.
pub(crate) async fn read_command_replay(
    store: &PostgresProjectReader,
    binding: &ProjectCommandChallengeBinding,
    receipt_id: &str,
    canonical_command_bytes: &[u8],
    effect: &ReplayEffect,
) -> Result<CommandReplay, ReplayFault> {
    let client = store.connect_challenge().await.map_err(unavailable)?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .await
        .map_err(unavailable)?;
    let rows = async {
        set_challenge_scope_on_client(&client, &binding.project_scope)
            .await
            .map_err(unavailable)?;
        let owner_user_id = binding.project_scope.owner_user_id.as_ref();
        let project_id = binding.project_scope.project_id.as_ref();
        let row = client
            .query_opt(
                REPLAY_SQL,
                &[
                    &owner_user_id,
                    &project_id,
                    &receipt_id,
                    &binding.command_kind,
                    &binding.canonical_command_digest,
                    &binding.idempotency_key,
                    &canonical_command_bytes,
                ],
            )
            .await
            .map_err(unavailable)?;
        let effect = match effect {
            ReplayEffect::NoQuery => None,
            ReplayEffect::Query(sql) => client
                .query_opt(*sql, &[&owner_user_id, &project_id, &receipt_id])
                .await
                .map_err(unavailable)?
                .and_then(|effect| effect.get::<_, Option<String>>(/*idx*/ 0)),
        };
        Ok((row, effect))
    }
    .await;
    match &rows {
        Ok(_) => client.batch_execute("COMMIT").await.map_err(unavailable)?,
        Err(_) => {
            let _rollback = client.batch_execute("ROLLBACK").await;
        }
    }
    let (row, effect) = rows?;
    let row = row.ok_or(ReplayFault::BindingConflict)?;
    let authority = match (
        row.get::<_, Option<String>>(9),
        row.get::<_, Option<String>>(10),
        row.get::<_, Option<String>>(11),
        row.get::<_, Option<String>>(12),
        row.get::<_, Option<String>>(13),
    ) {
        (
            Some(authoritative_commit_id),
            Some(author_action_sequence),
            Some(snapshot_id),
            Some(prior_manuscript_tree_revision),
            Some(resulting_manuscript_tree_revision),
        ) => Some(ReplayedAuthority {
            authoritative_commit_id,
            author_action_sequence: author_action_sequence.parse().map_err(unavailable)?,
            snapshot_id,
            prior_manuscript_tree_revision: prior_manuscript_tree_revision
                .parse()
                .map_err(unavailable)?,
            resulting_manuscript_tree_revision: resulting_manuscript_tree_revision
                .parse()
                .map_err(unavailable)?,
            resulting_revision_id: row.get(14),
        }),
        _ => None,
    };
    Ok(CommandReplay {
        author_action_sequence: row.get(/*idx*/ 10),
        snapshot_id: row.get(/*idx*/ 11),
        manuscript_tree_revision: row.get(/*idx*/ 17),
        ids: AuthorCommandAdmissionIds {
            command_id: row.get(0),
            author_command_admission_id: row.get(1),
            receipt_id: row.get(2),
        },
        receipt_created_at: row.get(3),
        result_kind: row.get(4),
        receipt: JsonText::parse(row.get(5))?,
        activity: JsonText::parse(row.get(6))?,
        effect: JsonText::parse(effect)?,
        project_activity_position: row
            .get::<_, Option<String>>(7)
            .unwrap_or_else(|| "0".to_owned())
            .parse()
            .map_err(unavailable)?,
        project_activity_event_id: row.get::<_, Option<String>>(8).unwrap_or_default(),
        authority,
        acknowledgement_format: row.get(15),
        response_project: row.get(16),
        response_assistance: row.get(/*idx*/ 18),
        resulting_heads: row.get(/*idx*/ 19),
        fence_digest_matches: row.get(/*idx*/ 20),
        author_action_disposition: row.get(/*idx*/ 21),
        draft_artifact_refs: row.get(/*idx*/ 22),
        admission_matches: row.get::<_, Option<bool>>(/*idx*/ 23).unwrap_or(false),
    })
}

fn unavailable(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> ReplayFault {
    ReplayFault::Unavailable(error.into())
}

const REPLAY_SQL: &str = "SELECT receipt.command_id::text,
        receipt.author_command_admission_id::text,
        receipt.receipt_id::text,
        to_char(receipt.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
        receipt.result_kind,
        (SELECT jsonb_object_agg(key, value) FROM jsonb_each_text(receipt.result_payload))::text,
        (SELECT jsonb_object_agg(key, value) FROM jsonb_each_text(payload.payload))::text,
        payload.project_activity_position::text,
        payload.project_activity_event_id::text,
        authoritative_commit.authoritative_commit_id::text,
        action.author_action_sequence::text,
        snapshot.snapshot_id::text,
        authoritative_commit.prior_manuscript_tree_revision::text,
        authoritative_commit.resulting_manuscript_tree_revision::text,
        authoritative_commit.resulting_revision_id::text,
        idempotency.acknowledgement_format,
        idempotency.response_project::text,
        (SELECT structure.payload->>'tree_revision'
           FROM storyos.project_activity_event_payloads AS structure
          WHERE (structure.owner_user_id, structure.project_id) =
                (receipt.owner_user_id, receipt.project_id)
            AND jsonb_typeof(structure.payload->'tree_revision') = 'string'
            AND (payload.project_activity_position IS NULL
                 OR structure.project_activity_position <= payload.project_activity_position)
          ORDER BY structure.project_activity_position DESC
          LIMIT 1),
        idempotency.response_assistance::text,
        receipt.resulting_heads::text[],
        idempotency.canonical_command_digest = receipt.command_digest,
        action.disposition,
        receipt.draft_artifact_refs,
        (admission.command_kind, admission.canonical_command_digest, admission.idempotency_key,
         admission.command_payload) =
        (receipt.command_kind, receipt.command_digest, receipt.idempotency_key,
         convert_from($7::bytea, 'UTF8')::jsonb)
   FROM storyos.domain_receipts AS receipt
   JOIN storyos.author_command_admission_settlements AS settlement
     ON (settlement.owner_user_id, settlement.project_id,
         settlement.author_command_admission_id, settlement.receipt_id) =
        (receipt.owner_user_id, receipt.project_id,
         receipt.author_command_admission_id, receipt.receipt_id)
   JOIN storyos.command_idempotency AS idempotency
     ON (idempotency.owner_user_id, idempotency.project_id,
         idempotency.command_kind, idempotency.idempotency_key,
         idempotency.result_reference) =
        (receipt.owner_user_id, receipt.project_id, receipt.command_kind,
         receipt.idempotency_key, receipt.receipt_id::text)
LEFT JOIN storyos.author_command_admissions AS admission
     ON (admission.owner_user_id, admission.project_id,
         admission.author_command_admission_id, admission.command_id) =
        (receipt.owner_user_id, receipt.project_id,
         receipt.author_command_admission_id, receipt.command_id)
LEFT JOIN storyos.project_activity_event_payloads AS payload
     ON (payload.owner_user_id, payload.project_id, payload.receipt_id) =
        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
LEFT JOIN storyos.authoritative_commits AS authoritative_commit
     ON (authoritative_commit.owner_user_id, authoritative_commit.project_id,
         authoritative_commit.receipt_id) =
        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
LEFT JOIN storyos.author_action_entries AS action
     ON (action.owner_user_id, action.project_id, action.receipt_id) =
        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
LEFT JOIN LATERAL (
         SELECT canonical.snapshot_id
           FROM storyos.project_snapshots AS canonical
          WHERE (canonical.owner_user_id, canonical.project_id,
                 canonical.project_activity_position) =
                (payload.owner_user_id, payload.project_id, payload.project_activity_position)
            AND canonical.snapshot_kind = 'canonical'
          ORDER BY canonical.created_at, canonical.snapshot_id
          LIMIT 1
       ) AS snapshot ON true
  WHERE receipt.owner_user_id = $1::text::uuid
    AND receipt.project_id = $2::text::uuid
    AND receipt.receipt_id = $3::text::uuid
    AND receipt.command_kind = $4
    AND receipt.command_digest = $5
    AND receipt.idempotency_key = $6::text::uuid
    AND settlement.settlement_kind = 'receipt_settled'
    AND idempotency.outcome_kind = 'settled'";
