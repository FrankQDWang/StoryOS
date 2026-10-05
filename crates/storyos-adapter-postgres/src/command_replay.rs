//! One replay read for the settled acknowledgement of a project command.

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
use uuid::Uuid;

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
    /// The JSON text of the `tree_revision` value of the newest Activity payload that records one
    /// at or before the Receipt. Only a profile that needs it reads it.
    manuscript_tree_revision: Option<String>,
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
    receipt: JsonFields,
    activity: JsonFields,
    effect: JsonFields,
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

/// The Proposal State Axis of one preserved value that replay reads.
#[derive(Clone, Copy)]
pub(crate) enum StateAxis {
    Generation,
    Validation,
    Closure,
    OperationResolution,
}

impl StateAxis {
    /// Returns `value` when this axis has it. Another value is damaged evidence.
    pub(crate) fn preserved(self, value: String) -> Result<String, ReplayFault> {
        let known: &[&str] = match self {
            Self::Generation => &["generating", "ready_partial", "ready"],
            Self::Validation => &["pending", "valid", "invalid", "conflicted"],
            Self::Closure => &["open", "withdrawn", "superseded"],
            Self::OperationResolution => &["pending", "applied", "rejected"],
        };
        if known.contains(&value.as_str()) {
            return Ok(value);
        }
        Err(unavailable(format!(
            "the preserved Proposal state {value} is unknown"
        )))
    }
}

/// The top-level fields of one stored JSON object, with their JSON types.
struct JsonFields(serde_json::Map<String, serde_json::Value>);

impl JsonFields {
    fn parse(text: Option<String>) -> Result<Self, ReplayFault> {
        let Some(text) = text else {
            return Ok(Self(serde_json::Map::new()));
        };
        match serde_json::from_str(&text).map_err(unavailable)? {
            serde_json::Value::Object(fields) => Ok(Self(fields)),
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_)
            | serde_json::Value::Array(_) => {
                Err(unavailable("the stored payload is not a JSON object"))
            }
        }
    }

    /// The field of `key` when it is absent, null, or a string. Another JSON type is damaged
    /// evidence.
    fn field(&self, key: &str) -> Result<Field<'_>, ReplayFault> {
        match self.0.get(key) {
            None => Ok(Field::Absent),
            Some(serde_json::Value::Null) => Ok(Field::Null),
            Some(serde_json::Value::String(text)) => Ok(Field::Text(text)),
            Some(
                serde_json::Value::Bool(_)
                | serde_json::Value::Number(_)
                | serde_json::Value::Array(_)
                | serde_json::Value::Object(_),
            ) => Err(unavailable(format!(
                "the stored field {key} is not a string"
            ))),
        }
    }

    /// The string of `key`. An absent or null field is damaged evidence.
    fn required(&self, key: &str) -> Result<&str, ReplayFault> {
        match self.field(key)? {
            Field::Text(text) => Ok(text),
            Field::Absent | Field::Null => {
                Err(unavailable(format!("the stored field {key} is missing")))
            }
        }
    }

    /// The string of `key`, or `None` when it is null. An absent field is damaged evidence.
    fn nullable(&self, key: &str) -> Result<Option<&str>, ReplayFault> {
        match self.field(key)? {
            Field::Text(text) => Ok(Some(text)),
            Field::Null => Ok(None),
            Field::Absent => Err(unavailable(format!("the stored field {key} is missing"))),
        }
    }
}

/// One top-level field of a stored JSON object whose JSON type replay accepts.
enum Field<'a> {
    Absent,
    Null,
    Text(&'a str),
}

/// The stored text of the UUID field `key`, unchanged. A text that is not a UUID is damaged
/// evidence. A command can store the client text of a UUID, which can be uppercase.
fn uuid_text(key: &str, text: &str) -> Result<String, ReplayFault> {
    Uuid::parse_str(text)
        .map(|_| text.to_owned())
        .map_err(|_| unavailable(format!("the stored field {key} is not a UUID")))
}

