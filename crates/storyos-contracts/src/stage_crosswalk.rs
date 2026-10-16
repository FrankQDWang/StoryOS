//! Shared generator of the stage contract crosswalks that record HND-003 only.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::digest::sha256_prefixed;
use crate::stage1_crosswalk::CrosswalkError;

pub(crate) const CLAIM_CEILING: &str =
    "implementation-evidence-completeness; no EV-SR; no PASS-STAGE";
pub(crate) const FORBIDDEN_EMISSIONS: &[&str] = &["EV-SR", "PASS-STAGE", "PASS-CLOUD"];
const SOURCE_PATHS: &[&str] = &[
    "docs/foundation/ai-independent-editor-first-release-baseline-and-handoff-criteria.md",
    "docs/foundation/deterministic-verification-and-failure-recovery-gates.md",
    "docs/foundation/product-delivery-proof-selection.md",
];

/// The fixed identity of one stage crosswalk.
pub(crate) struct StageCrosswalk {
    pub(crate) generated_path: &'static str,
    pub(crate) schema_id: &'static str,
    pub(crate) evaluated_stage: &'static str,
    pub(crate) mandatory_map: &'static str,
    pub(crate) contract_revision: &'static str,
    pub(crate) parent_issue: &'static str,
    pub(crate) ticket_issue: &'static str,
    pub(crate) baseline_commit: &'static str,
    pub(crate) baseline_tree: &'static str,
    pub(crate) required_ids: &'static [&'static str],
    pub(crate) bindings: &'static [Binding],
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceClass {
    Contract,
    Integration,
    Stage,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Binding {
    pub(crate) id: &'static str,
    pub(crate) owners: &'static [&'static str],
    pub(crate) implementation_issues: &'static [&'static str],
    pub(crate) evidence: &'static str,
    pub(crate) supporting_evidence: &'static [&'static str],
    pub(crate) evidence_class: EvidenceClass,
}

