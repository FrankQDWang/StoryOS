use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{
    GET_PROTOCOL_PROFILE, PROTOCOL_PROFILE_REQUEST_SCHEMA_ID, PROTOCOL_PROFILE_SCHEMA_ID,
    Release1CompatibilityIdentity, Release1ProtocolProfile,
};
use crate::release1_operation_registry::{
    ClientOverride, FixtureMembership, GeneratedSchema, OperationArtifacts, RegisteredOperation,
    method,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::{json_bytes, query_openapi, schema_value};

const GET_PROTOCOL_PROFILE_NOTES: &str =
    "Project-free profile discovery; no Project Scope or session authority is returned.";

const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/protocol-profile-request.schema.json";

pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/protocol-profile-response.schema.json";

const GOLDEN_PROFILE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-protocol-profile.json";

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_PROTOCOL_PROFILE,
        &["active_public_release_profile"],
        OperationCatalog::new("protocol:profile", Owner::Protocol, Binding::PROTOCOL_QUERY)
            .notes(GET_PROTOCOL_PROFILE_NOTES),
    )
    .client(ClientOverride::ResponseType("Release1ProtocolProfile"))],
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
    fixtures: &[FixtureMembership {
        path: GOLDEN_PROFILE_PATH,
        bytes: golden_profile_bytes,
    }],
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
