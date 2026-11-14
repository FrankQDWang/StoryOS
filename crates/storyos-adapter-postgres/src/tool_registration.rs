//! The global Tool Registration rows (ADR 0083) and the quarantine of a revision on Tool Contract
//! Drift.

use serde_json::json;
use storyos_core::{ContractDifference, ToolRegistration, ToolRegistrationLifecycle};

use crate::PostgresProjectReader;

type ToolRegistrationError = Box<dyn std::error::Error + Send + Sync>;

/// The result of one quarantine request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuarantineOutcome {
    /// This request moved the active revision to quarantined and wrote its Activity record.
    Quarantined,
    /// The revision was already quarantined or retired. Its record does not change.
    AlreadyUnavailable,
    UnknownRevision,
}

impl PostgresProjectReader {
    /// Inserts one global Tool Registration revision as active with its approved tools, if the
    /// revision is absent. A stored revision keeps its lifecycle.
    pub async fn insert_tool_registration(
        &self,
        registration: &ToolRegistration,
    ) -> Result<(), ToolRegistrationError> {
        let client = self.connect().await?;
        client.batch_execute("BEGIN").await?;
        match insert_registration_rows(&client, registration).await {
            Ok(()) => client.batch_execute("COMMIT").await?,
            Err(error) => {
                let _rollback = client.batch_execute("ROLLBACK").await;
                return Err(error.into());
            }
        }
        Ok(())
    }

    /// The lifecycle of a stored revision. `None` is an unknown revision.
    pub async fn tool_registration_lifecycle(
        &self,
        revision: &str,
    ) -> Result<Option<ToolRegistrationLifecycle>, ToolRegistrationError> {
        let client = self.connect().await?;
        let row = client
            .query_opt(
                "SELECT lifecycle FROM storyos.tool_registration_revisions
                  WHERE tool_registration_revision = $1::text::uuid",
                &[&revision],
            )
            .await?;
        row.map(|row| {
            ToolRegistrationLifecycle::parse(row.get(/*idx*/ 0))
                .ok_or_else(|| "the Tool Registration lifecycle is damaged".into())
        })
        .transpose()
    }

    /// Moves an active revision to quarantined. The same row update writes the one Activity
    /// record of the quarantine: the differences and the time. Exposure that derives from the
    /// revision clears, because only an active revision derives Exposure.
    pub async fn quarantine_tool_registration(
        &self,
        revision: &str,
        differences: &[ContractDifference],
    ) -> Result<QuarantineOutcome, ToolRegistrationError> {
        let client = self.connect().await?;
        let row = client
            .query_one(
                "WITH changed AS (
                   UPDATE storyos.tool_registration_revisions
                      SET lifecycle = 'quarantined',
                          quarantine_differences = $2::text::jsonb,
                          quarantined_at = clock_timestamp()
                    WHERE tool_registration_revision = $1::text::uuid AND lifecycle = 'active'
                    RETURNING 1)
                 SELECT EXISTS (SELECT 1 FROM changed),
                        EXISTS (SELECT 1 FROM storyos.tool_registration_revisions
                                 WHERE tool_registration_revision = $1::text::uuid)",
                &[&revision, &serde_json::to_string(differences)?],
            )
            .await?;
        Ok(match (row.get(/*idx*/ 0), row.get(/*idx*/ 1)) {
            (true, _) => QuarantineOutcome::Quarantined,
            (false, true) => QuarantineOutcome::AlreadyUnavailable,
            (false, false) => QuarantineOutcome::UnknownRevision,
        })
    }
}

/// Inserts the revision, and its tools only when this transaction inserted the revision.
pub(crate) async fn insert_registration_rows(
    client: &tokio_postgres::Client,
    registration: &ToolRegistration,
) -> Result<(), tokio_postgres::Error> {
    let tools: Vec<_> = registration
        .tools
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "spec": tool.spec.revision,
                "input": tool.input_schema_digest,
                "contract": tool.input_contract_digest,
                "output": tool.output_schema_digest,
                "provenance": tool.provenance_digest,
            })
        })
        .collect();
    let inserted = client
        .execute(
            "INSERT INTO storyos.tool_registration_revisions
                   (tool_registration_revision, registration_kind, server_url, server_name,
                    protocol_revision, adapter_revision)
                 VALUES ($1::text::uuid, $2, $3, $4, $5, $6)
                 ON CONFLICT (tool_registration_revision) DO NOTHING",
            &[
                &registration.revision,
                &registration.kind.as_str(),
                &registration.server_url,
                &registration.server_name,
                &registration.protocol_revision,
                &registration.adapter_revision,
            ],
        )
        .await?;
    if inserted == 1 {
        client
            .execute(
                "INSERT INTO storyos.tool_registration_tools
                       (tool_registration_revision, tool_name, tool_spec_revision,
                        input_schema_digest, input_contract_digest, output_schema_digest,
                        provenance_digest)
                     SELECT $1::text::uuid, tool.name, tool.spec, tool.input, tool.contract,
                            tool.output, tool.provenance
                       FROM jsonb_to_recordset($2::text::jsonb)
                              AS tool(name text, spec text, input text, contract text,
                                      output text, provenance text)",
                &[
                    &registration.revision,
                    &serde_json::Value::from(tools).to_string(),
                ],
            )
            .await?;
    }
    Ok(())
}

/// Inserts the global row of a Host Tool Registration, if it is absent. It has no server and no
/// pinned server contract, so it has no tool rows.
pub(crate) async fn insert_host_registration_row(
    client: &tokio_postgres::Client,
    registration: &storyos_core::HostToolRegistration,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "INSERT INTO storyos.tool_registration_revisions
                   (tool_registration_revision, registration_kind, adapter_revision)
                 VALUES ($1::text::uuid, $2, $3)
                 ON CONFLICT (tool_registration_revision) DO NOTHING",
            &[
                &registration.revision,
                &registration.kind.as_str(),
                &registration.adapter_revision,
            ],
        )
        .await?;
    Ok(())
}

#[cfg(test)]
#[path = "tool_registration_tests.rs"]
mod tests;
