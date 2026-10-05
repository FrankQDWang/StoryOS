//! One admission and acknowledgement sequence for the project command routes.

use axum::body::to_bytes;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use storyos_application::{
    AuthorCommandAdmissionIds, ChallengeRateClass, EditorClientBinding, Project,
    ProjectCommandChallengeBinding, ProjectCommandEnvelope, ProjectCommandError,
    StructureAuthority,
};
use storyos_core::ReceiptResult;

use super::editor_session::{exact_header, session_binding_ref};
use super::project_command_challenge::{
    hex_bytes, plain_digest, valid_uuid_v7, validate_json_content_type,
};
use super::*;

/// The fixed public identity and admission options of one project command route.
pub(super) struct ProjectCommandRoute {
    pub(super) display_name: &'static str,
    pub(super) command_kind: &'static str,
    pub(super) method: &'static str,
    pub(super) path: &'static str,
    pub(super) schema_id: &'static str,
    pub(super) digest_profile: &'static str,
    pub(super) revision_mismatch: RevisionMismatch,
    pub(super) schema_mismatch: SchemaMismatch,
    pub(super) body_validation: BodyValidation,
    pub(super) target_validation: TargetValidation,
    pub(super) problem_mapping: ProblemMapping,
}

/// The problem of a client contract or security policy revision that differs from the session.
pub(super) enum RevisionMismatch {
    InvalidRequest,
    AuthenticationRequired,
}

/// When the command-specific body fields are validated, relative to the revision check.
pub(super) enum BodyValidation {
    AfterRevisionCheck,
    BeforeRevisionCheck,
}

/// When the path identities after the Project are validated, relative to the content type.
pub(super) enum TargetValidation {
    BeforeContentType,
    AfterContentType,
}

/// The problem of a command schema that differs from the route schema.
pub(super) enum SchemaMismatch {
    InvalidRequest,
    CommandTargetRefused,
}

/// The problems that a route returns for a `ProjectCommandError`.
pub(super) enum ProblemMapping {
    /// The four shared problems, with messages that name the route display name.
    Standard,
    Route(fn(ProjectCommandError) -> ApiError),
}

/// The admission fields that every project command body carries.
pub(super) trait ProjectCommandRequest: DeserializeOwned + Serialize {
    fn command_schema(&self) -> &str;
    fn client_contract_revision(&self) -> &str;
    fn security_policy_revision(&self) -> &str;
    fn correlation_id(&self) -> &str;

    /// Whether the body targets the route command. A route can also require its editor contract.
    fn targets_route(&self, schema_id: &str) -> bool {
        self.command_schema() == schema_id
    }
}

macro_rules! project_command_request {
    ($request:ty, nested $input:ident) => {
        impl ProjectCommandRequest for $request {
            fn command_schema(&self) -> &str {
                &self.command_schema
            }

            fn client_contract_revision(&self) -> &str {
                &self.$input.client_contract_revision
            }

            fn security_policy_revision(&self) -> &str {
                &self.$input.security_policy_revision
            }

            fn correlation_id(&self) -> &str {
                &self.$input.correlation_id
            }
        }
    };
}

project_command_request!(contracts::CreateVolumeRequest, nested create_volume_input);
project_command_request!(contracts::UpdateVolumeRequest, nested update_volume_input);
project_command_request!(contracts::DeleteVolumeRequest, nested delete_volume_input);
project_command_request!(contracts::CreateChapterRequest, nested create_chapter_input);
project_command_request!(contracts::UpdateChapterRequest, nested update_chapter_input);
project_command_request!(contracts::DeleteChapterRequest, nested delete_chapter_input);
project_command_request!(contracts::UpdateProjectRequest, nested update_project_input);
project_command_request!(contracts::ArchiveProjectRequest, nested archive_project_input);
project_command_request!(contracts::SetCurrentChapterRequest, nested set_current_chapter_input);
project_command_request!(
    contracts::UpdateProjectAssistanceRequest,
    nested update_project_assistance_input
);
project_command_request!(
    contracts::ReopenWithdrawnProposalRequest,
    nested reopen_withdrawn_proposal_input
);

