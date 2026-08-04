use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::stage1_selection::{
    BASELINE_COMMIT, BASELINE_TREE, CONTRACT_REVISION, ISSUE_BODY_SHA256, OPERATION_IDS,
    PERSISTENCE_CATALOG_PATH, ProofSelection, ROUTE_CATALOG_PATH, RequirementBinding, SOURCE_PATHS,
    proof_selection, requirement_bindings,
};

/// Checked-in deterministic output owned by this module.
pub const GENERATED_CROSSWALK_PATH: &str = "generated/evidence/stage1-contract-crosswalk.json";

/// Resolve the repository that owns this build-time contract tool.
pub fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("contracts crate must be nested under the repository root")
}

#[derive(Debug)]
pub enum CrosswalkError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Invalid(String),
}

impl fmt::Display for CrosswalkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Json { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for CrosswalkError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Invalid(_) => None,
        }
    }
}

#[derive(Debug, Serialize)]
struct Crosswalk {
    schema_id: &'static str,
    claim_ceiling: &'static str,
    declarations: Declarations,
    verifications: Verifications,
}

#[derive(Debug, Serialize)]
struct Declarations {
    authority: &'static str,
    execution_contract: ExecutionContract,
    requirements: Vec<RequirementBinding>,
    proof_selection: ProofSelection,
}

#[derive(Debug, Serialize)]
struct Verifications {
    baseline: BaselineBinding,
    sources: Vec<SourceBinding>,
    catalogs: Vec<CatalogBinding>,
    operations: Vec<OperationBinding>,
}

#[derive(Debug, Serialize)]
struct BaselineBinding {
    commit: &'static str,
    tree: &'static str,
}

#[derive(Debug, Serialize)]
struct ExecutionContract {
    issue: &'static str,
    revision: &'static str,
    baseline_commit: &'static str,
    baseline_tree: &'static str,
    issue_body_sha256: &'static str,
}

#[derive(Debug, Serialize)]
struct SourceBinding {
    path: &'static str,
    sha256: String,
}

