//! The route catalog metadata of each registered operation, and the generated route catalog.

use serde::Serialize;
use serde_json::{Value, json};

use crate::ProjectActivityKind;
use crate::release1::QueryOperation;
use crate::release1_operation_registry::{RELEASE1_OPERATIONS, RegisteredOperation};
use crate::release1_wire::json_bytes;

#[path = "release1_route_catalog_capabilities.rs"]
mod capabilities;
#[path = "release1_route_catalog_events.rs"]
mod events;
#[path = "release1_route_catalog_sections.rs"]
mod sections;

pub(super) const ROUTE_CATALOG_PATH: &str =
    "generated/route-catalog/versioned-protocol-release-1-route-catalog.json";
const WIRE_OWNER: Owner = Owner::Protocol;
const AUTHOR_RECOVERY_RULE: &str = "Only the protected author cause may use the Author Command Admission recovery path; RecoveryDraft ingress remains ApplyAuthorEdit only.";
const DURABLE_VISIBILITY: &str =
    "only after the durable transition or async settlement named by the operation";

/// A named semantic owner; `sections` gives the document anchor of each owner.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Owner {
    Protocol,
    Core,
    Admission,
    Editor,
    Assistance,
    Artifact,
    Replay,
    Storage,
    Retention,
    Deletion,
    Release,
    Context,
    Memory,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ActionClass {
    None,
    ExplicitProjectCommand,
    ExplicitEditorCommand,
    DirectEditorAction,
    SessionControl,
    AgentRunStart,
    AgentRunControl,
    ProducerCause,
    ConditionalByCause,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Admission {
    None,
    Query,
    Challenge,
    Sse,
    AuthorCommand,
    AuthorCommandOrAsyncOperation,
    ProducerCause,
    SessionControl,
    ConditionalByCause,
}

/// The Editor Session, writer generation, and takeover rule of one operation.
#[derive(Clone, Copy)]
pub(super) enum EditorRule {
    Unbound,
    Optional,
    NotApplicable,
    NewSession,
    ObservedWriter,
    ExplicitTakeover,
    CurrentWriter,
    BoundToOriginalAdmission,
    ConditionalByCause,
}

impl EditorRule {
    fn json(self) -> Value {
        let (session, writer_generation, takeover) = match self {
            Self::Unbound => ("none", "none", "none"),
            Self::Optional => ("optional", "not-applicable", "no"),
            Self::NotApplicable => ("not-applicable", "not-applicable", "no"),
            Self::NewSession => ("new", "returned_projection", "no"),
            Self::ObservedWriter => ("required", "observed", "no"),
            Self::ExplicitTakeover => ("required", "observed", "explicit"),
            Self::CurrentWriter => ("required", "current", "no"),
            Self::BoundToOriginalAdmission => {
                ("required", "bound-to-original-admission-when-present", "no")
            }
            Self::ConditionalByCause => (
                "conditional_by_cause",
                "conditional_by_cause",
                "conditional_by_cause",
            ),
        };
        json!({ "session": session, "writer_generation": writer_generation, "takeover": takeover })
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Idempotency {
    None,
    BodyKeyAndChallenge,
    StreamCursor,
    HeaderAndAdmissionRecord,
    HeaderAndOperationRecord,
    HeaderAndProducerCauseRecord,
    ConditionalByCause,
}

#[derive(Clone, Copy)]
pub(super) enum Settlement {
    Read,
    Challenge,
    Stream,
    Committed,
    CommittedOrReconfirmation,
    /// Accepted; the settlement is observed through the named GET query.
    Accepted {
        query: &'static QueryOperation,
        operation_ref: &'static str,
    },
    ConditionalByCause,
}

impl Settlement {
    fn json(self, http_statuses: &[u16]) -> Value {
        let (mode, acknowledgements): (_, &[&str]) = match self {
            Self::Read => ("read", &[]),
            Self::Challenge => ("challenge", &[]),
            Self::Stream => ("stream", &[]),
            Self::Committed => ("sync", &["committed"]),
            Self::CommittedOrReconfirmation => ("sync", &["committed", "requires_reconfirmation"]),
            Self::Accepted { .. } => ("async", &["accepted"]),
            Self::ConditionalByCause => ("conditional_by_cause", &["conditional_by_cause"]),
        };
        let mut settlement = json!({
            "mode": mode, "acknowledgements": acknowledgements, "http_statuses": http_statuses,
            "settlement_query": null, "operation_ref": null,
        });
        if let Self::Accepted {
            query,
            operation_ref,
        } = self
        {
            settlement["settlement_query"] = json!(query.path);
            settlement["settlement_query_operation_id"] = json!(query.operation_id);
            settlement["operation_ref"] =
                json!({ "kind": operation_ref, "id_source": "operation_ref" });
        }
        settlement
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ErrorProfile {
    CommandSync,
    EditorCommand,
    AsyncCommand,
    SessionControl,
    QueryCanonical,
    QueryProjection,
    QueryProtocol,
    Challenge,
    Stream,
}

/// The admission, idempotency, settlement, and error rules that operations share.
#[derive(Clone, Copy)]
pub(super) struct Binding {
    pub(super) action_class: ActionClass,
    pub(super) admission: Admission,
    pub(super) editor: EditorRule,
    pub(super) idempotency: Idempotency,
    pub(super) settlement: Settlement,
    pub(super) error_profile: ErrorProfile,
}

impl Binding {
    pub(super) const PROTOCOL_QUERY: Self =
        Self::query(EditorRule::Unbound, ErrorProfile::QueryProtocol);
    pub(super) const PROJECT_QUERY: Self =
        Self::query(EditorRule::Unbound, ErrorProfile::QueryCanonical);
    pub(super) const CANONICAL_QUERY: Self =
        Self::query(EditorRule::Optional, ErrorProfile::QueryCanonical);
    pub(super) const PROJECTION_QUERY: Self =
        Self::query(EditorRule::Optional, ErrorProfile::QueryProjection);
    pub(super) const CHALLENGE: Self = Self {
        action_class: ActionClass::None,
        admission: Admission::Challenge,
        editor: EditorRule::Unbound,
        idempotency: Idempotency::BodyKeyAndChallenge,
        settlement: Settlement::Challenge,
        error_profile: ErrorProfile::Challenge,
    };
    pub(super) const PROJECT_COMMAND: Self = Self {
        action_class: ActionClass::ExplicitProjectCommand,
        admission: Admission::AuthorCommand,
        editor: EditorRule::Unbound,
        idempotency: Idempotency::HeaderAndAdmissionRecord,
        settlement: Settlement::CommittedOrReconfirmation,
        error_profile: ErrorProfile::CommandSync,
    };
    pub(super) const EDITOR_COMMAND: Self = Self {
        action_class: ActionClass::ExplicitEditorCommand,
        admission: Admission::AuthorCommand,
        editor: EditorRule::CurrentWriter,
        idempotency: Idempotency::HeaderAndAdmissionRecord,
        settlement: Settlement::CommittedOrReconfirmation,
        error_profile: ErrorProfile::EditorCommand,
    };
    pub(super) const AGENT_RUN_CONTROL: Self = Self {
        action_class: ActionClass::AgentRunControl,
        admission: Admission::ProducerCause,
        editor: EditorRule::Optional,
        idempotency: Idempotency::HeaderAndOperationRecord,
        settlement: Settlement::Committed,
        error_profile: ErrorProfile::CommandSync,
    };
    /// The rules of a command whose author and current-producer causes have their own profiles.
    const CONDITIONAL_BY_CAUSE: Self = Self {
        action_class: ActionClass::ConditionalByCause,
        admission: Admission::ConditionalByCause,
        editor: EditorRule::ConditionalByCause,
        idempotency: Idempotency::ConditionalByCause,
        settlement: Settlement::ConditionalByCause,
        error_profile: ErrorProfile::EditorCommand,
    };
    /// The current-producer cause of a conditional command: no admission and no Editor Session.
    const CURRENT_PRODUCER_CAUSE: Self = Self {
        action_class: ActionClass::ProducerCause,
        admission: Admission::None,
        editor: EditorRule::NotApplicable,
        idempotency: Idempotency::HeaderAndProducerCauseRecord,
        settlement: Settlement::Committed,
        error_profile: ErrorProfile::EditorCommand,
    };

    const fn query(editor: EditorRule, error_profile: ErrorProfile) -> Self {
        Self {
            action_class: ActionClass::None,
            admission: Admission::Query,
            editor,
            idempotency: Idempotency::None,
            settlement: Settlement::Read,
            error_profile,
        }
    }

    fn json(self, http_statuses: &[u16]) -> Value {
        json!({
            "action_class": self.action_class, "admission": self.admission,
            "editor": self.editor.json(), "idempotency": self.idempotency,
            "settlement": self.settlement.json(http_statuses), "error_profile": self.error_profile,
        })
    }
}

/// The two causes of a conditional command. Each cause has its fixed admission and recovery rules.
pub(super) struct CauseProfiles {
    pub(super) author: AuthorCause,
    pub(super) current_producer: CurrentProducerCause,
}

/// The cause of the author, through a protected Author Command Admission.
pub(super) struct AuthorCause {
    pub(super) semantic_payload: &'static str,
    pub(super) request_schema: &'static str,
    pub(super) response_schema: &'static str,
}

/// The cause of the exact current `AgentRunDecision` producer, with no admission or Editor Session.
pub(super) struct CurrentProducerCause {
    pub(super) semantic_cause: &'static str,
    pub(super) semantic_payload: &'static str,
    pub(super) cause_binding: &'static str,
    pub(super) request_schema: &'static str,
    pub(super) response_schema: &'static str,
    pub(super) recovery_rule: &'static str,
}

/// The route catalog metadata of one registered operation.
#[derive(Clone, Copy)]
pub(super) struct OperationCatalog {
    shape: &'static str,
    semantic_owner: Owner,
    binding: Binding,
    success: &'static [ProjectActivityKind],
    no_effect: &'static [ProjectActivityKind],
    visibility: &'static str,
    notes: Option<&'static str>,
    action_coverage: &'static [&'static str],
    causes: Option<&'static CauseProfiles>,
    query_proof: &'static [(&'static str, &'static str)],
}

impl OperationCatalog {
    pub(super) const fn new(shape: &'static str, semantic_owner: Owner, binding: Binding) -> Self {
        Self {
            shape,
            semantic_owner,
            binding,
            success: &[],
            no_effect: &[],
            visibility: DURABLE_VISIBILITY,
            notes: None,
            action_coverage: &[],
            causes: None,
            query_proof: &[],
        }
    }

    /// A command that its author and its current producer can both send.
    pub(super) const fn conditional_by_cause(
        shape: &'static str,
        semantic_owner: Owner,
        causes: &'static CauseProfiles,
    ) -> Self {
        Self {
            causes: Some(causes),
            ..Self::new(shape, semantic_owner, Binding::CONDITIONAL_BY_CAUSE)
        }
    }

    /// The Project Activity of a settled command; each kind maps to all of its Event schemas.
    pub(super) const fn activity(
        self,
        success: &'static [ProjectActivityKind],
        no_effect: &'static [ProjectActivityKind],
    ) -> Self {
        Self {
            success,
            no_effect,
            ..self
        }
    }

    pub(super) const fn visibility(self, visibility: &'static str) -> Self {
        Self { visibility, ..self }
    }

    pub(super) const fn notes(self, notes: &'static str) -> Self {
        Self {
            notes: Some(notes),
            ..self
        }
    }

    pub(super) const fn action_coverage(self, action_coverage: &'static [&'static str]) -> Self {
        Self {
            action_coverage,
            ..self
        }
    }

    pub(super) const fn query_proof(
        self,
        query_proof: &'static [(&'static str, &'static str)],
    ) -> Self {
        Self {
            query_proof,
            ..self
        }
    }
}

/// Write the route catalog from the registry, the event list, and the catalog sections.
pub(super) fn route_catalog_bytes() -> Vec<u8> {
    let mut catalog = sections::catalog_sections();
    catalog["operations"] = registered_operations().map(operation_json).collect();
    catalog["events"] = events::catalog_events().collect();
    json_bytes(&catalog)
}

fn registered_operations() -> impl Iterator<Item = &'static RegisteredOperation> {
    RELEASE1_OPERATIONS
        .iter()
        .flat_map(|artifacts| artifacts.operations)
}

fn operation_json(registered: &RegisteredOperation) -> Value {
    let operation = registered.operation;
    let catalog = &registered.catalog;
    let statuses = operation
        .responses
        .iter()
        .map(|(status, _)| *status)
        .collect::<Vec<_>>();
    let event_schemas = |kinds: &[ProjectActivityKind]| {
        kinds
            .iter()
            .flat_map(|kind| kind.event_schemas())
            .collect::<Vec<_>>()
    };
    let mut entry = catalog.binding.json(&statuses);
    let fields = json!({
        "operation_id": operation.operation_id, "kind": registered.kind.as_str(),
        "method": operation.method, "path": operation.path, "shape": catalog.shape,
        "semantic_owner": catalog.semantic_owner, "wire_owner": WIRE_OWNER,
        "schemas": {
            "request": operation.request_schema, "response": operation.response_schema,
            "request_source": WIRE_OWNER, "response_source": WIRE_OWNER,
            "semantic_source": catalog.semantic_owner,
        },
        "preconditions": registered.graph.preconditions(),
        "activity": {
            "success": event_schemas(catalog.success), "no_effect": event_schemas(catalog.no_effect),
            "pre_admission": "none", "visibility": catalog.visibility,
        },
        "fixtures": { "positive": operation.fixtures[0], "negative": &operation.fixtures[1..] },
    });
    extend(&mut entry, fields);
    if let Some(notes) = catalog.notes {
        entry["notes"] = json!(notes);
    }
    if !catalog.action_coverage.is_empty() {
        entry["action_coverage"] = json!(catalog.action_coverage);
    }
    if let Some(causes) = catalog.causes {
        entry["cause_discriminator"] = json!({
            "field": "cause", "values": ["author", "current_producer"],
            "semantic_owner": catalog.semantic_owner,
        });
        entry["cause_profiles"] = cause_json(causes, &statuses);
    }
    if !catalog.query_proof.is_empty() {
        entry["query_proof"] = catalog
            .query_proof
            .iter()
            .map(|(key, value)| ((*key).to_owned(), json!(value)))
            .collect();
    }
    entry
}

fn cause_json(causes: &CauseProfiles, http_statuses: &[u16]) -> Value {
    let CauseProfiles {
        author,
        current_producer: producer,
    } = causes;
    let profile = |binding: Binding, fields: Value| {
        let mut profile = binding.json(http_statuses);
        extend(&mut profile, fields);
        extend(
            &mut profile,
            json!({ "request_schema_source": WIRE_OWNER, "response_schema_source": WIRE_OWNER }),
        );
        profile
    };
    json!({
        "author": profile(Binding::EDITOR_COMMAND, json!({
            "semantic_cause": "AuthorCommandAdmission { action_class: explicit_editor_command }",
            "semantic_payload": author.semantic_payload,
            "cause_binding": "protected_author_command_admission",
            "request_schema": author.request_schema, "response_schema": author.response_schema,
            "recovery": { "requires_reconfirmation": "allowed", "rule": AUTHOR_RECOVERY_RULE },
        })),
        "current_producer": profile(Binding::CURRENT_PRODUCER_CAUSE, json!({
            "semantic_cause": producer.semantic_cause, "semantic_payload": producer.semantic_payload,
            "cause_binding": producer.cause_binding, "producer_types": ["AgentRunDecision"],
            "request_schema": producer.request_schema, "response_schema": producer.response_schema,
            "recovery": { "requires_reconfirmation": "forbidden", "rule": producer.recovery_rule },
        })),
    })
}

fn extend(object: &mut Value, fields: Value) {
    if let (Value::Object(object), Value::Object(fields)) = (object, fields) {
        object.extend(fields);
    }
}

#[cfg(test)]
#[path = "release1_route_catalog_tests.rs"]
mod tests;
