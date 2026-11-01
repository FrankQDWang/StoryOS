//! The rules of the old route catalog verifier that apply to one named operation, cause, or Event.

use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::tests::{array, assert_unique, catalog, text};

fn operation<'a>(catalog: &'a Value, operation_id: &str) -> &'a Value {
    let mut matches = array(&catalog["operations"])
        .iter()
        .filter(|operation| operation["operation_id"] == operation_id);
    let operation = matches
        .next()
        .unwrap_or_else(|| panic!("catalog must contain {operation_id}"));
    assert!(
        matches.next().is_none(),
        "catalog must contain one {operation_id}"
    );
    operation
}

fn owners(catalog: &Value) -> BTreeSet<&str> {
    catalog["owners"]
        .as_object()
        .expect("owners must be an object")
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn operations_use_known_owners_and_complete_route_rules() {
    let catalog = catalog();
    let owners = owners(&catalog);
    for operation in array(&catalog["operations"]) {
        let id = text(&operation["operation_id"]);
        assert!(
            matches!(
                text(&operation["method"]),
                "GET" | "POST" | "PATCH" | "PUT" | "DELETE"
            ),
            "{id}"
        );
        let path = text(&operation["path"]);
        assert!(
            path.starts_with("/api/v1/") && !path.contains('?'),
            "{id}: {path}"
        );
        let schemas = &operation["schemas"];
        assert_ne!(schemas["request"], schemas["response"], "{id}");
        for owner in [
            &operation["semantic_owner"],
            &operation["wire_owner"],
            &schemas["request_source"],
            &schemas["response_source"],
        ] {
            assert!(owners.contains(text(owner)), "{id}: {owner}");
        }
        assert_eq!(
            schemas["semantic_source"], operation["semantic_owner"],
            "{id}"
        );
        assert!(!array(&operation["preconditions"]).is_empty(), "{id}");
        assert_eq!(operation["activity"]["pre_admission"], "none", "{id}");
        if let Some(actions) = operation.get("action_coverage") {
            assert!(!array(actions).is_empty(), "{id}");
            assert_unique(id, array(actions).iter().map(text));
        }
    }
    for id in ["updateVolume", "updateChapter"] {
        assert_eq!(
            operation(&catalog, id)["action_coverage"],
            json!(["rename", "reorder"]),
            "{id}"
        );
    }
}

#[test]
fn every_event_has_an_owner_a_profile_and_an_operation_mapping() {
    let catalog = catalog();
    let owners = owners(&catalog);
    let events = array(&catalog["events"]);
    let mapped = array(&catalog["operations"])
        .iter()
        .flat_map(|operation| {
            array(&operation["activity"]["success"])
                .iter()
                .chain(array(&operation["activity"]["no_effect"]))
        })
        .map(text)
        .collect::<BTreeSet<_>>();
    let schema_ids = events
        .iter()
        .map(|event| text(&event["schema_id"]))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        mapped, schema_ids,
        "each Event and each Activity reference must match"
    );
    for event in events {
        let id = text(&event["schema_id"]);
        assert!(id.starts_with("storyos.event."), "{id}");
        assert_eq!(event["wire_profile"], "storyos.project-activity.v1", "{id}");
        assert!(owners.contains(text(&event["semantic_owner"])), "{id}");
        assert!(!text(&event["event_kind"]).is_empty(), "{id}");
    }
    let non_public_operations = array(&catalog["non_public_operations"]);
    assert_unique(
        "non-public operation",
        non_public_operations
            .iter()
            .map(|operation| text(&operation["name"])),
    );
    for operation in non_public_operations {
        assert!(operation["public_route"].is_null(), "{}", operation["name"]);
        assert!(
            owners.contains(text(&operation["source"])),
            "{}",
            operation["name"]
        );
    }
    for event in array(&catalog["non_public_events"]) {
        assert!(event["public_schema"].is_null(), "{}", event["name"]);
        assert!(owners.contains(text(&event["source"])), "{}", event["name"]);
    }
}

