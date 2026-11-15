//! The row of the Skill list query in the operation table (S6-02c).

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources, UUID,
    raw_schema,
};
use crate::release1_list_skills::{LIST_SKILLS, ListSkillsResponse, SkillCatalogEntry};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};

pub(super) const LIST_SKILLS_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &LIST_SKILLS,
        &["server_derived_project_scope", "project_visibility"],
        OperationCatalog::new("skill:list", Owner::Assistance, Binding::PROJECT_QUERY),
    ),
    summary: "List the Skills that the deployment offers, with no file of a package",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "skills-response",
        raw_schema::<ListSkillsResponse>,
        "StoryOS Skills Response",
        LIST_SKILLS_RESPONSE,
    )),
    typescript: &[SkillCatalogEntry::decl, ListSkillsResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000e01",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.query.skills.response.v1",
      "skills": [
        {
          "description": "Rules for the dialogue of a novel. Use it when the author asks for dialogue.",
          "reference": {
            "name": "dialogue-style-guide",
            "package_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
          }
        }
      ]
    }"#,
    ),
};

const LIST_SKILLS_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        Edit::Pattern("^[a-z0-9]+(-[a-z0-9]+)*$"),
        &["/$defs/SkillPackageReference/properties/name"],
    ),
    Patch(
        Edit::MaxLength(64),
        &["/$defs/SkillPackageReference/properties/name"],
    ),
    Patch(
        Edit::Pattern("^sha256:[0-9a-f]{64}$"),
        &["/$defs/SkillPackageReference/properties/package_digest"],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/SkillCatalogEntry/properties/description"],
    ),
    Patch(
        Edit::MaxLength(1024),
        &["/$defs/SkillCatalogEntry/properties/description"],
    ),
    Patch(Edit::MaxItems(32), &["/properties/skills"]),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
