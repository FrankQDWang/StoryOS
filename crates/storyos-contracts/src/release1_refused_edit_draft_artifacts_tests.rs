use serde_json::{Value, json};

#[test]
fn refused_edit_query_and_creation_reject_incomplete_or_operational_content() {
    let files = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let query_schema: Value = serde_json::from_slice(
        &files[crate::release1_refused_edit_draft_artifacts::RESPONSE_SCHEMA_PATH],
    )
    .unwrap();
    let event_schema: Value = serde_json::from_slice(
        &files[crate::release1_refused_edit_draft_artifacts::EVENT_SCHEMA_PATH],
    )
    .unwrap();
    let query: Value = serde_json::from_slice(
        &files[crate::release1_refused_edit_draft_artifacts::FIXTURE_PATHS[0]],
    )
    .unwrap();
    let event: Value = serde_json::from_slice(
        &files[crate::release1_refused_edit_draft_artifacts::EVENT_FIXTURE_PATHS[0]],
    )
    .unwrap();
    let query_validator = jsonschema::validator_for(&query_schema).unwrap();
    let event_validator = jsonschema::validator_for(&event_schema).unwrap();
    assert!(query_validator.is_valid(&query));
    assert!(event_validator.is_valid(&event));
    assert!(serde_json::from_value::<crate::GetRefusedEditDraftResponse>(query.clone()).is_ok());
    assert!(serde_json::from_value::<crate::RefusedEditDraftCreated>(event.clone()).is_ok());

    let mut missing_source = query.clone();
    missing_source["draft"]["payload"]["author_edit_units"][0]["selection_snapshot"]["ordered_selection"]["sources"][1]["owner"].as_object_mut().unwrap().remove("revision_id");
    let mut transport_content = query;
    transport_content["draft"]["payload"]["anti_forgery_nonce"] = json!("transport-secret");
    for invalid in [missing_source, transport_content] {
        assert!(!query_validator.is_valid(&invalid));
        assert!(serde_json::from_value::<crate::GetRefusedEditDraftResponse>(invalid).is_err());
    }

    let mut wrong_creator = event.clone();
    wrong_creator["creator"] =
        json!({"kind": "author", "receipt_id": "018f0000-0000-7001-8000-000000000b02"});
    let mut incomplete_creation = event;
    incomplete_creation["source"]
        .as_object_mut()
        .unwrap()
        .remove("author_command_admission_id");
    for invalid in [wrong_creator, incomplete_creation] {
        assert!(!event_validator.is_valid(&invalid));
        assert!(serde_json::from_value::<crate::RefusedEditDraftCreated>(invalid).is_err());
    }
}