impl ProjectCommandRequest for contracts::TakeOverProjectWriterRequest {
    fn command_schema(&self) -> &str {
        &self.command_schema
    }

    fn client_contract_revision(&self) -> &str {
        &self.client_contract_revision
    }

    fn security_policy_revision(&self) -> &str {
        &self.security_policy_revision
    }

    fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    fn targets_route(&self, schema_id: &str) -> bool {
        self.command_schema == schema_id
            && self.editor_contract_revision == contracts::EDITOR_CONTRACT_REVISION
    }
}

/// One admitted project command, ready for its Core Transition.
pub(super) struct Admitted<I> {
    pub(super) store: PostgresProjectReader,
    pub(super) envelope: ProjectCommandEnvelope,
    pub(super) input: I,
    pub(super) digest_hex: String,
}

/// The acknowledgement fields that every project command response carries.
pub(super) struct Acknowledgement {
    pub(super) correlation_id: String,
    pub(super) project_scope: contracts::ProjectScope,
    pub(super) command_id: String,
    pub(super) author_command_admission_id: String,
    pub(super) receipt: contracts::DomainReceipt,
}

/// Authenticates one project command request and binds it to its Command Challenge.
///
/// `targets` are the path identities after the Project. `input` validates and parses the
/// command-specific body fields.
pub(super) async fn admit<R: ProjectCommandRequest, I, F: FnOnce(&R) -> Result<I, ApiError>>(
    state: &ServerState,
    project_id: &str,
    targets: &[&str],
    request: Request,
    route: &ProjectCommandRoute,
    input: F,
) -> Result<Admitted<I>, ApiError> {
    let (parts, body_stream) = request.into_parts();
    let headers = parts.headers;
    let scope = authenticate_scope(
        state,
        &headers,
        project_id,
        RequestOriginPolicy::StateChanging,
    )?;
    let validate_targets = || targets.iter().try_for_each(|target| valid_uuid(target));
    match route.target_validation {
        TargetValidation::BeforeContentType => {
            validate_targets()?;
            validate_json_content_type(&headers)?;
        }
        TargetValidation::AfterContentType => {
            validate_json_content_type(&headers)?;
            validate_targets()?;
        }
    }
    let bytes = to_bytes(body_stream, contracts::AUTHOR_EDIT_MAX_WIRE_BODY_BYTES)
        .await
        .map_err(|_| payload_too_large())?;
    let body = serde_json::from_slice::<R>(&bytes).map_err(|_| invalid_request_shape())?;
    let session_handle = session_cookie(&headers).ok_or_else(authentication_required)?;
    let session = state
        .client_session_binding(session_handle)
        .ok_or_else(authentication_required)?;
    if !body.targets_route(route.schema_id) {
        return Err(match route.schema_mismatch {
            SchemaMismatch::InvalidRequest => invalid_request(),
            SchemaMismatch::CommandTargetRefused => command_target_refused(),
        });
    }
    let check_revision = || {
        if body.client_contract_revision() != session.client_contract_revision
            || body.security_policy_revision() != session.security_policy_revision
        {
            return Err(match route.revision_mismatch {
                RevisionMismatch::InvalidRequest => invalid_request(),
                RevisionMismatch::AuthenticationRequired => authentication_required(),
            });
        }
        Ok(())
    };
    let validate_body = |input: F| {
        let input = input(&body)?;
        valid_uuid(body.correlation_id())?;
        Ok::<I, ApiError>(input)
    };
    let input = match route.body_validation {
        BodyValidation::AfterRevisionCheck => {
            check_revision()?;
            validate_body(input)?
        }
        BodyValidation::BeforeRevisionCheck => {
            let input = validate_body(input)?;
            check_revision()?;
            input
        }
    };
    let idempotency_key = exact_header(&headers, "idempotency-key")?;
    let nonce = exact_header(&headers, "x-storyos-anti-forgery")?;
    if !valid_uuid_v7(idempotency_key)
        || nonce.len() != 64
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid_request());
    }
    let secret = state
        .config
        .project_command_challenge_secret
        .as_deref()
        .filter(|secret| secret.len() >= 32)
        .ok_or_else(challenge_store_unavailable)?;
    let binding_ref = session_binding_ref(secret, session_handle);
    let canonical_command_bytes = serde_json::to_value(&body)
        .and_then(|value| serde_json::to_vec(&value))
        .map_err(|_| invalid_request_shape())?;
    let digest_hex = hex_bytes(&Sha256::digest(&canonical_command_bytes));
    let store = project_reader(state).await?;
    let envelope = ProjectCommandEnvelope {
        project_scope: scope.clone(),
        client_binding: EditorClientBinding {
            binding_ref: binding_ref.clone(),
            session_generation: session.session_generation,
            client_contract_revision: session.client_contract_revision.clone(),
            security_policy_revision: session.security_policy_revision.clone(),
        },
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope: scope,
            client_session_binding_digest: binding_ref,
            client_session_generation: session.session_generation,
            client_contract_revision: session.client_contract_revision.clone(),
            security_policy_revision: session.security_policy_revision.clone(),
            limit_profile_revision: contracts::LIMIT_PROFILE_REVISION.to_owned(),
            challenge_rate_policy_revision: ChallengeRateClass::for_command_kind(
                route.command_kind,
            )
            .policy_revision()
            .to_owned(),
            method: route.method.to_owned(),
            route_template: route.path.to_owned(),
            command_schema: body.command_schema().to_owned(),
            command_kind: route.command_kind.to_owned(),
            canonical_command_digest: format!("sha256:{}:{digest_hex}", route.digest_profile),
            idempotency_key: idempotency_key.to_owned(),
        },
        nonce_digest: plain_digest(nonce.as_bytes()),
        canonical_command_bytes,
        correlation_id: body.correlation_id().to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: Uuid::now_v7().to_string(),
            author_command_admission_id: Uuid::now_v7().to_string(),
            receipt_id: Uuid::now_v7().to_string(),
        },
    };
    Ok(Admitted {
        store,
        envelope,
        input,
        digest_hex,
    })
}

