//! Writes the artifact parts of each public operation from its row in the operation table.

use std::sync::LazyLock;

use serde_json::{Value, json};
use ts_rs::Config;

use super::{ArtifactSource, GeneratedFile};
use crate::release1::HttpMethod;
use crate::release1::Release1ProtocolProfile;
use crate::release1_operation_registry::{
    ClientOverride, GeneratedSchema, OpenApiMethod, OperationKind,
};
use crate::release1_operation_table::{
    Fixture, Headers, OPERATION_TABLE, OperationRow, QueryValue, ResponseMedia, SchemaBody,
    SchemaSource, apply_patches, registered_operations,
};
use crate::release1_route_catalog::route_catalog_bytes;
use crate::release1_wire::json_bytes;

const SCHEMA_DIRECTORY: &str = "generated/json-schema/storyos-public-release-1/";
const FIXTURE_DIRECTORY: &str = "generated/golden-wire/storyos-public-release-1/";
const RETRY_AFTER: &str = "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n";
const NO_STORE: &str = "          headers:\n            Cache-Control:\n              required: true\n              schema:\n                type: string\n                const: no-store\n";
const PROJECT_ARCHIVE_MEDIA: &str = "            application/vnd.storyos.project-archive+zip:\n              schema:\n                type: string\n                format: binary\n";

/// The generated paths of one row; the paths live as long as the table.
struct RowPaths {
    request_schema: Option<String>,
    response_schema: String,
    fixture: String,
}

static ROW_PATHS: LazyLock<Vec<RowPaths>> = LazyLock::new(|| {
    OPERATION_TABLE
        .iter()
        .map(|row| RowPaths {
            request_schema: row.schemas.request.as_ref().map(schema_path),
            response_schema: schema_path(&row.schemas.response),
            fixture: format!(
                "{FIXTURE_DIRECTORY}{}.json",
                kebab_case(row.operation.operation.operation_id)
            ),
        })
        .collect()
});

/// The artifact parts that the operation table writes.
pub(super) fn table_source() -> ArtifactSource {
    ArtifactSource {
        route_catalog: route_catalog_bytes(),
        operations: registered_operations().collect(),
        openapi_methods: rows().map(openapi_method).collect(),
        schemas: rows().flat_map(schema_files).collect(),
        typescript_types: rows().map(|(row, _)| declarations(row)).collect(),
    }
}

fn rows() -> impl Iterator<Item = (&'static OperationRow, &'static RowPaths)> {
    OPERATION_TABLE.iter().copied().zip(ROW_PATHS.iter())
}

fn schema_path(source: &SchemaSource) -> String {
    format!("{SCHEMA_DIRECTORY}{}.schema.json", source.file)
}

fn kebab_case(operation_id: &str) -> String {
    let mut kebab = String::new();
    for character in operation_id.chars() {
        if character.is_ascii_uppercase() {
            kebab.push('-');
        }
        kebab.push(character.to_ascii_lowercase());
    }
    kebab
}

fn path_parameters(path: &str) -> Vec<&str> {
    path.split('/')
        .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
        .collect()
}

/// One synthesized path item method. The registry kind picks the stream form or the JSON form.
fn openapi_method((row, paths): (&OperationRow, &RowPaths)) -> OpenApiMethod {
    let registered = &row.operation;
    let operation = registered.operation;
    let reference = |path: &str| {
        format!(
            "'../{}'",
            path.strip_prefix("generated/")
                .expect("schema is a generated artifact")
        )
    };
    let mut parameters = path_parameters(operation.path)
        .into_iter()
        .map(|name| parameter(name, "path", Requirement::Required, "format: uuid"))
        .collect::<String>();
    if let ClientOverride::OptionalQueryParameter(name) = registered.client {
        let schema = match row.schemas.query_value {
            QueryValue::Uuid => "format: uuid",
            QueryValue::Decimal => "pattern: '^(0|[1-9][0-9]*)$'",
            QueryValue::None => panic!("{name} needs a query value schema"),
        };
        parameters.push_str(&parameter(name, "query", Requirement::Optional, schema));
    }
    let media = match registered.kind {
        OperationKind::Query | OperationKind::Command | OperationKind::Challenge => {
            "application/json"
        }
        OperationKind::Stream => {
            parameters.push_str(&parameter(
                "snapshot_id",
                "query",
                Requirement::Required,
                "format: uuid",
            ));
            parameters.push_str(&string_parameter(
                "protocol_release",
                "query",
                Requirement::Required,
            ));
            "text/event-stream"
        }
    };
    parameters.push_str(&header_parameters(row.headers));
    if registered.kind == OperationKind::Stream {
        parameters.push_str(&string_parameter(
            "Last-Event-ID",
            "header",
            Requirement::Optional,
        ));
    }
    let mut yaml = format!(
        "    {}:\n      operationId: {}\n      summary: {}\n",
        operation.method.as_str().to_ascii_lowercase(),
        operation.operation_id,
        row.summary,
    );
    if !parameters.is_empty() {
        yaml.push_str("      parameters:\n");
        yaml.push_str(&parameters);
    }
    if operation.method != HttpMethod::Get {
        yaml.push_str(&format!(
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: {}\n",
            reference(
                paths
                    .request_schema
                    .as_ref()
                    .expect("a request body has a schema source")
            ),
        ));
    }
    yaml.push_str("      responses:\n");
    for (status, description) in operation.responses {
        yaml.push_str(&format!(
            "        '{status}':\n          description: {description}\n"
        ));
        if row.headers.no_store {
            yaml.push_str(NO_STORE);
        }
        if *status == 429 && row.headers.retry_after {
            yaml.push_str(RETRY_AFTER);
        }
        if (200..300).contains(status) {
            yaml.push_str(&format!(
                "          content:\n            {media}:\n              schema:\n                $ref: {}\n",
                reference(&paths.response_schema),
            ));
            match row.schemas.response_media {
                ResponseMedia::Json => {}
                ResponseMedia::JsonOrProjectArchive => yaml.push_str(PROJECT_ARCHIVE_MEDIA),
            }
        }
    }
    OpenApiMethod {
        path: operation.path,
        yaml,
    }
}

