use super::*;

const STANDARD_ROUTE: ProjectCommandRoute = ProjectCommandRoute {
    display_name: "Update Project",
    command_kind: "updateProject",
    method: "POST",
    path: "/api/v1/projects/{project_id}/updates",
    schema_id: "storyos.command.update-project.request.v1",
    digest_profile: "storyos.command.update-project.digest.v1",
    revision_mismatch: RevisionMismatch::InvalidRequest,
    body_validation: BodyValidation::AfterRevisionCheck,
    target_validation: TargetValidation::BeforeContentType,
    schema_mismatch: SchemaMismatch::InvalidRequest,
    problem_mapping: ProblemMapping::Standard,
};

// ADR 0032: a replay without its stored acknowledgement is a 409, and damaged stored evidence is a 503. The
// Profile Probes do not damage stored evidence (ADR 0075), so this test owns the HTTP answer of both errors.
#[test]
fn a_replay_separates_historical_absence_from_damaged_evidence_at_http() {
    let historical =
        STANDARD_ROUTE.problem(ProjectCommandError::HistoricalAcknowledgementUnavailable);
    let damaged = STANDARD_ROUTE.problem(ProjectCommandError::Unavailable(Box::new(
        std::io::Error::other("damaged acknowledgement evidence"),
    )));

    assert_eq!(
        [
            (historical.0, historical.1.1.code.as_str()),
            (damaged.0, damaged.1.1.code.as_str()),
        ],
        [
            (
                StatusCode::CONFLICT,
                "historical_acknowledgement_unavailable"
            ),
            (StatusCode::SERVICE_UNAVAILABLE, "project_store_unavailable"),
        ]
    );
}
