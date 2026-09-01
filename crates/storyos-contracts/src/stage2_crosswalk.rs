use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::digest::sha256_prefixed;
use crate::stage1_crosswalk::CrosswalkError;

/// Checked-in deterministic output owned by this module.
pub const GENERATED_STAGE2_CROSSWALK_PATH: &str =
    "generated/evidence/stage2-contract-crosswalk.json";

const SCHEMA_ID: &str = "storyos.evidence.stage2-contract-crosswalk.v1";
const CLAIM_CEILING: &str = "implementation-evidence-completeness; no EV-SR; no PASS-STAGE";
const EVALUATED_STAGE: &str = "Stage 2";
const MANDATORY_MAP: &str = "SMAP-STAGE-2";
const CONTRACT_REVISION: &str = "product-ticket-mvp-boundary-2026-08-29-v2";
const PARENT_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/241";
const TICKET_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/281";
const BASELINE_COMMIT: &str = "6fed2a853f4479de1d6b8c17efe2135c315fc7ca";
const BASELINE_TREE: &str = "608da886c377af1bf2b291f60240af85f2c0149d";
const FORBIDDEN_EMISSIONS: &[&str] = &["EV-SR", "PASS-STAGE", "PASS-CLOUD"];
const REQUIRED_IDS: &[&str] = &[
    "REL-007",
    "S2-REQ-001",
    "S2-REQ-002",
    "S2-REQ-003",
    "S2-REQ-004",
    "S2-REQ-005",
    "S2-REQ-006",
    "S2-REQ-007",
    "S2-REQ-008",
    "S2-REQ-009",
    "S2-EVD-001",
    "S2-EVD-002",
    "S2-EVD-003",
    "S2-EVD-004",
    "S2-EVD-005",
    "S2-EVD-006",
    "S2-EVD-007",
    "S2-EVD-008",
    "S2-EVD-009",
    "S2-JRN-001",
    "S2-JRN-001/1",
    "S2-JRN-001/2",
    "S2-JRN-001/3",
    "S2-JRN-001/4",
    "S2-JRN-001/5",
    "S2-JRN-001/6",
    "S2-JRN-001/7",
    "S2-JRN-001/8",
    "S2-JRN-001/9",
    "S2-JRN-001/10",
    "S2-JRN-001/11",
    "S2-JRN-001/12",
];
const SOURCE_PATHS: &[&str] = &[
    "docs/foundation/ai-independent-editor-first-release-baseline-and-handoff-criteria.md",
    "docs/foundation/deterministic-verification-and-failure-recovery-gates.md",
    "docs/foundation/product-delivery-proof-selection.md",
];

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum EvidenceClass {
    Contract,
    Integration,
    PhysicalRecovery,
    Stage,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct Binding {
    id: &'static str,
    owners: &'static [&'static str],
    implementation_issues: &'static [&'static str],
    evidence: &'static str,
    evidence_class: EvidenceClass,
}