#[derive(Clone, Copy)]
enum Requirement {
    Required,
    Optional,
}

fn parameter(name: &str, location: &str, requirement: Requirement, schema: &str) -> String {
    format!(
        "{}            {schema}\n",
        string_parameter(name, location, requirement)
    )
}

fn string_parameter(name: &str, location: &str, requirement: Requirement) -> String {
    let required = match requirement {
        Requirement::Required => "true",
        Requirement::Optional => "false",
    };
    format!(
        "        - name: {name}\n          in: {location}\n          required: {required}\n          schema:\n            type: string\n"
    )
}

fn header_parameters(headers: Headers) -> String {
    [
        (headers.origin, "Origin", "format: uri"),
        (headers.idempotency_key, "Idempotency-Key", "format: uuid"),
        (
            headers.anti_forgery,
            "X-StoryOS-Anti-Forgery",
            "pattern: '^[0-9a-f]{64}$'",
        ),
    ]
    .into_iter()
    .filter(|(declared, _, _)| *declared)
    .map(|(_, name, schema)| parameter(name, "header", Requirement::Required, schema))
    .collect()
}

/// The request schema file, when the operation has a request body, and the response schema file.
fn schema_files(
    (row, paths): (&'static OperationRow, &'static RowPaths),
) -> impl Iterator<Item = GeneratedSchema> {
    let operation = row.operation.operation;
    let request = row
        .schemas
        .request
        .as_ref()
        .zip(paths.request_schema.as_ref())
        .map(|(source, path)| (source, operation.request_schema, path));
    request
        .into_iter()
        .chain([(
            &row.schemas.response,
            operation.response_schema,
            &paths.response_schema,
        )])
        .map(|(source, schema_id, path)| GeneratedSchema {
            schema_id,
            path,
            bytes: json_bytes(&schema_value(source, schema_id)),
        })
}

fn schema_value(source: &SchemaSource, schema_id: &str) -> Value {
    match source.body {
        SchemaBody::Type {
            schema,
            title,
            patches,
        } => {
            let mut schema = schema();
            schema["$id"] = json!(schema_id);
            schema["title"] = json!(title);
            apply_patches(&mut schema, patches);
            schema
        }
        SchemaBody::Json { title, json } => {
            let mut schema: Value = serde_json::from_str(json).expect("schema literal is JSON");
            schema["$id"] = json!(schema_id);
            schema["title"] = json!(title);
            schema
        }
    }
}

pub(super) fn declarations(row: &OperationRow) -> String {
    let config = Config::default();
    row.typescript
        .iter()
        .map(|declaration| format!("export {}", declaration(&config)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(super) fn fixture_files(profile: &Release1ProtocolProfile) -> Vec<GeneratedFile> {
    rows()
        .map(|(row, paths)| {
            let value = match row.fixture {
                Fixture::Json(json) => serde_json::from_str(json).expect("fixture literal is JSON"),
                Fixture::ProtocolProfile => {
                    serde_json::to_value(profile).expect("protocol profile should serialize")
                }
            };
            (paths.fixture.as_str(), json_bytes(&value))
        })
        .collect()
}
