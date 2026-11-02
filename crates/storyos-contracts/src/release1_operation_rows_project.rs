//! The rows of the Project creation and Project queries in the operation table.

use ts_rs::TS;

use super::{
    DATE_TIME, Edit, Fixture, Headers, NO_PATCHES, OperationRow, Patch, SCHEMA_ID, SchemaSource,
    SchemaSources, U64, UUID, raw_schema,
};
use crate::ProjectActivityKind;
use crate::release1::{
    CREATE_PROJECT_COMMAND_CHALLENGE, CreateProjectCommandChallengeRequest,
    CreateProjectCommandChallengeResponse, GET_PROJECT, GET_PROTOCOL_PROFILE, GetProjectResponse,
    LIMIT_PROFILE_REVISION, Release1CompatibilityIdentity, Release1ProtocolProfile,
};
use crate::release1_create_project::{
    CREATE_PROJECT, CREATE_PROJECT_CHALLENGE, CREATE_PROJECT_DIGEST_PROFILE,
    CREATE_PROJECT_REQUEST_SCHEMA_ID, CreateProjectChallengeRequest,
    CreateProjectChallengeResponse, CreateProjectInput, CreateProjectRequest,
    CreateProjectResponse,
};
use crate::release1_list_projects::{LIST_PROJECTS, ListProjectsResponse, ProjectListItem};
use crate::release1_operation_registry::{ClientOverride, RegisteredOperation};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};

pub(super) const GET_PROTOCOL_PROFILE_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_PROTOCOL_PROFILE,
        &["active_public_release_profile"],
        OperationCatalog::new("protocol:profile", Owner::Protocol, Binding::PROTOCOL_QUERY)
            .notes(GET_PROTOCOL_PROFILE_NOTES),
    )
    .client(ClientOverride::ResponseType("Release1ProtocolProfile")),
    summary: "Discover the active StoryOS Release 1 protocol profile",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::no_parameters(
            "protocol-profile-request",
            "StoryOS Release 1 Protocol Profile Request",
        ),
        SchemaSource::typed(
            "protocol-profile-response",
            raw_schema::<Release1ProtocolProfile>,
            "StoryOS Release 1 Protocol Profile",
            NO_PATCHES,
        ),
    ),
    typescript: &[
        Release1CompatibilityIdentity::decl,
        Release1ProtocolProfile::decl,
    ],
    fixture: Fixture::ProtocolProfile,
};

pub(super) const CREATE_PROJECT_CHALLENGE_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::challenge(
        &CREATE_PROJECT_CHALLENGE,
        &[
            "server_derived_user",
            "prospective_project_scope",
            "strict_origin",
            "protected_client_session_binding",
            "closed_create_project_schema",
            "body_idempotency_key",
        ],
        OperationCatalog::new(
            "challenge:prospective-project",
            Owner::Admission,
            Binding::CHALLENGE,
        )
        .notes(CREATE_PROJECT_CHALLENGE_NOTES),
    ),
    summary: "Issue one prospective Project command challenge",
    headers: Headers::NONE.origin().retry_after(),
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "create-project-challenge-request",
            raw_schema::<CreateProjectChallengeRequest>,
            "StoryOS Create Project Challenge Request",
            CREATE_PROJECT_CHALLENGE_REQUEST,
        ),
        SchemaSource::typed(
            "create-project-challenge-response",
            raw_schema::<CreateProjectChallengeResponse>,
            "StoryOS Create Project Challenge Response",
            CREATE_PROJECT_CHALLENGE_RESPONSE,
        ),
    ),
    typescript: &[
        CreateProjectInput::decl,
        CreateProjectChallengeRequest::decl,
        CreateProjectChallengeResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "canonical_command_digest": {
        "algorithm": "sha256",
        "profile": "storyos.command.createProject.jcs.v1",
        "value_hex_lowercase": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
      },
      "expires_at": "2026-08-23T08:05:00.000Z",
      "limit_profile_revision": "storyos.foundation.absolute.v1",
      "nonce": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
      "prospective_project_id": "018f0000-0000-7001-8000-000000000201"
    }"#,
    ),
};

