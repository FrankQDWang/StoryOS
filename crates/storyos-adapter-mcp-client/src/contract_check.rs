//! The Tool Contract Drift check of a fresh Tool Discovery Record against a pinned Tool
//! Registration revision. The Worker runs it at Project enablement and before each dispatch.

use serde_json::{Value, json};
use storyos_core::{ContractCheck, ContractDifference, ToolRegistration};

use crate::ToolDiscoveryRecord;
use crate::record::digest;

type Difference = fn(String, String) -> ContractDifference;

impl ToolDiscoveryRecord {
    /// Compares this record with a pinned revision. A tool that the Registration does not list
    /// stays uncallable and is not a difference. A record of another protocol revision has no
    /// tool list, so only its identity differences are named.
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
        let compared = if registration.protocol_revision == self.protocol_revision {
            registration.tools
        } else {
            &[]
        };
        for pinned in compared {
            let tool = pinned.name.to_owned();
            let Some(observed) = self.tools.iter().find(|known| known.name == pinned.name) else {
                differences.push(ContractDifference::ToolRemoved { tool });
                continue;
            };
            if observed.input_schema_digest != pinned.input_schema_digest {
                let contract = digest(&without_descriptions(&observed.input_schema));
                if contract == pinned.input_contract_digest {
                    provenance_changed = true;
                } else {
                    differences.push(ContractDifference::InputSchema {
                        tool: tool.clone(),
                        pinned: pinned.input_contract_digest.to_owned(),
                        observed: contract,
                    });
                }
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

/// A schema without its `description` keywords. The keys of a property map are names, and an
/// `enum`, `const`, `default`, or `examples` value is data, so the function keeps them.
fn without_descriptions(schema: &Value) -> Value {
    match schema {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .filter(|(key, value)| !(key.as_str() == "description" && value.is_string()))
                .map(|(key, value)| {
                    let value = match (key.as_str(), value) {
                        (
                            "properties" | "patternProperties" | "definitions" | "$defs",
                            Value::Object(named),
                        ) => Value::Object(
                            named
                                .iter()
                                .map(|(name, child)| (name.clone(), without_descriptions(child)))
                                .collect(),
                        ),
                        ("enum" | "const" | "default" | "examples", data) => data.clone(),
                        (_, child) => without_descriptions(child),
                    };
                    (key.clone(), value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_descriptions).collect()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => schema.clone(),
    }
}

#[cfg(test)]
#[path = "contract_check_tests.rs"]
mod tests;