impl<I> Admitted<I> {
    pub(super) async fn hold_first_acknowledgement(&self) {
        super::acknowledgement_hold::hold_first_acknowledgement_if_requested(
            &self.envelope.challenge_binding.idempotency_key,
        )
        .await;
    }

    /// The Domain Receipt and request identities of one settlement.
    pub(super) fn acknowledgement(
        &self,
        route: &ProjectCommandRoute,
        receipt_kind: contracts::DomainReceiptCommandKind,
        settled: SettledReceipt,
    ) -> Acknowledgement {
        let project_scope = contract_scope(&self.envelope.project_scope);
        let (commit_ids, action_sequence) = match settled.authority {
            Some(authority) => (
                authority.authoritative_commit_ids,
                Some(authority.author_action_sequence.to_string()),
            ),
            None => (Vec::new(), None),
        };
        Acknowledgement {
            correlation_id: self.envelope.correlation_id.clone(),
            project_scope: project_scope.clone(),
            command_id: settled.ids.command_id,
            author_command_admission_id: settled.ids.author_command_admission_id.clone(),
            receipt: contracts::DomainReceipt {
                receipt_id: settled.ids.receipt_id,
                project_scope,
                command_kind: receipt_kind,
                command_digest: contracts::DigestValue {
                    algorithm: contracts::DigestAlgorithm::Sha256,
                    profile: route.digest_profile.to_owned(),
                    value_hex_lowercase: self.digest_hex.clone(),
                },
                idempotency_key: self.envelope.challenge_binding.idempotency_key.clone(),
                producer_cause: contracts::DomainReceiptProducerCause::AuthorCommandAdmission,
                author_command_admission_id: settled.ids.author_command_admission_id,
                expected_heads: settled.heads.clone(),
                prior_heads: settled.heads.clone(),
                resulting_heads: settled.heads,
                authoritative_revision_ids: Vec::new(),
                proposal_revision_ids: Vec::new(),
                authoritative_commit_ids: commit_ids,
                author_action_sequence: action_sequence,
                draft_artifact_refs: Vec::new(),
                artifact_lifecycle_event_refs: Vec::new(),
                condition_refs: Vec::new(),
                result: match settled.result {
                    ReceiptResult::AuthoritativeApplied => {
                        contracts::DomainReceiptResult::AuthoritativeApplied
                    }
                    ReceiptResult::NoEffect => contracts::DomainReceiptResult::NoEffect,
                    ReceiptResult::Conflicted => contracts::DomainReceiptResult::Conflicted,
                    ReceiptResult::Refused => contracts::DomainReceiptResult::Refused,
                },
                created_at: settled.receipt_created_at,
            },
        }
    }
}

