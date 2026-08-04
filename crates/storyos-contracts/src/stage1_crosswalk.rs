use std::fmt::{self, Write as _};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::stage1_selection::{
    BASELINE_COMMIT, BASELINE_TREE, CONTRACT_REVISION, ISSUE_BODY_SHA256, OPERATIONS,
    OperationExpectation, PERSISTENCE_CATALOG_PATH, ROUTE_CATALOG_PATH, SOURCE_PATHS,
};

/// Checked-in deterministic output owned by this module.
pub const GENERATED_CROSSWALK_PATH: &str = "generated/evidence/stage1-contract-crosswalk.json";

#[derive(Debug)]
pub enum ManifestError {
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

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Json { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Invalid(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ManifestError {
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
    stack_layer: StackLayer,
    execution_contract: ExecutionContract,
    claim_ceiling: &'static str,
    sources: Vec<SourceBinding>,
    catalogs: Vec<CatalogBinding>,
    requirements: Vec<RequirementBinding>,
    operations: Vec<OperationBinding>,
    proof_selection: ProofSelection,
}

#[derive(Debug, Serialize)]
struct StackLayer {
    position: u8,
    name: &'static str,
    predecessor: &'static str,
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

#[derive(Debug, Serialize)]
struct RequirementBinding {
    id: &'static str,
    owners: &'static [&'static str],
    project_scope: &'static str,
}

#[derive(Debug, Serialize, Eq, PartialEq)]
struct OperationBinding {
    operation_id: &'static str,
    kind: &'static str,
    method: &'static str,
    path: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
    semantic_owner: &'static str,
    error_profile: &'static str,
    project_scope: &'static str,
}

impl From<OperationExpectation> for OperationBinding {
    fn from(operation: OperationExpectation) -> Self {
        Self {
            operation_id: operation.operation_id,
            kind: operation.kind,
            method: operation.method,
            path: operation.path,
            request_schema: operation.request_schema,
            response_schema: operation.response_schema,
            semantic_owner: operation.semantic_owner,
            error_profile: operation.error_profile,
            project_scope: operation.project_scope,
        }
    }
}

#[derive(Debug, Serialize)]
struct ProofSelection {
    gates: &'static [&'static str],
    evidence_classes: &'static [&'static str],
    fixtures: &'static [&'static str],
    fault_points: &'static [&'static str],
    schedules: &'static [&'static str],
    oracles: &'static [&'static str],
    bundles: &'static [&'static str],
    aggregate: &'static str,
    disposition: &'static str,
}

