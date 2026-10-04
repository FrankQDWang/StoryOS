use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{
    GET_PROTOCOL_PROFILE, PROTOCOL_PROFILE_REQUEST_SCHEMA_ID, PROTOCOL_PROFILE_SCHEMA_ID,
    Release1CompatibilityIdentity, Release1ProtocolProfile,
};
use crate::release1_operation_registry::{
    GeneratedSchema, OperationArtifacts, RegisteredOperation, fixture_triple, method,
};
use crate::release1_wire::{json_bytes, query_openapi, schema_value};

const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/protocol-profile-request.schema.json";

pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/protocol-profile-response.schema.json";

const GOLDEN_PROFILE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-protocol-profile.json";

const INVALID_PROFILE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-protocol-profile.invalid.json";

const BOUNDARY_PROFILE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-protocol-profile.boundary.json";

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_PROTOCOL_PROFILE,
        &["active_public_release_profile"],
    )],
    schemas: || {
        vec![
            GeneratedSchema {
                schema_id: PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
                path: REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&protocol_profile_request_schema()),
            },
            GeneratedSchema {
                schema_id: PROTOCOL_PROFILE_SCHEMA_ID,
                path: RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&protocol_profile_schema()),
            },
        ]
    },
    openapi: || {
        method(
            &GET_PROTOCOL_PROFILE,
            query_openapi(
                &GET_PROTOCOL_PROFILE,
                "Discover the active StoryOS Release 1 protocol profile",
                RESPONSE_SCHEMA_PATH,
                &[],
            ),
        )
    },
    typescript_types: || {
        let config = Config::default();
        format!(
            "export {}\n\nexport {}",
            Release1CompatibilityIdentity::decl(&config),
            Release1ProtocolProfile::decl(&config),
        )
    },
    typescript_client: || {
        format!(
            "\nexport async function getProtocolProfile(options = {{}}) {{\n  return queryJson({{ ...options, path: \"{}\" }});\n}}\n",
            GET_PROTOCOL_PROFILE.path,
        )
    },
    typescript_declarations: || "export declare function getProtocolProfile(options: StoryOSQueryOptions): Promise<Release1ProtocolProfile>;\n",
    fixtures: || {
        fixture_triple(
            [
                GOLDEN_PROFILE_PATH,
                INVALID_PROFILE_PATH,
                BOUNDARY_PROFILE_PATH,
            ],
            &GET_PROTOCOL_PROFILE,
            [
                golden_profile_bytes,
                invalid_profile_bytes,
                boundary_profile_bytes,
            ],
        )
        .into()
    },
};

fn protocol_profile_request_schema() -> Value {
    json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "$id": PROTOCOL_PROFILE_REQUEST_SCHEMA_ID,
           "title": "StoryOS Release 1 Protocol Profile Request", "type": "object",
           "additionalProperties": false, "maxProperties": 0})
}

fn protocol_profile_schema() -> Value {
    schema_value::<Release1ProtocolProfile>(
        PROTOCOL_PROFILE_SCHEMA_ID,
        "StoryOS Release 1 Protocol Profile",
    )
}

fn golden_profile_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    json_bytes(&serde_json::to_value(profile).expect("protocol profile should serialize"))
}

fn invalid_profile_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    let mut invalid = serde_json::to_value(profile).expect("protocol profile should serialize");
    invalid
        .as_object_mut()
        .expect("protocol profile should be an object")
        .remove("release_identity");
    json_bytes(&invalid)
}

fn boundary_profile_bytes(profile: &Release1ProtocolProfile) -> Vec<u8> {
    let mut boundary = profile.clone();
    boundary.release_identity.generated_client_revision =
        "storyos.typescript-client.release-0.v1".to_owned();
    golden_profile_bytes(&boundary)
}