pub(super) const CREATE_PROJECT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &CREATE_PROJECT,
        &[
            "server_derived_user",
            "prospective_project_scope",
            "strict_origin",
            "protected_client_session_binding",
            "challenge_nonce_record",
            "expected_absent_project",
        ],
        OperationCatalog::new("project:create", Owner::Release, Binding::PROJECT_COMMAND)
            .activity(&[ProjectActivityKind::ProjectCreated], &[])
            .notes("The only command admitted against a prospective Project Scope."),
    )
    .client(ClientOverride::NoDigest),
    summary: "Create one empty Project",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "create-project-request",
            raw_schema::<CreateProjectRequest>,
            "StoryOS Create Project Request",
            CREATE_PROJECT_REQUEST,
        ),
        SchemaSource::typed(
            "create-project-response",
            raw_schema::<CreateProjectResponse>,
            "StoryOS Create Project Response",
            CREATE_PROJECT_RESPONSE,
        ),
    ),
    typescript: &[CreateProjectRequest::decl, CreateProjectResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000212",
      "command_id": "018f0000-0000-7001-8000-000000000211",
      "correlation_id": "018f0000-0000-7001-8000-000000000210",
      "project": {
        "open": {"kind": "empty"},
        "project_id": "018f0000-0000-7001-8000-000000000201",
        "title": "Empty Novel"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "receipt": {
        "artifact_lifecycle_event_refs": [],
        "author_action_sequence": null,
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000212",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.createProject.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "createProject",
        "condition_refs": [],
        "created_at": "2026-08-25T08:05:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": [],
        "idempotency_key": "018f0000-0000-7001-8000-000000000214",
        "prior_heads": [],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000213",
        "result": "authoritative_applied",
        "resulting_heads": []
      },
      "schema_id": "storyos.command.create-project.response.v1"
    }"#,
    ),
};

pub(super) const CREATE_PROJECT_COMMAND_CHALLENGE_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::challenge(
        &CREATE_PROJECT_COMMAND_CHALLENGE,
        &[
            "server_derived_project_scope",
            "strict_origin",
            "protected_client_session_binding",
            "route_method_schema_digest_match",
            "closed_command_schema",
        ],
        OperationCatalog::new(
            "challenge:project-command",
            Owner::Admission,
            Binding::CHALLENGE,
        )
        .notes("Creates no Admission, Receipt, domain Attempt, or authority."),
    ),
    summary: "Issue one exact Project command challenge",
    headers: Headers::NONE.retry_after(),
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "project-command-challenge-request",
            raw_schema::<CreateProjectCommandChallengeRequest>,
            "StoryOS Project Command Challenge Request",
            NO_PATCHES,
        ),
        SchemaSource::typed(
            "project-command-challenge-response",
            raw_schema::<CreateProjectCommandChallengeResponse>,
            "StoryOS Project Command Challenge Response",
            CREATE_PROJECT_COMMAND_CHALLENGE_RESPONSE,
        ),
    ),
    typescript: &[
        CreateProjectCommandChallengeRequest::decl,
        CreateProjectCommandChallengeResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "expires_at": "2026-08-12T08:05:00.000Z",
      "limit_profile_revision": "storyos.foundation.absolute.v1",
      "nonce": "4f9f5ad05c4d1294d4114fb15595d831b64e3f4312a17e639213ad36e941ca71"
    }"#,
    ),
};

pub(super) const LIST_PROJECTS_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &LIST_PROJECTS,
        &["server_derived_user"],
        OperationCatalog::new(
            "project:collection-read",
            Owner::Release,
            Binding::PROJECT_QUERY,
        ),
    ),
    summary: "List Projects owned by the current User",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::no_parameters("project-list-request", "StoryOS Project List Request"),
        SchemaSource::typed(
            "project-list-response",
            raw_schema::<ListProjectsResponse>,
            "StoryOS Project List Response",
            LIST_PROJECTS_RESPONSE,
        ),
    ),
    typescript: &[ProjectListItem::decl, ListProjectsResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000013",
      "owner_user_id": "018f0000-0000-7001-8000-000000000001",
      "projects": [
        {
          "open": {
            "current_chapter_id": "018f0000-0000-7001-8000-000000000003",
            "kind": "current_chapter"
          },
          "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
          },
          "revision": "1",
          "title": "Project A"
        },
        {
          "open": {"kind": "empty"},
          "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000012"
          },
          "revision": "1",
          "title": "Empty Novel"
        }
      ],
      "schema_id": "storyos.query.project-list.response.v1"
    }"#,
    ),
};

