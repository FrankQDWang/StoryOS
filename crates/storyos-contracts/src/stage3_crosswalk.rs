use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::digest::sha256_prefixed;
use crate::stage1_crosswalk::CrosswalkError;

/// Checked-in deterministic output owned by this module.
pub const GENERATED_STAGE3_CROSSWALK_PATH: &str =
    "generated/evidence/stage3-contract-crosswalk.json";

const SCHEMA_ID: &str = "storyos.evidence.stage3-contract-crosswalk.v1";
const CLAIM_CEILING: &str = "implementation-evidence-completeness; no EV-SR; no PASS-STAGE";
const EVALUATED_STAGE: &str = "Stage 3";
const MANDATORY_MAP: &str = "SMAP-STAGE-3";
const CONTRACT_REVISION: &str = "refused-edit-draft-expand-contract-2026-09-26-v2-journey";
const PARENT_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/361";
const TICKET_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/391";
const BASELINE_COMMIT: &str = "2fa13fa2c21b81b8e727b4553cb92cba55cfa19e";
const BASELINE_TREE: &str = "49179c6a7c5a5b29e8c1f29220081a71153fcc02";
const FORBIDDEN_EMISSIONS: &[&str] = &["EV-SR", "PASS-STAGE", "PASS-CLOUD"];
const REQUIRED_IDS: &[&str] = &[
    "REL-007",
    "S3-REQ-001",
    "S3-REQ-002",
    "S3-REQ-003",
    "S3-REQ-004",
    "S3-REQ-005",
    "S3-REQ-006",
    "S3-REQ-007",
    "S3-REQ-008",
    "S3-EVD-001",
    "S3-EVD-002",
    "S3-EVD-003",
    "S3-EVD-004",
    "S3-EVD-005",
    "S3-EVD-006",
    "S3-EVD-007",
    "S3-EVD-008",
    "S3-JRN-001",
    "S3-JRN-001/1",
    "S3-JRN-001/2",
    "S3-JRN-001/3",
    "S3-JRN-001/4",
    "S3-JRN-001/5",
    "S3-JRN-001/6",
    "S3-JRN-001/7",
];
const SOURCE_PATHS: &[&str] = &[
    "docs/foundation/ai-independent-editor-first-release-baseline-and-handoff-criteria.md",
    "docs/foundation/deterministic-verification-and-failure-recovery-gates.md",
    "docs/foundation/product-delivery-proof-selection.md",
];

const PROOF: &str = "docs/foundation/product-delivery-proof-selection.md";
const WORKSPACE: &str = "apps/web/test/browser-exact-dist/s2-workspace.integration.test.ts";
const STAGE2_JOURNEY: &str = "apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts";
const PHYSICAL_DRILL: &str =
    "apps/web/test/browser-exact-dist/s2-physical-drill.integration.test.ts";
const PRODUCTION_HOST: &str =
    "apps/web/test/browser-exact-dist/production-host.integration.test.ts";
const FAKE_DECISION: &str =
    "apps/web/test/node-postgresql/complete-fake-model-decision-http.integration.test.ts";
const CREATE_RUN: &str = "apps/web/test/node-postgresql/create-agent-run-http.integration.test.ts";
const CONTINUE: &str =
    "apps/web/test/node-postgresql/continue-conversation-input-http.integration.test.ts";
const COMPACT: &str =
    "apps/web/test/node-postgresql/compact-active-context-http.integration.test.ts";
const EXPIRY: &str =
    "apps/web/test/node-postgresql/rebuild-expired-reference-http.integration.test.ts";
const LOOKUP: &str =
    "apps/web/test/node-postgresql/retrieve-original-result-http.integration.test.ts";
const SUCCESSOR: &str =
    "apps/web/test/node-postgresql/unknown-create-successor-http.integration.test.ts";
const CANCEL: &str =
    "apps/web/test/node-postgresql/recover-or-cancel-agent-run-http.integration.test.ts";
const INLINE: &str = "apps/web/test/node-postgresql/edit-inline-proposal-http.integration.test.ts";
const OPEN_BLOCK: &str =
    "apps/web/test/node-postgresql/open-block-proposal-http.integration.test.ts";
const MULTI: &str =
    "apps/web/test/node-postgresql/settle-multi-operation-selections-http.integration.test.ts";