/// Build the deterministic Stage 1 contract crosswalk bytes.
pub fn generate_crosswalk(repo_root: &Path) -> Result<Vec<u8>, ManifestError> {
    let route_catalog = read_json(repo_root, ROUTE_CATALOG_PATH)?;
    let persistence_catalog = read_json(repo_root, PERSISTENCE_CATALOG_PATH)?;
    let operations = validate_operations(&route_catalog)?;
    let mut bytes = serde_json::to_vec_pretty(&Crosswalk {
        schema_id: "storyos.evidence.stage1-contract-crosswalk.v1",
        stack_layer: StackLayer {
            position: 1,
            name: "contract-verification-spine",
            predecessor: "main@af50f2340a8ef0775fc344bc374390fe1d355e49",
        },
        execution_contract: ExecutionContract {
            issue: "https://github.com/FrankQDWang/StoryOS/issues/100",
            revision: CONTRACT_REVISION,
            baseline_commit: BASELINE_COMMIT,
            baseline_tree: BASELINE_TREE,
            issue_body_sha256: ISSUE_BODY_SHA256,
        },
        claim_ceiling: "contract-foundation-only; no S1 requirement or stage acceptance",
        sources: source_bindings(repo_root)?,
        catalogs: vec![
            catalog_binding(repo_root, ROUTE_CATALOG_PATH, &route_catalog)?,
            catalog_binding(repo_root, PERSISTENCE_CATALOG_PATH, &persistence_catalog)?,
        ],
        requirements: requirement_bindings(),
        operations,
        proof_selection: proof_selection(),
    })
    .map_err(|source| ManifestError::Json {
        path: repo_root.join(GENERATED_CROSSWALK_PATH),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Write the deterministic Stage 1 contract crosswalk to its checked-in path.
pub fn write_crosswalk(repo_root: &Path) -> Result<(), ManifestError> {
    let path = repo_root.join(GENERATED_CROSSWALK_PATH);
    let parent = path.parent().ok_or_else(|| {
        ManifestError::Invalid(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| ManifestError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    fs::write(&path, generate_crosswalk(repo_root)?)
        .map_err(|source| ManifestError::Io { path, source })
}

/// Check that the checked-in crosswalk equals a fresh deterministic generation.
pub fn check_crosswalk(repo_root: &Path) -> Result<(), ManifestError> {
    let path = repo_root.join(GENERATED_CROSSWALK_PATH);
    let actual = fs::read(&path).map_err(|source| ManifestError::Io {
        path: path.clone(),
        source,
    })?;
    let expected = generate_crosswalk(repo_root)?;
    if actual != expected {
        return Err(ManifestError::Invalid(format!(
            "{} is stale; run `make generate-contracts`",
            path.display()
        )));
    }
    Ok(())
}

fn source_bindings(repo_root: &Path) -> Result<Vec<SourceBinding>, ManifestError> {
    SOURCE_PATHS
        .iter()
        .map(|path| {
            Ok(SourceBinding {
                path,
                sha256: file_sha256(repo_root, path)?,
            })
        })
        .collect()
}

fn catalog_binding(
    repo_root: &Path,
    path: &'static str,
    catalog: &Value,
) -> Result<CatalogBinding, ManifestError> {
    Ok(CatalogBinding {
        path,
        catalog_id: required_string(catalog, "catalog_id")?.to_owned(),
        contract_revision: required_string(catalog, "contract_revision")?.to_owned(),
        sha256: file_sha256(repo_root, path)?,
    })
}

fn read_json(repo_root: &Path, relative_path: &str) -> Result<Value, ManifestError> {
    let path = repo_root.join(relative_path);
    let bytes = read_normalized_file(&path)?;
    serde_json::from_slice(&bytes).map_err(|source| ManifestError::Json { path, source })
}

fn read_normalized_file(path: &Path) -> Result<Vec<u8>, ManifestError> {
    let bytes = fs::read(path).map_err(|source| ManifestError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if bytes.contains(&b'\r') || !bytes.ends_with(b"\n") {
        return Err(ManifestError::Invalid(format!(
            "{} must use UTF-8/LF with a final LF",
            path.display()
        )));
    }
    std::str::from_utf8(&bytes).map_err(|error| {
        ManifestError::Invalid(format!("{} is not UTF-8: {error}", path.display()))
    })?;
    Ok(bytes)
}

fn file_sha256(repo_root: &Path, relative_path: &str) -> Result<String, ManifestError> {
    let path = repo_root.join(relative_path);
    let digest = Sha256::digest(read_normalized_file(&path)?);
    let mut encoded = String::with_capacity("sha256:".len() + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(encoded)
}

fn required_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, ManifestError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| ManifestError::Invalid(format!("catalog field {field:?} must be a string")))
}

fn validate_operations(catalog: &Value) -> Result<Vec<OperationBinding>, ManifestError> {
    let operations = catalog
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ManifestError::Invalid("route catalog operations must be an array".into())
        })?;
    OPERATIONS
        .iter()
        .copied()
        .map(|expected| {
            let actual = operations
                .iter()
                .find(|operation| {
                    operation.get("operation_id").and_then(Value::as_str)
                        == Some(expected.operation_id)
                })
                .ok_or_else(|| {
                    ManifestError::Invalid(format!(
                        "missing Stage 1 operation {}",
                        expected.operation_id
                    ))
                })?;
            validate_operation(actual, expected)?;
            Ok(expected.into())
        })
        .collect()
}

fn validate_operation(
    operation: &Value,
    expected: OperationExpectation,
) -> Result<(), ManifestError> {
    let schemas = operation.get("schemas").ok_or_else(|| {
        ManifestError::Invalid(format!("{} has no schemas", expected.operation_id))
    })?;
    let actual = [
        (
            "kind",
            operation.get("kind").and_then(Value::as_str),
            expected.kind,
        ),
        (
            "method",
            operation.get("method").and_then(Value::as_str),
            expected.method,
        ),
        (
            "path",
            operation.get("path").and_then(Value::as_str),
            expected.path,
        ),
        (
            "request_schema",
            schemas.get("request").and_then(Value::as_str),
            expected.request_schema,
        ),
        (
            "response_schema",
            schemas.get("response").and_then(Value::as_str),
            expected.response_schema,
        ),
        (
            "semantic_owner",
            operation.get("semantic_owner").and_then(Value::as_str),
            expected.semantic_owner,
        ),
        (
            "error_profile",
            operation.get("error_profile").and_then(Value::as_str),
            expected.error_profile,
        ),
    ];
    if let Some((field, found, wanted)) = actual
        .into_iter()
        .find(|(_, found, wanted)| *found != Some(*wanted))
    {
        return Err(ManifestError::Invalid(format!(
            "{} {field} drifted: expected {wanted:?}, found {found:?}",
            expected.operation_id
        )));
    }
    Ok(())
}

fn requirement_bindings() -> Vec<RequirementBinding> {
    vec![
        requirement(
            "S1-REQ-001",
            &["OWN-WEB", "OWN-PROTO", "OWN-CORE", "OWN-PG", "OWN-TRUST"],
        ),
        requirement("S1-REQ-002", &["OWN-WEB"]),
        requirement(
            "S1-REQ-003",
            &["OWN-ADM", "OWN-CORE", "OWN-PROTO", "OWN-PG"],
        ),
        requirement("S1-REQ-004", &["OWN-WEB", "OWN-ADM", "OWN-CORE", "OWN-RET"]),
        requirement(
            "S1-REQ-005",
            &["OWN-PROTO", "OWN-PG", "OWN-ADM", "OWN-TRUST"],
        ),
        requirement(
            "S1-REQ-006",
            &[
                "OWN-GOV",
                "OWN-WEB",
                "OWN-PROTO",
                "OWN-CORE",
                "OWN-PG",
                "OWN-ADM",
                "OWN-RET",
            ],
        ),
        requirement(
            "S1-JRN-001",
            &["OWN-REL", "OWN-WEB", "OWN-CORE", "OWN-ADM", "OWN-PG"],
        ),
        requirement("S1-EVD-001", &["OWN-REL", "OWN-DVG"]),
        requirement("S1-EVD-002", &["OWN-WEB"]),
        requirement("S1-EVD-003", &["OWN-ADM", "OWN-CORE", "OWN-PG"]),
        requirement("S1-EVD-004", &["OWN-WEB", "OWN-ADM", "OWN-CORE", "OWN-RET"]),
        requirement("S1-EVD-005", &["OWN-TRUST", "OWN-PROTO", "OWN-PG"]),
        requirement("S1-EVD-006", &["OWN-GOV"]),
        requirement("SMAP-STAGE-1", &["OWN-DVG", "OWN-REL"]),
    ]
}

fn requirement(id: &'static str, owners: &'static [&'static str]) -> RequirementBinding {
    RequirementBinding {
        id,
        owners,
        project_scope: "exact-owner-user-id-and-project-id where applicable",
    }
}

fn proof_selection() -> ProofSelection {
    ProofSelection {
        gates: &[
            "DVG-01", "DVG-02", "DVG-03", "DVG-07", "DVG-08", "DVG-09", "DVG-11", "DVG-13",
        ],
        evidence_classes: &[
            "EC-01", "EC-02", "EC-03", "EC-04", "EC-05", "EC-07", "EV-CP", "EV-IT", "EV-INT",
            "EV-SE",
        ],
        fixtures: &[
            "FX-CONTRACT-R1",
            "FX-CORE-PROPOSAL",
            "FX-EDITOR-IME",
            "FX-HANDOFF",
            "FX-JOURNAL-GROUP",
            "FX-RECOVERY-EDITOR",
            "FX-SCOPE-2U2P",
        ],
        fault_points: &[
            "CFP-ADMISSION-BEFORE-CORE",
            "CFP-ADMISSION-EXPIRY",
            "CFP-CONTRACT-DRIFT",
            "CFP-CORE-AFTER-COMMIT-BEFORE-ACK",
            "CFP-CORE-BEFORE-COMMIT",
            "CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE",
            "CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP",
            "CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK",
            "CFP-EDITOR-BEFORE-GROUP-ADMISSION",
            "CFP-EDITOR-BEFORE-JOURNAL-DURABILITY",
            "CFP-FENCE-AFTER-TAKEOVER",
            "CFP-LATE-RESULT",
            "CFP-REPLAY-AFTER-GENERATION-SNAPSHOT",
            "CFP-REPLAY-BELOW-FLOOR",
            "CFP-SCOPE-BEFORE-QUERY",
        ],
        schedules: &[
            "SCH-CRASH",
            "SCH-DRIFT",
            "SCH-FENCE",
            "SCH-NORMAL",
            "SCH-REORDER",
            "SCH-REPLAY",
            "SCH-SCOPE",
        ],
        oracles: &[
            "ORC-ATOMIC-AUTHORITY",
            "ORC-CONTRACT",
            "ORC-CROSSWALK-COMPLETENESS",
            "ORC-EDITOR-JOURNAL",
            "ORC-NEGATIVE-CLOSURE",
            "ORC-OUTCOME-UNKNOWN",
            "ORC-RECOVERY-ATOMICITY",
            "ORC-REPLAY-TRUTH",
            "ORC-SCOPE",
        ],
        bundles: &[
            "B-CONTRACT",
            "B-SCOPE",
            "B-EDITOR",
            "B-CORE",
            "B-RECOVERY",
            "B-REPLAY",
            "B-HANDOFF",
        ],
        aggregate: "B-S1-MANDATORY-SET",
        disposition: "foundation-only; no PASS-STAGE claim",
    }
}

#[cfg(test)]
#[path = "stage1_crosswalk_tests.rs"]
mod tests;
