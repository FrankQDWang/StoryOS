//! The global Tool Registration rows (ADR 0083). The Tool Gateway quarantines a revision on Tool
//! Contract Drift (`agent_run_tool_authority.rs`) with `quarantine_revision`.

use serde_json::json;
use storyos_core::ToolRegistration;

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

/// Moves an active revision to quarantined with the stored form of its Tool Contract Drift
/// differences. A revision that is already unavailable, or an unknown revision, does not change.
/// The result is the number of changed rows: 1 or 0.
pub(crate) async fn quarantine_revision(
    client: &tokio_postgres::Client,
    revision: &str,
    differences: &serde_json::Value,
) -> Result<u64, tokio_postgres::Error> {
    client
        .execute(
            "UPDATE storyos.tool_registration_revisions
                SET lifecycle = 'quarantined', quarantine_differences = $2::text::jsonb,
                    quarantined_at = clock_timestamp()
              WHERE tool_registration_revision = $1::text::uuid AND lifecycle = 'active'",
            &[&revision, &differences.to_string()],
        )
        .await
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
