//! The published shape of the Receipt Author Action sequence (#1680).

use crate::release1_author_edit::DomainReceiptResult;
use crate::release1_undo_latest_author_action::{
    UndoLatestAuthorActionConflictReason, UndoLatestAuthorActionEffect,
    UndoLatestAuthorActionResponse, UndoLatestAuthorActionUnavailableReason,
};

const SCHEMA_DIRECTORY: &str = "generated/json-schema/storyos-public-release-1";

type GeneratedFiles = std::collections::BTreeMap<&'static str, Vec<u8>>;

fn generated() -> GeneratedFiles {
    super::generated_files().into_iter().collect()
}

fn json(generated: &GeneratedFiles, path: &str) -> serde_json::Value {
    serde_json::from_slice(&generated[path]).expect("generated file must be JSON")
}

#[test]
fn each_schema_with_a_receipt_gives_the_sequence_one_nullable_shape() {
    let nullable = serde_json::json!({"anyOf": [
        {"type": "string", "pattern": crate::release1_wire::U64_WIRE},
        {"type": "null"},
    ]});
    let generated = generated();
    let mut checked = 0;
    for path in generated
        .keys()
        .filter(|path| path.starts_with(SCHEMA_DIRECTORY))
    {
        let schema = json(&generated, path);
        let Some(receipt) = schema.pointer("/$defs/DomainReceipt") else {
            continue;
        };
        assert_eq!(
            receipt["properties"]["author_action_sequence"], nullable,
            "{path}"
        );
        let required = receipt["required"].as_array().expect("required list");
        assert!(
            required.contains(&serde_json::json!("author_action_sequence")),
            "{path}"
        );
        checked += 1;
    }
    assert_eq!(checked, 19, "each schema with a Receipt is checked");
}

/// The Author Undo response schema accepts the Receipt of an applied, a conflicted, and an
/// unavailable Undo, as the Server writes each one.
#[test]
fn the_undo_response_schema_accepts_each_receipt_sequence_of_undo() {
    let generated = generated();
    let schema = json(
        &generated,
        &format!("{SCHEMA_DIRECTORY}/undo-latest-author-action-response.schema.json"),
    );
    let response = jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("generated schema must compile");
    let applied: UndoLatestAuthorActionResponse = serde_json::from_value(json(
        &generated,
        "generated/golden-wire/storyos-public-release-1/undo-latest-author-action.json",
    ))
    .expect("the Undo fixture must deserialize");
    assert_eq!(applied.receipt.author_action_sequence.as_deref(), Some("2"));
    let without_action = |result, effect| {
        let mut changed = applied.clone();
        changed.receipt.author_action_sequence = None;
        changed.receipt.authoritative_revision_ids = Vec::new();
        changed.receipt.authoritative_commit_ids = Vec::new();
        changed.receipt.result = result;
        changed.effect = effect;
        changed
    };
    let conflicted = without_action(
        DomainReceiptResult::Conflicted,
        UndoLatestAuthorActionEffect::Conflicted {
            reason: UndoLatestAuthorActionConflictReason::FrontierMismatch,
            current_author_undo_frontier_sequence: Some("3".to_owned()),
        },
    );
    let unavailable = without_action(
        DomainReceiptResult::Refused,
        UndoLatestAuthorActionEffect::Unavailable {
            reason: UndoLatestAuthorActionUnavailableReason::Barrier,
        },
    );
    for (name, document) in [
        ("applied", &applied),
        ("conflicted", &conflicted),
        ("unavailable", &unavailable),
    ] {
        let wire = serde_json::to_value(document).expect("the Undo response must serialize");
        assert!(response.is_valid(&wire), "{name}: {wire}");
        let mut not_canonical = wire.clone();
        not_canonical["receipt"]["author_action_sequence"] = serde_json::json!("01");
        assert!(!response.is_valid(&not_canonical), "{name}");
        let mut without_field = wire.clone();
        without_field["receipt"]
            .as_object_mut()
            .expect("the Receipt is an object")
            .remove("author_action_sequence");
        assert!(!response.is_valid(&without_field), "{name}");
    }
}