#[derive(Debug, Serialize)]
struct CrosswalkRecord {
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
pub(crate) struct HandoffEvidence {
    pub(crate) hnd_003: Hnd003,
    pub(crate) hnd_004: DeferredHandoff,
    pub(crate) emitted: &'static [&'static str],
    pub(crate) forbidden_emissions: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Hnd003 {
    pub(crate) id: &'static str,
    pub(crate) evaluated_stage: &'static str,
    pub(crate) mandatory_map: &'static str,
    pub(crate) disposition: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct DeferredHandoff {
    pub(crate) id: &'static str,
    pub(crate) status: &'static str,
}

impl StageCrosswalk {
    /// Build the deterministic crosswalk bytes.
    pub(crate) fn generate(&self, repo_root: &Path) -> Result<Vec<u8>, CrosswalkError> {
        self.verify_implementation_baseline(repo_root)?;
        let handoff_evidence = HandoffEvidence {
            hnd_003: Hnd003 {
                id: "HND-003",
                evaluated_stage: self.evaluated_stage,
                mandatory_map: self.mandatory_map,
                disposition: CLAIM_CEILING,
            },
            hnd_004: DeferredHandoff {
                id: "HND-004",
                status: "not-emitted",
            },
            emitted: &[],
            forbidden_emissions: FORBIDDEN_EMISSIONS,
        };
        self.verify_record(self.bindings, &handoff_evidence)
            .map_err(CrosswalkError::Invalid)?;
        let mut bytes = serde_json::to_vec_pretty(&CrosswalkRecord {
            schema_id: self.schema_id,
            claim_ceiling: CLAIM_CEILING,
            evaluated_stage: self.evaluated_stage,
            execution_contract: ExecutionContract {
                issue: self.ticket_issue,
                parent: self.parent_issue,
                revision: self.contract_revision,
                baseline_commit: self.baseline_commit,
                baseline_tree: self.baseline_tree,
            },
            declarations: Declarations {
                bindings: self.bindings,
            },
            verifications: Verifications {
                baseline: BaselineBinding {
                    commit: self.baseline_commit,
                    tree: self.baseline_tree,
                },
                sources: self.source_bindings(repo_root)?,
                handoff_evidence,
            },
        })
        .map_err(|source| CrosswalkError::Json {
            path: repo_root.join(self.generated_path),
            source,
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Write the deterministic crosswalk to its tracked path.
    pub(crate) fn write(&self, repo_root: &Path) -> Result<(), CrosswalkError> {
        let path = repo_root.join(self.generated_path);
        let parent = path.parent().ok_or_else(|| {
            CrosswalkError::Invalid(format!("{} has no parent directory", path.display()))
        })?;
        fs::create_dir_all(parent).map_err(|source| CrosswalkError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        fs::write(&path, self.generate(repo_root)?)
            .map_err(|source| CrosswalkError::Io { path, source })
    }

    /// Examine whether the tracked crosswalk is equal to a fresh deterministic generation.
    pub(crate) fn check(&self, repo_root: &Path) -> Result<(), CrosswalkError> {
        let path = repo_root.join(self.generated_path);
        let actual = fs::read(&path).map_err(|source| CrosswalkError::Io {
            path: path.clone(),
            source,
        })?;
        if actual != self.generate(repo_root)? {
            return Err(CrosswalkError::Invalid(format!(
                "{} is stale; run `make generate-contracts`",
                path.display()
            )));
        }
        Ok(())
    }

    pub(crate) fn verify_record(
        &self,
        bindings: &[Binding],
        handoff: &HandoffEvidence,
    ) -> Result<(), String> {
        let stage = self.evaluated_stage;
        if bindings.len() != self.required_ids.len() {
            return Err(format!("{stage} binding count drifted"));
        }
        for (binding, required) in bindings.iter().zip(self.required_ids) {
            if binding.id != *required {
                return Err(format!(
                    "{stage} binding {required} is missing or reordered"
                ));
            }
            if binding.owners.is_empty()
                || binding.implementation_issues.is_empty()
                || binding.evidence.is_empty()
                || binding
                    .supporting_evidence
                    .iter()
                    .any(|path| path.is_empty())
            {
                return Err(format!("{stage} binding {required} is not attributable"));
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
                return Err(format!("{stage} evidence classes are not distinct"));
            }
        }
        if handoff.hnd_003.evaluated_stage != stage
            || handoff.hnd_003.mandatory_map != self.mandatory_map
            || handoff.hnd_004.status != "not-emitted"
            || !handoff.emitted.is_empty()
            || handoff.forbidden_emissions != FORBIDDEN_EMISSIONS
        {
            return Err(format!("{stage} handoff must record HND-003 only"));
        }
        Ok(())
    }

    fn source_bindings(&self, repo_root: &Path) -> Result<Vec<SourceBinding>, CrosswalkError> {
        SOURCE_PATHS
            .iter()
            .map(|path| {
                Ok(SourceBinding {
                    path,
                    sha256: self.baseline_file_sha256(repo_root, path)?,
                })
            })
            .collect()
    }

    fn baseline_file_sha256(
        &self,
        repo_root: &Path,
        relative_path: &str,
    ) -> Result<String, CrosswalkError> {
        let object = format!("{}:{relative_path}", self.baseline_commit);
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

    fn verify_implementation_baseline(&self, repo_root: &Path) -> Result<(), CrosswalkError> {
        let revision = format!("{}^{{tree}}", self.baseline_commit);
        let output = git_output(repo_root, &["rev-parse", &revision])?;
        let actual = std::str::from_utf8(&output)
            .map_err(|error| {
                CrosswalkError::Invalid(format!("git tree output is not UTF-8: {error}"))
            })?
            .trim();
        if actual != self.baseline_tree {
            return Err(CrosswalkError::Invalid(format!(
                "implementation baseline tree drifted: expected {}, found {actual}",
                self.baseline_tree
            )));
        }
        Ok(())
    }
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
