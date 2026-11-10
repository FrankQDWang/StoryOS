//! The Release 1 operation table: one row for each public operation, in the declared order.

use schemars::{JsonSchema, schema_for};
use serde_json::{Value, json};
use ts_rs::Config;

use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_wire::U64_WIRE;

#[path = "release1_operation_rows_agent_run.rs"]
mod agent_run_rows;
#[path = "release1_operation_rows_author_edit.rs"]
mod author_edit_rows;
#[path = "release1_operation_rows_chapter.rs"]
mod chapter_rows;
#[path = "release1_operation_rows_editor_session.rs"]
mod editor_session_rows;
#[path = "release1_operation_rows_export.rs"]
mod export_rows;
#[path = "release1_operation_rows_generation.rs"]
mod generation_rows;
#[path = "release1_operation_rows_project.rs"]
mod project_rows;
#[path = "release1_operation_rows_project_settings.rs"]
mod project_settings_rows;
#[path = "release1_operation_rows_proposal_recovery.rs"]
mod proposal_recovery_rows;
#[path = "release1_operation_rows_proposal.rs"]
mod proposal_rows;
#[path = "release1_operation_rows_query.rs"]
mod query_rows;
#[path = "release1_operation_rows_research_source.rs"]
mod research_source_rows;
#[path = "release1_operation_rows_run_control.rs"]
mod run_control_rows;
#[path = "release1_operation_rows_stream.rs"]
mod stream_rows;
#[path = "release1_operation_rows_tool_enablement.rs"]
mod tool_enablement_rows;
#[path = "release1_operation_rows_volume.rs"]
mod volume_rows;

/// One public operation: its registered operation and the hand data that the registry does not hold.
pub(super) struct OperationRow {
    pub(super) operation: RegisteredOperation,
    pub(super) summary: &'static str,
    pub(super) headers: Headers,
    pub(super) schemas: SchemaSources,
    /// The ts-rs declarations that this row adds to the TypeScript declaration file, in order.
    pub(super) typescript: &'static [Declaration],
    pub(super) fixture: Fixture,
}

pub(super) type Declaration = fn(&Config) -> String;

/// The header parameters of an operation, and the headers of its responses.
#[derive(Clone, Copy)]
pub(super) struct Headers {
    pub(super) origin: bool,
    pub(super) idempotency_key: bool,
    pub(super) anti_forgery: bool,
    /// The 429 response declares the Retry-After header.
    pub(super) retry_after: bool,
    /// Each response declares `Cache-Control: no-store`.
    pub(super) no_store: bool,
}

impl Headers {
    pub(super) const NONE: Self = Self {
        origin: false,
        idempotency_key: false,
        anti_forgery: false,
        retry_after: false,
        no_store: false,
    };

    /// The three command header parameters and the Retry-After header of the 429 response.
    pub(super) const COMMAND: Self = Self::NONE
        .origin()
        .idempotency_key()
        .anti_forgery()
        .retry_after();

    pub(super) const fn origin(self) -> Self {
        Self {
            origin: true,
            ..self
        }
    }

    pub(super) const fn idempotency_key(self) -> Self {
        Self {
            idempotency_key: true,
            ..self
        }
    }

    pub(super) const fn anti_forgery(self) -> Self {
        Self {
            anti_forgery: true,
            ..self
        }
    }

    pub(super) const fn retry_after(self) -> Self {
        Self {
            retry_after: true,
            ..self
        }
    }

    pub(super) const fn no_store(self) -> Self {
        Self {
            no_store: true,
            ..self
        }
    }
}

/// The request and response schemas of an operation, and the wire values that only a schema gives.
pub(super) struct SchemaSources {
    /// The schema of the request body. A GET operation has no request body and no request schema.
    pub(super) request: Option<SchemaSource>,
    pub(super) response: SchemaSource,
    /// The schema of the optional query parameter that the client override names.
    pub(super) query_value: QueryValue,
    pub(super) response_media: ResponseMedia,
}

impl SchemaSources {
    pub(super) const fn new(request: SchemaSource, response: SchemaSource) -> Self {
        Self::of(Some(request), response)
    }

    /// The schemas of a GET operation: only the response has a schema.
    pub(super) const fn get(response: SchemaSource) -> Self {
        Self::of(None, response)
    }

    const fn of(request: Option<SchemaSource>, response: SchemaSource) -> Self {
        Self {
            request,
            response,
            query_value: QueryValue::None,
            response_media: ResponseMedia::Json,
        }
    }

    pub(super) const fn query_value(self, query_value: QueryValue) -> Self {
        Self {
            query_value,
            ..self
        }
    }