#[derive(Debug, Serialize)]
struct CatalogBinding {
    path: &'static str,
    catalog_id: String,
    contract_revision: String,
    sha256: String,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
struct OperationBinding {
    operation_id: String,
    kind: OperationKind,
    method: HttpMethod,
    path: String,
    request_schema: String,
    response_schema: String,
    semantic_owner: String,
    error_profile: String,
    project_scope: ProjectScope,
    scope_precondition: String,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum OperationKind {
    Challenge,
    Command,
    Query,
    Stream,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
enum HttpMethod {
    #[serde(rename = "GET")]
    Get,
    #[serde(rename = "POST")]
    Post,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ProjectScope {
    ExactOwnerUserIdAndProjectId,
    ProjectFree,
}

/// Build the deterministic Stage 1 contract crosswalk bytes.
pub fn generate_crosswalk(repo_root: &Path) -> Result<Vec<u8>, CrosswalkError> {
    verify_baseline_tree(repo_root, BASELINE_TREE)?;
    let route_catalog = read_baseline_json(repo_root, ROUTE_CATALOG_PATH)?;
    let persistence_catalog = read_baseline_json(repo_root, PERSISTENCE_CATALOG_PATH)?;
    let operations = validate_operations(&route_catalog)?;
    let mut bytes = serde_json::to_vec_pretty(&Crosswalk {
        schema_id: "storyos.evidence.stage1-contract-crosswalk.v1",
        claim_ceiling: "contract-foundation-only; no S1 requirement or stage acceptance",
        declarations: Declarations {
            authority: "Issue #100 locked body and Claim lock",
            execution_contract: ExecutionContract {
                issue: "https://github.com/FrankQDWang/StoryOS/issues/100",
                revision: CONTRACT_REVISION,
                baseline_commit: BASELINE_COMMIT,
                baseline_tree: BASELINE_TREE,
                issue_body_sha256: ISSUE_BODY_SHA256,
            },
            requirements: requirement_bindings(),
            proof_selection: proof_selection(),
        },
        verifications: Verifications {
            baseline: BaselineBinding {
                commit: BASELINE_COMMIT,
                tree: BASELINE_TREE,
            },
            sources: source_bindings(repo_root)?,
            catalogs: vec![
                catalog_binding(repo_root, ROUTE_CATALOG_PATH, &route_catalog)?,
                catalog_binding(repo_root, PERSISTENCE_CATALOG_PATH, &persistence_catalog)?,
            ],
            operations,
        },
    })
    .map_err(|source| CrosswalkError::Json {
        path: repo_root.join(GENERATED_CROSSWALK_PATH),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Write the deterministic Stage 1 contract crosswalk to its checked-in path.
pub fn write_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    let path = repo_root.join(GENERATED_CROSSWALK_PATH);
    let parent = path.parent().ok_or_else(|| {
        CrosswalkError::Invalid(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| CrosswalkError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    fs::write(&path, generate_crosswalk(repo_root)?)
        .map_err(|source| CrosswalkError::Io { path, source })
}

/// Check that the checked-in crosswalk equals a fresh deterministic generation.
pub fn check_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    let path = repo_root.join(GENERATED_CROSSWALK_PATH);
    let actual = fs::read(&path).map_err(|source| CrosswalkError::Io {
        path: path.clone(),
        source,
    })?;
    let expected = generate_crosswalk(repo_root)?;
    if actual != expected {
        return Err(CrosswalkError::Invalid(format!(
            "{} is stale; run `make generate-contracts`",
            path.display()
        )));
    }
    Ok(())
}

fn source_bindings(repo_root: &Path) -> Result<Vec<SourceBinding>, CrosswalkError> {
    SOURCE_PATHS
        .iter()
        .map(|path| {
            Ok(SourceBinding {
                path,
                sha256: baseline_file_sha256(repo_root, path)?,
            })
        })
        .collect()
}

fn catalog_binding(
    repo_root: &Path,
    path: &'static str,
    catalog: &Value,
) -> Result<CatalogBinding, CrosswalkError> {
    Ok(CatalogBinding {
        path,
        catalog_id: required_string(catalog, "catalog_id")?.to_owned(),
        contract_revision: required_string(catalog, "contract_revision")?.to_owned(),
        sha256: baseline_file_sha256(repo_root, path)?,
    })
}

fn read_baseline_json(repo_root: &Path, relative_path: &str) -> Result<Value, CrosswalkError> {
    let bytes = read_baseline_file(repo_root, relative_path)?;
    serde_json::from_slice(&bytes).map_err(|source| CrosswalkError::Json {
        path: PathBuf::from(format!("{BASELINE_COMMIT}:{relative_path}")),
        source,
    })
}

fn read_baseline_file(repo_root: &Path, relative_path: &str) -> Result<Vec<u8>, CrosswalkError> {
    let object = format!("{BASELINE_COMMIT}:{relative_path}");
    let bytes = git_output(repo_root, &["show", &object])?;
    if bytes.contains(&b'\r') || !bytes.ends_with(b"\n") {
        return Err(CrosswalkError::Invalid(format!(
            "{object} must use UTF-8/LF with a final LF"
        )));
    }
    std::str::from_utf8(&bytes)
        .map_err(|error| CrosswalkError::Invalid(format!("{object} is not UTF-8: {error}")))?;
    Ok(bytes)
}

fn baseline_file_sha256(repo_root: &Path, relative_path: &str) -> Result<String, CrosswalkError> {
    let digest = Sha256::digest(read_baseline_file(repo_root, relative_path)?);
    let mut encoded = String::with_capacity("sha256:".len() + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(encoded)
}

fn verify_baseline_tree(repo_root: &Path, expected: &str) -> Result<(), CrosswalkError> {
    let revision = format!("{BASELINE_COMMIT}^{{tree}}");
    let output = git_output(repo_root, &["rev-parse", &revision])?;
    let actual = std::str::from_utf8(&output)
        .map_err(|error| CrosswalkError::Invalid(format!("git tree output is not UTF-8: {error}")))?
        .trim();
    if actual != expected {
        return Err(CrosswalkError::Invalid(format!(
            "baseline tree drifted: expected {expected}, found {actual}"
        )));
    }
    Ok(())
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<Vec<u8>, CrosswalkError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .map_err(|source| CrosswalkError::Io {
            path: repo_root.join(".git"),
            source,
        })?;
    if !output.status.success() {
        return Err(CrosswalkError::Invalid(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, CrosswalkError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| CrosswalkError::Invalid(format!("catalog field {field:?} must be a string")))
}

fn validate_operations(catalog: &Value) -> Result<Vec<OperationBinding>, CrosswalkError> {
    let operations = catalog
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CrosswalkError::Invalid("route catalog operations must be an array".into())
        })?;
    OPERATION_IDS
        .iter()
        .map(|operation_id| {
            let actual = operations
                .iter()
                .find(|operation| {
                    operation.get("operation_id").and_then(Value::as_str) == Some(operation_id)
                })
                .ok_or_else(|| {
                    CrosswalkError::Invalid(format!("missing Stage 1 operation {operation_id}"))
                })?;
            operation_binding(actual, operation_id)
        })
        .collect()
}

fn operation_binding(
    operation: &Value,
    operation_id: &str,
) -> Result<OperationBinding, CrosswalkError> {
    let schemas = operation
        .get("schemas")
        .ok_or_else(|| CrosswalkError::Invalid(format!("{operation_id} has no schemas")))?;
    let preconditions = operation
        .get("preconditions")
        .and_then(Value::as_array)
        .ok_or_else(|| CrosswalkError::Invalid(format!("{operation_id} has no preconditions")))?;
    let scope_precondition = preconditions
        .iter()
        .find_map(|value| match value.as_str()? {
            value @ ("server_derived_project_scope"
            | "command_scope_join"
            | "snapshot_scope_join") => Some(value),
            _ => None,
        });
    let (project_scope, scope_precondition) = match (operation_id, scope_precondition) {
        ("getProtocolProfile", None)
            if preconditions
                .iter()
                .any(|value| value.as_str() == Some("active_public_release_profile")) =>
        {
            (ProjectScope::ProjectFree, "active_public_release_profile")
        }
        ("getProtocolProfile", _) | (_, None) => return Err(scope_error(operation_id)),
        (_, Some(precondition)) => (ProjectScope::ExactOwnerUserIdAndProjectId, precondition),
    };
    Ok(OperationBinding {
        operation_id: operation_id.to_owned(),
        kind: parse_kind(required_string(operation, "kind")?)?,
        method: parse_method(required_string(operation, "method")?)?,
        path: required_string(operation, "path")?.to_owned(),
        request_schema: required_string(schemas, "request")?.to_owned(),
        response_schema: required_string(schemas, "response")?.to_owned(),
        semantic_owner: required_string(operation, "semantic_owner")?.to_owned(),
        error_profile: required_string(operation, "error_profile")?.to_owned(),
        project_scope,
        scope_precondition: scope_precondition.to_owned(),
    })
}

fn scope_error(operation_id: &str) -> CrosswalkError {
    CrosswalkError::Invalid(format!("{operation_id} Project Scope precondition drifted"))
}

fn parse_kind(value: &str) -> Result<OperationKind, CrosswalkError> {
    match value {
        "challenge" => Ok(OperationKind::Challenge),
        "command" => Ok(OperationKind::Command),
        "query" => Ok(OperationKind::Query),
        "stream" => Ok(OperationKind::Stream),
        value => Err(CrosswalkError::Invalid(format!(
            "unsupported operation kind {value:?}"
        ))),
    }
}

fn parse_method(value: &str) -> Result<HttpMethod, CrosswalkError> {
    match value {
        "GET" => Ok(HttpMethod::Get),
        "POST" => Ok(HttpMethod::Post),
        value => Err(CrosswalkError::Invalid(format!(
            "unsupported HTTP method {value:?}"
        ))),
    }
}

#[cfg(test)]
#[path = "stage1_crosswalk_tests.rs"]
mod tests;
