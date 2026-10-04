#[test]
fn agent_run_query_rejects_malformed_captured_settings_revision() {
    let generated = super::generated_files()
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    let schema: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_agent_run_artifacts::GET_RESPONSE_SCHEMA_PATH],
    )
    .expect("AgentRun response schema must be JSON");
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("AgentRun response schema must compile");
    let mut response: serde_json::Value = serde_json::from_slice(
        &generated[crate::release1_agent_run_artifacts::GET_FIXTURE_PATHS[0]],
    )
    .expect("positive AgentRun response must be JSON");
    assert!(validator.is_valid(&response));
    response["captured_memory_settings"] = serde_json::json!({
        "kind": "available",
        "memory_settings_revision": response["memory_settings_revision"],
        "use_enabled": true,
        "contribution_enabled": false
    });
    assert!(validator.is_valid(&response));
    response["context"]["passage_targets"] = serde_json::json!([{
        "chapter_id": response["run_id"],
        "base_authoritative_revision_id": response["run_id"],
        "manuscript_block_ids": [response["run_id"]]
    }]);
    assert!(validator.is_valid(&response));
    for path in [
        "/context/passage_targets/0/chapter_id",
        "/context/passage_targets/0/base_authoritative_revision_id",
        "/context/passage_targets/0/manuscript_block_ids/0",
    ] {
        let mut malformed = response.clone();
        *malformed.pointer_mut(path).expect("target ID exists") = serde_json::json!("not-a-uuid");
        assert!(!validator.is_valid(&malformed));
    }
    response["decision"] = serde_json::json!({
        "kind": "prose_change", "decision_id": response["run_id"],
        "opened_proposal": {"kind": "absent"}, "producer_input": "new candidate", "selected": true,
        "text": "new candidate", "continuation": {"kind": "absent"}, "authoritative": false,
        "locations": [{"chapter_id": response["run_id"], "manuscript_block_id": response["run_id"],
          "base_authoritative_revision_id": response["run_id"], "candidate_text": "new candidate",
          "explanation": "Revise the selected candidate.", "current": null,
          "outcome": {"kind": "revised", "proposal_id": response["run_id"], "operation_id": response["run_id"],
            "revision_id": response["run_id"], "prior_revision_id": response["run_id"], "validation_receipt_id": response["run_id"]}}]
    });
    assert!(validator.is_valid(&response));
    response["decision"]["locations"][0]["outcome"]["prior_revision_id"] =
        serde_json::json!("not-a-uuid");
    assert!(!validator.is_valid(&response));
    response["decision"]["locations"][0]["outcome"]["prior_revision_id"] =
        response["run_id"].clone();
    response["captured_memory_settings"]["memory_settings_revision"] =
        serde_json::json!("not-a-uuid");
    assert!(!validator.is_valid(&response));
}