    pub(super) const fn response_media(self, response_media: ResponseMedia) -> Self {
        Self {
            response_media,
            ..self
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum QueryValue {
    None,
    Uuid,
    /// A canonical decimal string without leading zeros.
    Decimal,
}

/// The media types of the success response beside the media type that the operation kind gives.
#[derive(Clone, Copy)]
pub(super) enum ResponseMedia {
    Json,
    JsonOrProjectArchive,
}

/// One generated JSON Schema file: its file name and the source of its content.
pub(super) struct SchemaSource {
    /// The file name without the directory and the `.schema.json` extension.
    pub(super) file: &'static str,
    pub(super) body: SchemaBody,
}

pub(super) enum SchemaBody {
    /// The schemars schema of a Rust type, then the patch list.
    Type {
        schema: fn() -> Value,
        title: &'static str,
        patches: &'static [Patch],
    },
    /// A hand-written JSON Schema without its `$id` and `title`.
    Json {
        title: &'static str,
        json: &'static str,
    },
}

impl SchemaSource {
    pub(super) const fn typed(
        file: &'static str,
        schema: fn() -> Value,
        title: &'static str,
        patches: &'static [Patch],
    ) -> Self {
        Self {
            file,
            body: SchemaBody::Type {
                schema,
                title,
                patches,
            },
        }
    }

    pub(super) const fn json(file: &'static str, title: &'static str, json: &'static str) -> Self {
        Self {
            file,
            body: SchemaBody::Json { title, json },
        }
    }
}

/// The schemars schema of a Rust type before its `$id`, its title, and its patches.
pub(super) fn raw_schema<T: JsonSchema>() -> Value {
    serde_json::to_value(schema_for!(T)).expect("contract schema serializes")
}

/// One edit at each of a list of JSON pointers inside a schema.
pub(super) struct Patch(pub(super) Edit, pub(super) &'static [&'static str]);

pub(super) const NO_PATCHES: &[Patch] = &[];
pub(super) const SCHEMA_ID: Edit = Edit::SchemaIdConst;
pub(super) const UUID: Edit = Edit::Format("uuid");
pub(super) const DATE_TIME: Edit = Edit::Format("date-time");
pub(super) const U64: Edit = Edit::Pattern(U64_WIRE);

pub(super) enum Edit {
    /// The `const` keyword with the `$id` of the patched schema.
    SchemaIdConst,
    Const(&'static str),
    Format(&'static str),
    Pattern(&'static str),
    MinLength(u64),
    MaxLength(u64),
    MaxUtf8Bytes(u64),
    MinItems(u64),
    MaxItems(u64),
    UniqueItems,
    Enum(&'static [&'static str]),
    /// Replace the value with a string of this pattern or null.
    NullablePattern(&'static str),
    /// Move the inline value to this definition, and replace it with a reference or null.
    NullableDefinition(&'static str),
    /// Replace a nullable `anyOf` with its first alternative.
    NotNull,
}

/// Apply a patch list in order. A missing object on a pointer becomes an object, as `IndexMut` does.
pub(super) fn apply_patches(schema: &mut Value, patches: &[Patch]) {
    let schema_id = schema["$id"].clone();
    for Patch(edit, pointers) in patches {
        for pointer in *pointers {
            if let Edit::NullableDefinition(definition) = edit {
                let reference = json!({"anyOf": [
                    {"$ref": format!("#/$defs/{definition}")},
                    {"type": "null"},
                ]});
                let inline = std::mem::replace(
                    schema.pointer_mut(pointer).expect("an inline schema"),
                    reference,
                );
                schema["$defs"][*definition] = inline;
                continue;
            }
            let node = pointer
                .split('/')
                .skip(1)
                .fold(&mut *schema, |node, token| match node {
                    Value::Array(items) => &mut items[token.parse::<usize>().expect("array index")],
                    _ => &mut node[token],
                });
            apply_edit(node, edit, &schema_id);
        }
    }
}

fn apply_edit(node: &mut Value, edit: &Edit, schema_id: &Value) {
    match edit {
        Edit::SchemaIdConst => node["const"] = schema_id.clone(),
        Edit::Const(value) => node["const"] = json!(value),
        Edit::Format(format) => node["format"] = json!(format),
        Edit::Pattern(pattern) => node["pattern"] = json!(pattern),
        Edit::MinLength(length) => node["minLength"] = json!(length),
        Edit::MaxLength(length) => node["maxLength"] = json!(length),
        Edit::MaxUtf8Bytes(bytes) => node["x-storyos-max-utf8-bytes"] = json!(bytes),
        Edit::MinItems(count) => node["minItems"] = json!(count),
        Edit::MaxItems(count) => node["maxItems"] = json!(count),
        Edit::UniqueItems => node["uniqueItems"] = json!(true),
        Edit::Enum(values) => node["enum"] = json!(values),
        Edit::NullablePattern(pattern) => {
            *node = json!({"anyOf": [{"type": "string", "pattern": pattern}, {"type": "null"}]});
        }
        Edit::NullableDefinition(_) => unreachable!("apply_patches moves the definition"),
        Edit::NotNull => {
            let Value::Object(first) = node["anyOf"][0].take() else {
                panic!("the first alternative of a nullable schema is an object");
            };
            let object = node
                .as_object_mut()
                .expect("a nullable schema is an object");
            object.remove("anyOf");
            object.extend(first);
        }
    }
}

/// The one positive golden-wire fixture of an operation.
pub(super) enum Fixture {
    Json(&'static str),
    /// The active protocol profile that the generator assembles.
    ProtocolProfile,
}

/// The declared order of the public operations; every generated artifact follows it.
pub(super) const OPERATION_TABLE: &[&OperationRow] = &[
    &project_rows::GET_PROTOCOL_PROFILE_ROW,
    &project_rows::CREATE_PROJECT_CHALLENGE_ROW,
    &project_rows::CREATE_PROJECT_ROW,
    &project_rows::CREATE_PROJECT_COMMAND_CHALLENGE_ROW,
    &project_rows::LIST_PROJECTS_ROW,
    &project_rows::GET_PROJECT_ROW,
    &project_settings_rows::UPDATE_PROJECT_ROW,
    &project_settings_rows::GET_PROJECT_ASSISTANCE_ROW,
    &project_settings_rows::UPDATE_PROJECT_ASSISTANCE_ROW,
    &tool_enablement_rows::SET_PROJECT_TOOL_ENABLEMENT_ROW,
    &editor_session_rows::CREATE_EDITOR_SESSION_ROW,
    &editor_session_rows::GET_EDITOR_SESSION_ROW,
    &editor_session_rows::TAKE_OVER_PROJECT_WRITER_ROW,
    &volume_rows::CREATE_VOLUME_ROW,
    &volume_rows::UPDATE_VOLUME_ROW,
    &volume_rows::DELETE_VOLUME_ROW,
    &chapter_rows::CREATE_CHAPTER_ROW,
    &chapter_rows::UPDATE_CHAPTER_ROW,
    &chapter_rows::DELETE_CHAPTER_ROW,
    &project_settings_rows::SET_CURRENT_CHAPTER_ROW,
    &author_edit_rows::APPLY_AUTHOR_EDIT_ROW,
    &proposal_rows::ACCEPT_PROPOSAL_ROW,
    &proposal_rows::REJECT_PROPOSAL_OPERATIONS_ROW,
    &proposal_rows::WITHDRAW_PROPOSAL_ROW,
    &proposal_recovery_rows::REPLAN_PROPOSAL_ROW,
    &proposal_recovery_rows::REOPEN_WITHDRAWN_PROPOSAL_ROW,
    &proposal_recovery_rows::REOPEN_REJECTED_OPERATIONS_ROW,
    &generation_rows::COMPLETE_READY_PARTIAL_PROPOSAL_ROW,
    &generation_rows::CONTINUE_PROPOSAL_GENERATION_ROW,
    &author_edit_rows::UNDO_LATEST_AUTHOR_ACTION_ROW,
    &agent_run_rows::CREATE_AGENT_RUN_ROW,
    &agent_run_rows::GET_AGENT_RUN_ROW,
    &run_control_rows::STEER_AGENT_RUN_ROW,
    &run_control_rows::PAUSE_AGENT_RUN_ROW,
    &run_control_rows::CANCEL_AGENT_RUN_ROW,
    &research_source_rows::LIST_RESEARCH_SOURCES_ROW,
    &research_source_rows::GET_RESEARCH_SOURCE_ROW,
    &export_rows::EXPORT_PROJECT_ARCHIVE_ROW,
    &export_rows::EXPORT_HUMAN_READABLE_MANUSCRIPT_ROW,
    &query_rows::GET_MANUSCRIPT_TREE_ROW,
    &query_rows::GET_CHAPTER_ROW,
    &query_rows::GET_STATISTICS_ROW,
    &generation_rows::GET_PROPOSAL_ROW,
    &generation_rows::LIST_CHAPTER_OPEN_PROPOSALS_ROW,
    &author_edit_rows::GET_APPLY_AUTHOR_EDIT_OUTCOME_ROW,
    &stream_rows::GET_SNAPSHOT_ROW,
    &stream_rows::ACTIVITY_STREAM_ROW,
    &query_rows::SEARCH_MANUSCRIPT_ROW,
    &export_rows::GET_EXPORT_OPERATION_ROW,
    &export_rows::GET_HUMAN_READABLE_MANUSCRIPT_EXPORT_ROW,
];

/// The registered operations of the table, in the declared order.
pub(super) fn registered_operations() -> impl Iterator<Item = &'static RegisteredOperation> {
    OPERATION_TABLE.iter().map(|row| &row.operation)
}