#[derive(Debug, Serialize)]
struct Stage2Crosswalk {
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

const BINDINGS: &[Binding] = &[
    Binding {
        id: "REL-007",
        owners: &["OWN-REL", "OWN-DVG", "OWN-GOV"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/62",
            TICKET_ISSUE,
        ],
        evidence: "docs/foundation/product-delivery-proof-selection.md",
        evidence_class: EvidenceClass::Contract,
    },
    Binding {
        id: "S2-REQ-001",
        owners: &["OWN-REL", "OWN-WEB", "OWN-CORE", "OWN-PG"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/280"],
        evidence: "apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-REQ-002",
        owners: &["OWN-PROTO", "OWN-CORE", "OWN-PG", "OWN-RET"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/248",
            "https://github.com/FrankQDWang/StoryOS/issues/250",
            "https://github.com/FrankQDWang/StoryOS/issues/251",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-02-create-empty-project.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-REQ-003",
        owners: &["OWN-WEB", "OWN-PROTO", "OWN-CORE", "OWN-RET"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/255",
            "https://github.com/FrankQDWang/StoryOS/issues/258",
            "https://github.com/FrankQDWang/StoryOS/issues/259",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-12-current-chapter.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-REQ-004",
        owners: &["OWN-WEB", "OWN-ADM", "OWN-CORE"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/261",
            "https://github.com/FrankQDWang/StoryOS/issues/262",
            "https://github.com/FrankQDWang/StoryOS/issues/264",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-input.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-REQ-005",
        owners: &["OWN-WEB", "OWN-PG", "OWN-RET", "OWN-ADM", "OWN-CORE"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/266",
            "https://github.com/FrankQDWang/StoryOS/issues/276",
            "https://github.com/FrankQDWang/StoryOS/issues/278",
        ],
        evidence: "scripts/verify-recovery-hold.sh",
        evidence_class: EvidenceClass::PhysicalRecovery,
    },
    Binding {
        id: "S2-REQ-006",
        owners: &["OWN-PROTO", "OWN-CTX", "OWN-CORE", "OWN-PLAIN"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/269",
            "https://github.com/FrankQDWang/StoryOS/issues/270",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-replace.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-REQ-007",
        owners: &["OWN-PROTO", "OWN-PG", "OWN-RET", "OWN-TRUST"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/271",
            "https://github.com/FrankQDWang/StoryOS/issues/272",
            "https://github.com/FrankQDWang/StoryOS/issues/275",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-readable-export.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-REQ-008",
        owners: &["OWN-MEASURE", "OWN-REL", "OWN-WEB", "OWN-PG", "OWN-RET"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/279"],
        evidence: "apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-REQ-009",
        owners: &["OWN-WEB", "OWN-CORE", "OWN-GOV"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/260",
            "https://github.com/FrankQDWang/StoryOS/issues/372",
            "https://github.com/FrankQDWang/StoryOS/issues/373",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-editor.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-EVD-001",
        owners: &["OWN-REL", "OWN-DVG"],
        implementation_issues: &[TICKET_ISSUE],
        evidence: GENERATED_STAGE2_CROSSWALK_PATH,
        evidence_class: EvidenceClass::Contract,
    },
    Binding {
        id: "S2-EVD-002",
        owners: &["OWN-WEB", "OWN-PROTO", "OWN-CORE", "OWN-RET"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/248",
            "https://github.com/FrankQDWang/StoryOS/issues/253",
            "https://github.com/FrankQDWang/StoryOS/issues/254",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-09-navigate-reopen.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-EVD-003",
        owners: &["OWN-WEB", "OWN-MEASURE"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/261",
            "https://github.com/FrankQDWang/StoryOS/issues/264",
            "https://github.com/FrankQDWang/StoryOS/issues/279",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-input.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-EVD-004",
        owners: &["OWN-ADM", "OWN-CORE", "OWN-PG", "OWN-RET"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/265"],
        evidence: "apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-EVD-005",
        owners: &["OWN-PG", "OWN-RET", "OWN-TRUST"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/276",
            "https://github.com/FrankQDWang/StoryOS/issues/277",
            "https://github.com/FrankQDWang/StoryOS/issues/278",
        ],
        evidence: "scripts/verify-recovery-hold.sh",
        evidence_class: EvidenceClass::PhysicalRecovery,
    },
    Binding {
        id: "S2-EVD-006",
        owners: &["OWN-CTX", "OWN-CORE", "OWN-PROTO"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/269",
            "https://github.com/FrankQDWang/StoryOS/issues/270",
            "https://github.com/FrankQDWang/StoryOS/issues/271",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-search.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-EVD-007",
        owners: &["OWN-PROTO", "OWN-PG", "OWN-RET"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/272",
            "https://github.com/FrankQDWang/StoryOS/issues/274",
            "https://github.com/FrankQDWang/StoryOS/issues/275",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-readable-export.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-EVD-008",
        owners: &["OWN-MEASURE", "OWN-PG", "OWN-WEB"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/279"],
        evidence: "apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-EVD-009",
        owners: &["OWN-WEB", "OWN-CORE", "OWN-PG", "OWN-GOV"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/260",
            "https://github.com/FrankQDWang/StoryOS/issues/373",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-workspace.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001",
        owners: &["OWN-REL", "OWN-WEB", "OWN-CORE", "OWN-PG"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/280"],
        evidence: "apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts",
        evidence_class: EvidenceClass::Stage,
    },
    Binding {
        id: "S2-JRN-001/1",
        owners: &["OWN-REL", "OWN-WEB", "OWN-PG"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/247",
            "https://github.com/FrankQDWang/StoryOS/issues/248",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-01-bootstrap-challenge.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/2",
        owners: &["OWN-WEB", "OWN-PROTO", "OWN-CORE"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/255",
            "https://github.com/FrankQDWang/StoryOS/issues/280",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/3",
        owners: &["OWN-WEB", "OWN-ADM", "OWN-CORE"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/261"],
        evidence: "apps/web/test/browser-exact-dist/s2-input.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/4",
        owners: &["OWN-WEB", "OWN-ADM", "OWN-CORE"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/265"],
        evidence: "apps/web/test/browser-exact-dist/s2-save-truth.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/5",
        owners: &["OWN-WEB", "OWN-ADM", "OWN-CORE", "OWN-RET"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/266"],
        evidence: "apps/web/test/browser-exact-dist/s2-interruption.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/6",
        owners: &["OWN-PG", "OWN-RET", "OWN-TRUST"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/278"],
        evidence: "scripts/verify-recovery-hold.sh",
        evidence_class: EvidenceClass::PhysicalRecovery,
    },
    Binding {
        id: "S2-JRN-001/7",
        owners: &["OWN-WEB", "OWN-CORE", "OWN-ADM"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/267"],
        evidence: "apps/web/test/browser-exact-dist/production-host.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/8",
        owners: &["OWN-PROTO", "OWN-CTX", "OWN-CORE"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/269",
            "https://github.com/FrankQDWang/StoryOS/issues/270",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-search.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/9",
        owners: &["OWN-PROTO", "OWN-WEB"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/271"],
        evidence: "apps/web/test/browser-exact-dist/s2-statistics.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/10",
        owners: &["OWN-RET", "OWN-PROTO", "OWN-WEB"],
        implementation_issues: &["https://github.com/FrankQDWang/StoryOS/issues/268"],
        evidence: "apps/web/test/browser-source/activity-resync.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/11",
        owners: &["OWN-PROTO", "OWN-PG", "OWN-RET"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/272",
            "https://github.com/FrankQDWang/StoryOS/issues/275",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-readable-export.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
    Binding {
        id: "S2-JRN-001/12",
        owners: &["OWN-MEASURE", "OWN-WEB", "OWN-REL"],
        implementation_issues: &[
            "https://github.com/FrankQDWang/StoryOS/issues/279",
            "https://github.com/FrankQDWang/StoryOS/issues/280",
        ],
        evidence: "apps/web/test/browser-exact-dist/s2-long-session.integration.test.ts",
        evidence_class: EvidenceClass::Integration,
    },
];

/// Build the deterministic Stage 2 contract crosswalk bytes.
pub fn generate_stage2_crosswalk(repo_root: &Path) -> Result<Vec<u8>, CrosswalkError> {
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
    verify_stage2_record(BINDINGS, &handoff_evidence).map_err(CrosswalkError::Invalid)?;
    let mut bytes = serde_json::to_vec_pretty(&Stage2Crosswalk {
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
        path: repo_root.join(GENERATED_STAGE2_CROSSWALK_PATH),
        source,
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Write the deterministic Stage 2 contract crosswalk to its checked-in path.
pub fn write_stage2_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    let path = repo_root.join(GENERATED_STAGE2_CROSSWALK_PATH);
    let parent = path.parent().ok_or_else(|| {
        CrosswalkError::Invalid(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent).map_err(|source| CrosswalkError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    fs::write(&path, generate_stage2_crosswalk(repo_root)?)
        .map_err(|source| CrosswalkError::Io { path, source })
}

/// Check that the checked-in Stage 2 crosswalk equals a fresh deterministic generation.
pub fn check_stage2_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    let path = repo_root.join(GENERATED_STAGE2_CROSSWALK_PATH);
    let actual = fs::read(&path).map_err(|source| CrosswalkError::Io {
        path: path.clone(),
        source,
    })?;
    let expected = generate_stage2_crosswalk(repo_root)?;
    if actual != expected {
        return Err(CrosswalkError::Invalid(format!(
            "{} is stale; run `make generate-contracts`",
            path.display()
        )));
    }
    Ok(())
}

fn verify_stage2_record(bindings: &[Binding], handoff: &HandoffEvidence) -> Result<(), String> {
    if bindings.len() != REQUIRED_IDS.len() {
        return Err("Stage 2 binding count drifted".into());
    }
    for (index, required) in REQUIRED_IDS.iter().enumerate() {
        if bindings.get(index).map(|binding| binding.id) != Some(*required) {
            return Err(format!(
                "Stage 2 binding {required} is missing or reordered"
            ));
        }
        let binding = &bindings[index];
        if binding.owners.is_empty()
            || binding.implementation_issues.is_empty()
            || binding.evidence.is_empty()
        {
            return Err(format!("Stage 2 binding {required} is not attributable"));
        }
    }
    let classes = bindings
        .iter()
        .map(|binding| binding.evidence_class)
        .collect::<Vec<_>>();
    for required in [
        EvidenceClass::Contract,
        EvidenceClass::Integration,
        EvidenceClass::PhysicalRecovery,
        EvidenceClass::Stage,
    ] {
        if !classes.contains(&required) {
            return Err("Stage 2 evidence classes are not distinct".into());
        }
    }
    if handoff.hnd_003.evaluated_stage != EVALUATED_STAGE
        || handoff.hnd_003.mandatory_map != MANDATORY_MAP
        || handoff.hnd_004.status != "not-emitted"
        || !handoff.emitted.is_empty()
        || handoff.forbidden_emissions != FORBIDDEN_EMISSIONS
    {
        return Err("Stage 2 handoff must record HND-003 only".into());
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
#[path = "stage2_crosswalk_tests.rs"]
mod tests;
