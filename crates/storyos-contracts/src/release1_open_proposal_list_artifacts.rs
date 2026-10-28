use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_open_proposal_list::{
    LIST_CHAPTER_OPEN_PROPOSALS, LIST_CHAPTER_OPEN_PROPOSALS_REQUEST_SCHEMA_ID,
    LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE_SCHEMA_ID, ListChapterOpenProposalsResponse,
    OPEN_PROPOSAL_LIST_MAX_ENTRIES, OpenProposalEntry, OpenProposalListCompleteness,
    OpenProposalSource,
};
use crate::release1_operation_registry::{
    OperationArtifacts, RegisteredOperation, fixture_triple, method, operation_schemas,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::json_bytes;

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &LIST_CHAPTER_OPEN_PROPOSALS,
        &[
            "server_derived_project_scope",
            "chapter_scope_join",
            "bounded_open_proposal_list",
        ],
        OperationCatalog::new(
            "proposal:chapter-open-list",
            Owner::Core,
            Binding::CANONICAL_QUERY,
        ),
    )],
    schemas: || {
        operation_schemas(
            &LIST_CHAPTER_OPEN_PROPOSALS,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&LIST_CHAPTER_OPEN_PROPOSALS, openapi()),
    typescript_types: typescript_type_declarations,
    fixtures: || {
        fixture_triple(
            FIXTURE_PATHS,
            [
                |_| fixture_bytes(),
                |_| invalid_fixture_bytes(),
                |_| boundary_fixture_bytes(),
            ],
        )
        .into()
    },
};

const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/chapter-open-proposals-request.schema.json";
const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/chapter-open-proposals-response.schema.json";
const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/list-chapter-open-proposals.json",
    "generated/golden-wire/storyos-public-release-1/list-chapter-open-proposals.invalid.json",
    "generated/golden-wire/storyos-public-release-1/list-chapter-open-proposals.boundary.json",
];

fn request_schema_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": LIST_CHAPTER_OPEN_PROPOSALS_REQUEST_SCHEMA_ID,
        "title": "StoryOS Chapter Open Proposals Request",
        "type": "object",
        "additionalProperties": false,
        "required": ["project_id", "chapter_id"],
        "properties": {
            "project_id": {"type": "string", "format": "uuid"},
            "chapter_id": {"type": "string", "format": "uuid"}
        }
    }))
}

fn response_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(ListChapterOpenProposalsResponse))
        .expect("chapter open proposals response schema serializes");
    schema["$id"] = json!(LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE_SCHEMA_ID);
    schema["title"] = json!("StoryOS Chapter Open Proposals Response");
    schema["additionalProperties"] = json!(false);
    schema["properties"]["schema_id"]["const"] =
        json!(LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["chapter_id"]["format"] = json!("uuid");
    schema["properties"]["proposals"]["maxItems"] = json!(OPEN_PROPOSAL_LIST_MAX_ENTRIES);
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    if let Some(entry) = schema["$defs"].get_mut("OpenProposalEntry") {
        for field in ["proposal_id", "revision_id"] {
            entry["properties"][field]["format"] = json!("uuid");
        }
        entry["properties"]["kind"]["enum"] = json!(["block_edit", "reversal"]);
    }
    if let Some(variants) = schema["$defs"]
        .get_mut("OpenProposalSource")
        .and_then(|source| source.get_mut("oneOf"))
        .and_then(Value::as_array_mut)
    {
        for variant in variants {
            for field in ["run_id", "decision_id"] {
                if variant
                    .get("properties")
                    .and_then(|properties| properties.get(field))
                    .is_some()
                {
                    variant["properties"][field]["format"] = json!("uuid");
                }
            }
        }
    }
    json_bytes(&schema)
}

fn openapi() -> String {
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("schema is a generated artifact");
    let responses = LIST_CHAPTER_OPEN_PROPOSALS
        .responses
        .iter()
        .map(|(status, description)| {
            let content = if *status == 200 {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!("        '{status}':\n          description: {description}\n{content}")
        })
        .collect::<String>();
    format!(
        concat!(
            "    get:\n      operationId: {}\n      summary: List the open Proposals of one Chapter\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: chapter_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "      responses:\n{}",
        ),
        LIST_CHAPTER_OPEN_PROPOSALS.operation_id, responses,
    )
}

fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}",
        OpenProposalSource::decl(&config),
        OpenProposalEntry::decl(&config),
        OpenProposalListCompleteness::decl(&config),
        ListChapterOpenProposalsResponse::decl(&config),
    )
}

fn fixture_bytes() -> Vec<u8> {
    json_bytes(&list_fixture())
}

fn invalid_fixture_bytes() -> Vec<u8> {
    let mut value = list_fixture();
    value
        .as_object_mut()
        .expect("fixture is an object")
        .remove("completeness");
    json_bytes(&value)
}

fn boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = list_fixture();
    boundary["proposals"] = json!([]);
    json_bytes(&boundary)
}

fn list_fixture() -> Value {
    json!({
        "schema_id": LIST_CHAPTER_OPEN_PROPOSALS_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000c01",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "chapter_id": "018f0000-0000-7001-8000-000000000301",
        "completeness": "complete",
        "proposals": [
            {
                "proposal_id": "018f0000-0000-7001-8000-000000000b02",
                "revision_id": "018f0000-0000-7001-8000-000000000b03",
                "kind": "block_edit",
                "source": {
                    "kind": "agent_run_decision",
                    "run_id": "018f0000-0000-7001-8000-000000000a34",
                    "decision_id": "018f0000-0000-7001-8000-000000000b07"
                }
            },
            {
                "proposal_id": "018f0000-0000-7001-8000-000000000c02",
                "revision_id": "018f0000-0000-7001-8000-000000000c03",
                "kind": "block_edit",
                "source": {
                    "kind": "agent_run_decision",
                    "run_id": "018f0000-0000-7001-8000-000000000c04",
                    "decision_id": "018f0000-0000-7001-8000-000000000c05"
                }
            }
        ]
    })
}