pub(super) const GET_PROJECT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_PROJECT,
        &["server_derived_project_scope", "project_visibility"],
        OperationCatalog::new(
            "project:read",
            Owner::Release,
            Binding::CURRENT_PROJECT_QUERY,
        ),
    ),
    summary: "Read one controlled StoryOS Project",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::path_parameters("project-request"),
        SchemaSource::typed(
            "project-response",
            raw_schema::<GetProjectResponse>,
            "StoryOS Project Query Response",
            NO_PATCHES,
        ),
    ),
    typescript: &[GetProjectResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000005",
      "project": {
        "open": {
          "current_chapter_id": "018f0000-0000-7001-8000-000000000003",
          "kind": "current_chapter"
        },
        "project_id": "018f0000-0000-7001-8000-000000000002",
        "title": "受控项目"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "schema_id": "storyos.query.project.response.v1"
    }"#,
    ),
};

const GET_PROTOCOL_PROFILE_NOTES: &str =
    "Project-free profile discovery; no Project Scope or session authority is returned.";

const CREATE_PROJECT_CHALLENGE_NOTES: &str = "Allocates only a prospective ProjectId and one challenge-bound idempotency record; it creates no Project, Admission, Receipt, or Core effect.";

const CREATE_PROJECT_CHALLENGE_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/CreateProjectInput/properties/correlation_id",
            "/properties/idempotency_key",
        ],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/CreateProjectInput/properties/title"],
    ),
    Patch(
        Edit::MaxUtf8Bytes(1024),
        &["/$defs/CreateProjectInput/properties/title"],
    ),
    Patch(
        Edit::Const(CREATE_PROJECT_REQUEST_SCHEMA_ID),
        &["/properties/command_schema"],
    ),
];

const CREATE_PROJECT_CHALLENGE_RESPONSE: &[Patch] = &[
    Patch(
        Edit::Const(CREATE_PROJECT_DIGEST_PROFILE),
        &["/$defs/DigestValue/properties/profile"],
    ),
    Patch(
        Edit::Pattern("^[0-9a-f]{64}$"),
        &[
            "/$defs/DigestValue/properties/value_hex_lowercase",
            "/properties/nonce",
        ],
    ),
    Patch(DATE_TIME, &["/properties/expires_at"]),
    Patch(
        Edit::Const(LIMIT_PROFILE_REVISION),
        &["/properties/limit_profile_revision"],
    ),
    Patch(UUID, &["/properties/prospective_project_id"]),
];

const CREATE_PROJECT_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/CreateProjectInput/properties/correlation_id",
            "/properties/prospective_project_id",
        ],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/CreateProjectInput/properties/title"],
    ),
    Patch(
        Edit::MaxUtf8Bytes(1024),
        &["/$defs/CreateProjectInput/properties/title"],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const CREATE_PROJECT_RESPONSE: &[Patch] = &[
    Patch(
        Edit::NullablePattern("^(0|[1-9][0-9]{0,19})$"),
        &["/$defs/DomainReceipt/properties/author_action_sequence"],
    ),
    Patch(
        UUID,
        &[
            "/$defs/DomainReceipt/properties/author_command_admission_id",
            "/$defs/DomainReceipt/properties/idempotency_key",
            "/$defs/DomainReceipt/properties/receipt_id",
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(DATE_TIME, &["/$defs/DomainReceipt/properties/created_at"]),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const CREATE_PROJECT_COMMAND_CHALLENGE_RESPONSE: &[Patch] =
    &[Patch(DATE_TIME, &["/properties/expires_at"])];

const LIST_PROJECTS_RESPONSE: &[Patch] = &[
    Patch(U64, &["/$defs/ProjectListItem/properties/revision"]),
    Patch(
        Edit::MinLength(1),
        &["/$defs/ProjectListItem/properties/title"],
    ),
    Patch(
        Edit::MaxUtf8Bytes(1024),
        &["/$defs/ProjectListItem/properties/title"],
    ),
    Patch(
        UUID,
        &[
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/correlation_id",
            "/properties/owner_user_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