/// The unsigned decimal value of the stored field `key`. Another text is damaged evidence.
fn decimal(key: &str, text: &str) -> Result<u64, ReplayFault> {
    text.parse::<u64>()
        .ok()
        .filter(|value| value.to_string() == text)
        .ok_or_else(|| unavailable(format!("the stored field {key} is not a decimal")))
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
        // A missing or unknown reason is a binding conflict (ADR 0043).
        match self.receipt.field("reason")? {
            Field::Text(reason) => {
                TransitionOutcome::from_zero_authority_codes(&self.result_kind, reason)
            }
            Field::Absent | Field::Null => None,
        }
        .ok_or(ReplayFault::BindingConflict)
    }

    /// One required string field of the Receipt payload.
    pub(crate) fn receipt_text(&self, key: &str) -> Result<&str, ReplayFault> {
        self.receipt.required(key)
    }

    /// One Receipt payload field that a historical format leaves out. It is `None` only when it
    /// is absent.
    pub(crate) fn receipt_historical_decimal(&self, key: &str) -> Result<Option<u64>, ReplayFault> {
        match self.receipt.field(key)? {
            Field::Absent => Ok(None),
            Field::Text(text) => decimal(key, text).map(Some),
            Field::Null => Err(unavailable(format!("the stored field {key} is null"))),
        }
    }

    /// The UUID text of one Receipt payload field, or `None` when it is null.
    pub(crate) fn receipt_nullable_uuid(&self, key: &str) -> Result<Option<String>, ReplayFault> {
        self.receipt
            .nullable(key)?
            .map(|text| uuid_text(key, text))
            .transpose()
    }

    /// The UUID text of one required Receipt payload field.
    pub(crate) fn receipt_uuid(&self, key: &str) -> Result<String, ReplayFault> {
        uuid_text(key, self.receipt.required(key)?)
    }

    /// Requires the Receipt payload field `key` to be present and null.
    pub(crate) fn require_receipt_null(&self, key: &str) -> Result<(), ReplayFault> {
        match self.receipt.field(key)? {
            Field::Null => Ok(()),
            Field::Absent | Field::Text(_) => {
                Err(unavailable(format!("the stored field {key} is not null")))
            }
        }
    }

    /// Requires `expected` as the Receipt payload value of `key`. Another value is damaged evidence.
    pub(crate) fn require_receipt_text(
        &self,
        key: &str,
        expected: &str,
    ) -> Result<(), ReplayFault> {
        if self.receipt.required(key)? == expected {
            return Ok(());
        }
        Err(unavailable(format!(
            "the applied Receipt payload has no {key} {expected}"
        )))
    }

    /// One required string field of the Activity payload.
    pub(crate) fn activity_text(&self, key: &str) -> Result<String, ReplayFault> {
        self.activity.required(key).map(str::to_owned)
    }

    /// The UUID text of one required Activity payload field.
    pub(crate) fn activity_uuid(&self, key: &str) -> Result<String, ReplayFault> {
        uuid_text(key, self.activity.required(key)?)
    }

    /// The UUID text of one Activity payload field, or `None` when it is null.
    pub(crate) fn activity_nullable_uuid(&self, key: &str) -> Result<Option<String>, ReplayFault> {
        self.activity
            .nullable(key)?
            .map(|text| uuid_text(key, text))
            .transpose()
    }

    /// The UUID text of one Activity payload field, or `None` when it is null or when a
    /// historical format leaves it out.
    pub(crate) fn activity_historical_uuid(
        &self,
        key: &str,
    ) -> Result<Option<String>, ReplayFault> {
        match self.activity.field(key)? {
            Field::Text(text) => uuid_text(key, text).map(Some),
            Field::Absent | Field::Null => Ok(None),
        }
    }

    /// The unsigned decimal value of one required Activity payload field.
    pub(crate) fn activity_u64(&self, key: &str) -> Result<u64, ReplayFault> {
        decimal(key, self.activity.required(key)?)
    }

    /// One string field of the effect row that the command's `ReplayEffect` query reads, or
    /// `None` when the row or its column is absent.
    pub(crate) fn effect_text(&self, key: &str) -> Result<Option<String>, ReplayFault> {
        Ok(match self.effect.field(key)? {
            Field::Text(text) => Some(text.to_owned()),
            Field::Absent | Field::Null => None,
        })
    }

    /// The Manuscript Tree Revision of the newest Activity payload that records one at or before
    /// the Receipt, or `None` when no payload records one.
    pub(crate) fn manuscript_tree_revision(&self) -> Result<Option<u64>, ReplayFault> {
        self.manuscript_tree_revision
            .as_deref()
            .map(
                |json| match serde_json::from_str(json).map_err(unavailable)? {
                    serde_json::Value::String(text) => decimal("tree_revision", &text),
                    serde_json::Value::Null
                    | serde_json::Value::Bool(_)
                    | serde_json::Value::Number(_)
                    | serde_json::Value::Array(_)
                    | serde_json::Value::Object(_) => Err(unavailable(
                        "the stored field tree_revision is not a string",
                    )),
                },
            )
            .transpose()
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
        receipt: JsonFields::parse(row.get(/*idx*/ 5))?,
        activity: JsonFields::parse(row.get(/*idx*/ 6))?,
        effect: JsonFields::parse(effect)?,
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
        admission_matches: row.get::<_, Option<bool>>(/*idx*/ 23).unwrap_or_default(),
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
        receipt.result_payload::text,
        payload.payload::text,
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
        (SELECT (structure.payload->'tree_revision')::text
           FROM storyos.project_activity_event_payloads AS structure
          WHERE (structure.owner_user_id, structure.project_id) =
                (receipt.owner_user_id, receipt.project_id)
            AND structure.payload ? 'tree_revision'
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