/// The public form of one Command-response Project.
pub(super) fn controlled_project(project: Project) -> contracts::ControlledProject {
    contracts::ControlledProject {
        project_id: project.project_id.as_ref().to_owned(),
        title: project.title,
        open: match project.current_chapter_id {
            Some(chapter_id) => contracts::ProjectOpenState::CurrentChapter {
                current_chapter_id: chapter_id.as_ref().to_owned(),
            },
            None => contracts::ProjectOpenState::Empty,
        },
    }
}

/// The settlement facts that the Domain Receipt shows.
pub(super) struct SettledReceipt {
    pub(super) ids: AuthorCommandAdmissionIds,
    pub(super) receipt_created_at: String,
    pub(super) result: ReceiptResult,
    pub(super) authority: Option<ReceiptAuthority>,
    /// The one head that the expected, prior, and resulting head arrays show. It can be empty.
    pub(super) heads: Vec<String>,
}

/// The Authoritative Commit identities and Author Action that an applied Receipt shows.
pub(super) struct ReceiptAuthority {
    pub(super) authoritative_commit_ids: Vec<String>,
    pub(super) author_action_sequence: u64,
}

impl From<StructureAuthority> for ReceiptAuthority {
    fn from(authority: StructureAuthority) -> Self {
        Self {
            authoritative_commit_ids: vec![authority.authoritative_commit_id],
            author_action_sequence: authority.author_action_sequence,
        }
    }
}

impl ProjectCommandRoute {
    pub(super) fn problem(&self, error: ProjectCommandError) -> ApiError {
        let name = match self.problem_mapping {
            ProblemMapping::Standard => self.display_name,
            ProblemMapping::Route(route_problem) => return route_problem(error),
        };
        match error {
            ProjectCommandError::BindingConflict => problem(
                StatusCode::CONFLICT,
                "idempotency_binding_conflict",
                &format!("The {name} binding conflicts."),
            ),
            ProjectCommandError::HistoricalAcknowledgementUnavailable => problem(
                StatusCode::CONFLICT,
                "historical_acknowledgement_unavailable",
                &format!(
                    "The original {name} acknowledgement cannot be recovered. Refresh to inspect the current Project."
                ),
            ),
            ProjectCommandError::InvalidChallenge => problem(
                StatusCode::UNPROCESSABLE_ENTITY,
                "challenge_invalid",
                &format!("The {name} challenge is invalid."),
            ),
            ProjectCommandError::MissingProject => resource_unavailable(),
            ProjectCommandError::Unavailable(_) => problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "project_store_unavailable",
                "The Project store is unavailable.",
            ),
        }
    }
}

/// Parses a positive decimal request field, such as an expected revision or an order.
pub(super) fn positive(value: &str) -> Result<u64, ApiError> {
    value
        .parse::<u64>()
        .ok()
        .filter(|value| *value >= 1)
        .ok_or_else(invalid_request)
}

/// Accepts a Project, Volume, or Chapter title of 1 to 1024 bytes.
pub(super) fn structure_title(title: &str) -> Result<String, ApiError> {
    if title.is_empty() || title.len() > 1024 {
        return Err(invalid_request());
    }
    Ok(title.to_owned())
}