const STREAM: &str =
    "apps/web/test/node-postgresql/stream-proposal-generation-http.integration.test.ts";
const PARTIAL: &str =
    "apps/web/test/node-postgresql/complete-ready-partial-proposal-http.integration.test.ts";
const CONTINUE_GENERATION: &str =
    "apps/web/test/node-postgresql/continue-proposal-generation-http.integration.test.ts";
const ACCEPT: &str = "apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts";
const REJECT: &str =
    "apps/web/test/node-postgresql/reject-proposal-operations-http.integration.test.ts";
const UNDO: &str = "apps/web/test/node-postgresql/undo-acceptance-http.integration.test.ts";
const WITHDRAW: &str = "apps/web/test/node-postgresql/withdraw-proposal-http.integration.test.ts";
const COMPARISON: &str = "crates/storyos-core/src/revision_comparison_tests.rs";
const PHYSICAL_SCRIPT: &str = "scripts/verify-recovery-hold.sh";

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum EvidenceClass {
    Contract,
    Integration,
    Stage,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct Binding {
    id: &'static str,
    owners: &'static [&'static str],
    implementation_issues: &'static [&'static str],
    evidence: &'static str,
    supporting_evidence: &'static [&'static str],
    evidence_class: EvidenceClass,
}

#[derive(Debug, Serialize)]
struct Stage3Crosswalk {
    schema_id: &'static str,
    claim_ceiling: &'static str,
    evaluated_stage: &'static str,
    execution_contract: ExecutionContract,
    declarations: Declarations,
    verifications: Verifications,
}

#[derive(Debug, Serialize)]
struct ExecutionContract {
    issue: &'static str,
    parent: &'static str,
    revision: &'static str,
    baseline_commit: &'static str,
    baseline_tree: &'static str,
}

#[derive(Debug, Serialize)]
struct Declarations {
    bindings: &'static [Binding],
}

#[derive(Debug, Serialize)]
struct Verifications {
    baseline: BaselineBinding,
    sources: Vec<SourceBinding>,
    handoff_evidence: HandoffEvidence,
}

#[derive(Debug, Serialize)]
struct BaselineBinding {
    commit: &'static str,
    tree: &'static str,
}

