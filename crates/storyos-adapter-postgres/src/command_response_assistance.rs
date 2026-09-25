use storyos_application::ProjectAssistanceRecord;

use crate::command_response_project::COMMAND_RESPONSE_PROJECT_FORMAT;
use crate::update_project_assistance::{availability_text, parse_availability, parse_u64};

pub(crate) const COMMAND_RESPONSE_ASSISTANCE_FORMAT: &str =
    "command_response_project_assistance.v1";

pub(crate) enum CommandResponseAssistanceEvidence {
    HistoricalUnavailable,
    Captured(Option<ProjectAssistanceRecord>),
}

pub(crate) fn encode_command_response_assistance(
    assistance: Option<&ProjectAssistanceRecord>,
) -> String {
    match assistance {
        None => "null".to_owned(),
        Some(record) => serde_json::json!({
            "availability": availability_text(record.availability),
            "revision": record.revision.to_string(),
            "model_registration_revision": record.model_registration_revision,
            "processing_destination_identity": record.processing_destination_identity,
            "processing_destination_identity_evidence_revision": record.processing_destination_identity_evidence_revision.to_string(),
            "project_model_use_binding_revision": record.project_model_use_binding_revision,
            "grant_id": record.grant_id,
            "external_compatibility_decision": record.external_compatibility_decision,
        })
        .to_string(),
    }
}

pub(crate) fn read_command_response_assistance(
    format: Option<&str>,
    payload: Option<&str>,
) -> Result<CommandResponseAssistanceEvidence, ()> {
    match (format, payload) {
        (None | Some(COMMAND_RESPONSE_PROJECT_FORMAT), None) => {
            Ok(CommandResponseAssistanceEvidence::HistoricalUnavailable)
        }
        (Some(COMMAND_RESPONSE_PROJECT_FORMAT), Some(_)) => {
            Ok(CommandResponseAssistanceEvidence::HistoricalUnavailable)
        }
        (Some(COMMAND_RESPONSE_ASSISTANCE_FORMAT), Some("null")) => {
            Ok(CommandResponseAssistanceEvidence::Captured(None))
        }
        (Some(COMMAND_RESPONSE_ASSISTANCE_FORMAT), Some(payload)) => {
            let value: serde_json::Value = serde_json::from_str(payload).map_err(|_| ())?;
            let object = value.as_object().ok_or(())?;
            if object.len() != 8 {
                return Err(());
            }
            let field = |name| {
                object
                    .get(name)
                    .and_then(serde_json::Value::as_str)
                    .ok_or(())
            };
            Ok(CommandResponseAssistanceEvidence::Captured(Some(
                ProjectAssistanceRecord {
                    availability: parse_availability(field("availability")?).map_err(|_| ())?,
                    revision: parse_u64(field("revision")?.to_owned()).map_err(|_| ())?,
                    model_registration_revision: field("model_registration_revision")?.to_owned(),
                    processing_destination_identity: field("processing_destination_identity")?
                        .to_owned(),
                    processing_destination_identity_evidence_revision: parse_u64(
                        field("processing_destination_identity_evidence_revision")?.to_owned(),
                    )
                    .map_err(|_| ())?,
                    project_model_use_binding_revision: field(
                        "project_model_use_binding_revision",
                    )?
                    .to_owned(),
                    grant_id: field("grant_id")?.to_owned(),
                    external_compatibility_decision: field("external_compatibility_decision")?
                        .to_owned(),
                },
            )))
        }
        _ => Err(()),
    }
}
