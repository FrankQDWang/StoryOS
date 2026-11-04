//! The Tool Contract Drift check of a fresh Tool Discovery Record against a pinned Tool
//! Registration revision. The Worker runs it at Project enablement and before each dispatch.

use serde_json::json;
use storyos_core::{ContractCheck, ContractDifference, ToolRegistration};

use crate::ToolDiscoveryRecord;
use crate::record::digest;

type Difference = fn(String, String) -> ContractDifference;

impl ToolDiscoveryRecord {
    /// Compares this record with a pinned revision. A tool that the Registration does not list
    /// stays uncallable and is not a difference.
    pub fn check(&self, registration: &ToolRegistration) -> ContractCheck {
        let mut differences = Vec::new();
        let identities: [(&str, &str, Difference); 3] = [
            (
                registration.server_url,
                &self.server_url,
                |pinned, observed| ContractDifference::ServerUrl { pinned, observed },
            ),
            (
                registration.server_name,
                &self.server_name,
                |pinned, observed| ContractDifference::ServerName { pinned, observed },
            ),
            (
                registration.protocol_revision,
                &self.protocol_revision,
                |pinned, observed| ContractDifference::ProtocolRevision { pinned, observed },
            ),
        ];
        for (pinned, observed, difference) in identities {
            if pinned != observed {
                differences.push(difference(pinned.to_owned(), observed.to_owned()));
            }
        }
        let mut provenance_changed = registration.server_version != self.server_version;
        for pinned in registration.tools {
            let tool = pinned.name.to_owned();
            let Some(observed) = self.tools.iter().find(|known| known.name == pinned.name) else {
                differences.push(ContractDifference::ToolRemoved { tool });
                continue;
            };
            if observed.input_schema_digest != pinned.input_schema_digest {
                differences.push(ContractDifference::InputSchema {
                    tool: tool.clone(),
                    pinned: pinned.input_schema_digest.to_owned(),
                    observed: observed.input_schema_digest.clone(),
                });
            }
            let output = observed.output_schema.as_ref().map(digest);
            if output.as_deref() != pinned.output_schema_digest {
                differences.push(ContractDifference::OutputSchema {
                    tool,
                    pinned: pinned.output_schema_digest.map(str::to_owned),
                    observed: output,
                });
            }
            let provenance = json!({
                "annotations": observed.annotations,
                "description": observed.description,
            });
            provenance_changed |= digest(&provenance) != pinned.provenance_digest;
        }
        if !differences.is_empty() {
            ContractCheck::Drift(differences)
        } else if provenance_changed {
            ContractCheck::ProvenanceChange
        } else {
            ContractCheck::Unchanged
        }
    }
}

#[cfg(test)]
#[path = "contract_check_tests.rs"]
mod tests;