#[derive(Debug, Serialize)]
struct SourceBinding {
    path: &'static str,
    sha256: String,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct HandoffEvidence {
    hnd_003: Hnd003,
    hnd_004: DeferredHandoff,
    emitted: &'static [&'static str],
    forbidden_emissions: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct Hnd003 {
    id: &'static str,
    evaluated_stage: &'static str,
    mandatory_map: &'static str,
    disposition: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct DeferredHandoff {
    id: &'static str,
    status: &'static str,
}

#[path = "stage3_crosswalk_bindings.rs"]
mod bindings;

use bindings::BINDINGS;

/// Build the deterministic Stage 3 contract crosswalk bytes.
pub fn generate_stage3_crosswalk(repo_root: &Path) -> Result<Vec<u8>, CrosswalkError> {
    verify_implementation_baseline(repo_root)?;
    let handoff_evidence = HandoffEvidence {
        hnd_003: Hnd003 {
            id: "HND-003",
            evaluated_stage: EVALUATED_STAGE,
            mandatory_map: MANDATORY_MAP,
            disposition: CLAIM_CEILING,
        },
        hnd_004: DeferredHandoff {
            id: "HND-004",
            status: "not-emitted",
        },
        emitted: &[],
        forbidden_emissions: FORBIDDEN_EMISSIONS,
    };
    verify_stage3_record(BINDINGS, &handoff_evidence).map_err(CrosswalkError::Invalid)?;
    let mut bytes = serde_json::to_vec_pretty(&Stage3Crosswalk {
        schema_id: SCHEMA_ID,
        claim_ceiling: CLAIM_CEILING,
        evaluated_stage: EVALUATED_STAGE,
        execution_contract: ExecutionContract {
            issue: TICKET_ISSUE,
            parent: PARENT_ISSUE,
            revision: CONTRACT_REVISION,
            baseline_commit: BASELINE_COMMIT,
            baseline_tree: BASELINE_TREE,
        },
        declarations: Declarations { bindings: BINDINGS },
        verifications: Verifications {
            baseline: BaselineBinding {
                commit: BASELINE_COMMIT,
                tree: BASELINE_TREE,
            },
            sources: source_bindings(repo_root)?,
            handoff_evidence,
        },
    })
    .map_err(|source| CrosswalkError::Json {
        path: repo_root.join(GENERATED_STAGE3_CROSSWALK_PATH),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Write the deterministic Stage 3 contract crosswalk to its checked-in path.
pub fn write_stage3_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    let path = repo_root.join(GENERATED_STAGE3_CROSSWALK_PATH);
    let parent = path.parent().ok_or_else(|| {
        CrosswalkError::Invalid(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| CrosswalkError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    fs::write(&path, generate_stage3_crosswalk(repo_root)?)
        .map_err(|source| CrosswalkError::Io { path, source })
}

/// Check that the checked-in Stage 3 crosswalk equals a fresh deterministic generation.
pub fn check_stage3_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    let path = repo_root.join(GENERATED_STAGE3_CROSSWALK_PATH);
    let actual = fs::read(&path).map_err(|source| CrosswalkError::Io {
        path: path.clone(),
        source,
    })?;
    let expected = generate_stage3_crosswalk(repo_root)?;
    if actual != expected {
        return Err(CrosswalkError::Invalid(format!(
            "{} is stale; run `make generate-contracts`",
            path.display()
        )));
    }
    Ok(())
}

fn verify_stage3_record(bindings: &[Binding], handoff: &HandoffEvidence) -> Result<(), String> {
    if bindings.len() != REQUIRED_IDS.len() {
        return Err("Stage 3 binding count drifted".into());
    }
    for (index, required) in REQUIRED_IDS.iter().enumerate() {
        if bindings.get(index).map(|binding| binding.id) != Some(*required) {
            return Err(format!(
                "Stage 3 binding {required} is missing or reordered"
            ));
        }
        let binding = &bindings[index];
        if binding.owners.is_empty()
            || binding.implementation_issues.is_empty()
            || binding.evidence.is_empty()
            || binding
                .supporting_evidence
                .iter()
                .any(|path| path.is_empty())
        {
            return Err(format!("Stage 3 binding {required} is not attributable"));
        }
    }
    let classes = bindings
        .iter()
        .map(|binding| binding.evidence_class)
        .collect::<Vec<_>>();
    for required in [
        EvidenceClass::Contract,
        EvidenceClass::Integration,
        EvidenceClass::Stage,
    ] {
        if !classes.contains(&required) {
            return Err("Stage 3 evidence classes are not distinct".into());
        }
    }
    if handoff.hnd_003.evaluated_stage != EVALUATED_STAGE
        || handoff.hnd_003.mandatory_map != MANDATORY_MAP
        || handoff.hnd_004.status != "not-emitted"
        || !handoff.emitted.is_empty()
        || handoff.forbidden_emissions != FORBIDDEN_EMISSIONS
    {
        return Err("Stage 3 handoff must record HND-003 only".into());
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

fn baseline_file_sha256(repo_root: &Path, relative_path: &str) -> Result<String, CrosswalkError> {
    let object = format!("{BASELINE_COMMIT}:{relative_path}");
    let bytes = git_output(repo_root, &["show", &object])?;
    if bytes.contains(&b'\r') || !bytes.ends_with(b"\n") {
        return Err(CrosswalkError::Invalid(format!(
            "{object} must use UTF-8/LF with a final LF"
        )));
    }
    std::str::from_utf8(&bytes)
        .map_err(|error| CrosswalkError::Invalid(format!("{object} is not UTF-8: {error}")))?;
    Ok(sha256_prefixed(bytes))
}

fn verify_implementation_baseline(repo_root: &Path) -> Result<(), CrosswalkError> {
    let revision = format!("{BASELINE_COMMIT}^{{tree}}");
    let output = git_output(repo_root, &["rev-parse", &revision])?;
    let actual = std::str::from_utf8(&output)
        .map_err(|error| CrosswalkError::Invalid(format!("git tree output is not UTF-8: {error}")))?
        .trim();
    if actual != BASELINE_TREE {
        return Err(CrosswalkError::Invalid(format!(
            "implementation baseline tree drifted: expected {BASELINE_TREE}, found {actual}"
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

#[cfg(test)]
#[path = "stage3_crosswalk_tests.rs"]
mod tests;