#[test]
fn conditional_commands_bind_the_exact_author_and_producer_causes() {
    let catalog = catalog();
    let owners = owners(&catalog);
    let conditional = array(&catalog["operations"])
        .iter()
        .filter(|operation| operation.get("cause_profiles").is_some())
        .collect::<Vec<_>>();
    let ids = conditional
        .iter()
        .map(|operation| text(&operation["operation_id"]))
        .collect::<Vec<_>>();
    assert_eq!(ids, ["withdrawProposal", "replanProposal"]);
    for operation in conditional {
        let id = text(&operation["operation_id"]);
        assert_eq!(
            operation["cause_discriminator"],
            json!({
                "field": "cause",
                "values": ["author", "current_producer"],
                "semantic_owner": operation["semantic_owner"],
            }),
            "{id}"
        );
        let causes = operation["cause_profiles"]
            .as_object()
            .expect("cause profiles must be an object");
        assert_eq!(
            causes.keys().map(String::as_str).collect::<Vec<_>>(),
            ["author", "current_producer"],
            "{id}"
        );
        for (name, cause) in causes {
            assert_ne!(
                cause["request_schema"], cause["response_schema"],
                "{id}.{name}"
            );
            assert!(
                owners.contains(text(&cause["request_schema_source"])),
                "{id}.{name}"
            );
            assert!(
                owners.contains(text(&cause["response_schema_source"])),
                "{id}.{name}"
            );
            let settlement = &cause["settlement"];
            assert_eq!(
                settlement["http_statuses"], operation["settlement"]["http_statuses"],
                "{id}.{name}"
            );
            assert_eq!(settlement["mode"], "sync", "{id}.{name}");
            assert!(settlement["settlement_query"].is_null(), "{id}.{name}");
            assert!(settlement["operation_ref"].is_null(), "{id}.{name}");
        }
        let author = &causes["author"];
        assert_eq!(
            (
                &author["cause_binding"],
                &author["action_class"],
                &author["admission"],
                &author["idempotency"],
            ),
            (
                &json!("protected_author_command_admission"),
                &json!("explicit_editor_command"),
                &json!("author_command"),
                &json!("header_and_admission_record"),
            ),
            "{id}"
        );
        assert_eq!(
            author["editor"],
            json!({"session": "required", "writer_generation": "current", "takeover": "no"}),
            "{id}"
        );
        assert_eq!(
            author["recovery"]["requires_reconfirmation"], "allowed",
            "{id}"
        );
        assert_eq!(
            author["settlement"]["acknowledgements"],
            json!(["committed", "requires_reconfirmation"]),
            "{id}"
        );
        let producer = &causes["current_producer"];
        let binding = if id == "withdrawProposal" {
            "exact_proposal_producer"
        } else {
            "exact_current_proposal_revision_producer"
        };
        assert_eq!(
            (
                &producer["cause_binding"],
                &producer["action_class"],
                &producer["admission"],
                &producer["idempotency"],
            ),
            (
                &json!(binding),
                &json!("producer_cause"),
                &json!("none"),
                &json!("header_and_producer_cause_record"),
            ),
            "{id}"
        );
        assert_eq!(
            producer["editor"],
            json!({"session": "not-applicable", "writer_generation": "not-applicable", "takeover": "no"}),
            "{id}"
        );
        assert_eq!(
            producer["recovery"]["requires_reconfirmation"], "forbidden",
            "{id}"
        );
        assert_eq!(
            producer["settlement"]["acknowledgements"],
            json!(["committed"]),
            "{id}"
        );
        assert_eq!(
            producer["producer_types"],
            json!(["AgentRunDecision"]),
            "{id}"
        );
        assert!(
            !text(&producer["semantic_cause"]).contains("admission"),
            "{id}"
        );
    }
}

#[test]
fn export_and_volume_capabilities_name_their_exact_operations_and_actions() {
    let catalog = catalog();
    let capability = |id: &str| {
        array(&catalog["release1_capability_coverage"]["capabilities"])
            .iter()
            .find(|capability| capability["capability_id"] == id)
            .unwrap_or_else(|| panic!("capability {id} must exist"))
            .clone()
    };
    let required = |id: &str| {
        array(&capability(id)["required_operations"])
            .iter()
            .map(|operation| text(operation).to_owned())
            .collect::<BTreeSet<_>>()
    };
    let set = |operations: &[&str]| {
        operations
            .iter()
            .map(|operation| (*operation).to_owned())
            .collect()
    };
    assert_eq!(
        required("human_readable_manuscript_export"),
        set(&[
            "exportHumanReadableManuscript",
            "getHumanReadableManuscriptExport"
        ])
    );
    // The Server serves no archive import route, so the archive journey is export only.
    assert_eq!(
        required("portable_project_archive"),
        set(&["exportProjectArchive", "getExportOperation"])
    );
    let actions = &capability("volume_chapter_lifecycle")["operation_actions"];
    for id in ["updateVolume", "updateChapter"] {
        let covered = array(&actions[id])
            .iter()
            .map(text)
            .collect::<BTreeSet<_>>();
        assert_eq!(covered, BTreeSet::from(["rename", "reorder"]), "{id}");
    }
}

#[test]
fn author_edit_keeps_its_one_ingress_and_its_independent_outcome_query() {
    let catalog = catalog();
    let edit = operation(&catalog, "applyAuthorEdit");
    assert_eq!(
        (
            &edit["method"],
            &edit["path"],
            &edit["action_class"],
            &edit["admission"]
        ),
        (
            &json!("POST"),
            &json!("/api/v1/projects/{project_id}/manuscript/author-edits"),
            &json!("direct_editor_action"),
            &json!("author_command"),
        )
    );
    let notes = text(&edit["notes"]);
    assert!(notes.contains("Core classifies") && notes.contains("Web Client never chooses"));
    assert!(edit["settlement"]["settlement_query"].is_null());
    assert!(
        edit["settlement"]
            .get("settlement_query_operation_id")
            .is_none()
    );

    let outcome = operation(&catalog, "getApplyAuthorEditOutcome");
    assert_eq!(
        (&outcome["kind"], &outcome["method"], &outcome["path"]),
        (
            &json!("query"),
            &json!("GET"),
            &json!(
                "/api/v1/projects/{project_id}/manuscript/author-edit-outcomes/{idempotency_key}"
            ),
        )
    );
    assert_eq!(
        outcome["query_proof"],
        json!({
            "identity": "apply_author_edit_idempotency_key",
            "header": "X-StoryOS-Anti-Forgery",
            "source": "original_project_command_challenge_nonce",
            "url": "forbidden",
            "response": "forbidden",
            "consumption": "none",
            "cache_control": "no-store",
        })
    );
    assert_eq!(
        outcome["editor"]["writer_generation"],
        "bound-to-original-admission-when-present"
    );
    let settlement = &outcome["settlement"];
    assert_eq!(settlement["mode"], "read");
    assert_eq!(settlement["acknowledgements"], json!([]));
    assert!(settlement["settlement_query"].is_null() && settlement["operation_ref"].is_null());
    let notes = text(&outcome["notes"]);
    for boundary in [
        "Rejected requires",
        "StillUnknown grants no retry",
        "creates no Admission",
    ] {
        assert!(
            notes.contains(boundary),
            "getApplyAuthorEditOutcome notes omit {boundary}"
        );
    }
}
