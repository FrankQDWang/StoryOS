use std::fs;
use std::path::Path;

use serde::Serialize;

const EXPECTED_CRATES: &[&str] = &[
    "crates/storyos-adapter-postgres",
    "crates/storyos-application",
    "crates/storyos-contracts",
    "crates/storyos-core",
    "crates/storyos-server",
];
const EXCLUDED_TREES: &[&str] = &["prototypes/**", ".reference/**"];
const JOURNEY_TEST: &str = "apps/web/test/s1-jrn-001-browser.integration.test.mjs";
const JOURNEY_MARKER: &str = "Vite production page";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct ProvenanceEvidence {
    pub(super) s1_req_006: RequirementClaim,
    pub(super) s1_evd_006: RequirementClaim,
    pub(super) workspace: WorkspaceProvenance,
    pub(super) journey_evidence: JourneyEvidence,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct RequirementClaim {
    pub(super) id: &'static str,
    pub(super) owners: &'static [&'static str],
    pub(super) claim: &'static str,
    pub(super) forbidden: &'static [&'static str],
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct WorkspaceProvenance {
    pub(super) crates: &'static [&'static str],
    pub(super) excluded_trees: &'static [&'static str],
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct JourneyEvidence {
    pub(super) requirement: &'static str,
    pub(super) test: &'static str,
    pub(super) page: &'static str,
    pub(super) server: &'static str,
    pub(super) store: &'static str,
}

pub(super) fn provenance_evidence(repo_root: &Path) -> Result<ProvenanceEvidence, String> {
    let evidence = ProvenanceEvidence {
        s1_req_006: RequirementClaim {
            id: "S1-REQ-006",
            owners: &[
                "OWN-GOV",
                "OWN-WEB",
                "OWN-PROTO",
                "OWN-CORE",
                "OWN-PG",
                "OWN-ADM",
                "OWN-RET",
            ],
            claim: "production-shaped",
            forbidden: &[
                "prototype",
                "in-memory-authority",
                "local-file-authority",
                "test-adapter",
                ".reference/**",
            ],
        },
        s1_evd_006: RequirementClaim {
            id: "S1-EVD-006",
            owners: &["OWN-GOV"],
            claim: "no-contamination",
            forbidden: &[
                "prototype",
                "in-memory",
                "test-adapter",
                "local-authority",
                ".reference/**",
            ],
        },
        workspace: WorkspaceProvenance {
            crates: EXPECTED_CRATES,
            excluded_trees: EXCLUDED_TREES,
        },
        journey_evidence: JourneyEvidence {
            requirement: "S1-JRN-001",
            test: JOURNEY_TEST,
            page: "vite-production",
            server: "storyos-server",
            store: "postgresql",
        },
    };
    verify_provenance_evidence(repo_root, &evidence)?;
    Ok(evidence)
}

pub(super) fn verify_provenance_evidence(
    repo_root: &Path,
    evidence: &ProvenanceEvidence,
) -> Result<(), String> {
    let members = cargo_workspace_members(repo_root)?;
    if let Some(path) = members.iter().find(|path| {
        path.contains(".reference/")
            || path.contains("prototypes/")
            || path.contains(".reference\\")
    }) {
        return Err(format!("Stage 1 workspace contains forbidden tree {path}"));
    }
    if members != EXPECTED_CRATES {
        return Err(format!(
            "Stage 1 workspace members drifted: expected {EXPECTED_CRATES:?}, found {members:?}"
        ));
    }
    if evidence.workspace.crates != EXPECTED_CRATES
        || evidence.workspace.excluded_trees != EXCLUDED_TREES
    {
        return Err("provenance workspace claim drifted".into());
    }
    let journey = repo_root.join(JOURNEY_TEST);
    let journey_source = fs::read_to_string(&journey)
        .map_err(|error| format!("{} is missing: {error}", journey.display()))?;
    if !journey_source.contains(JOURNEY_MARKER) {
        return Err(format!(
            "{JOURNEY_TEST} is not the Vite production-page journey"
        ));
    }
    if evidence.journey_evidence.test != JOURNEY_TEST
        || evidence.journey_evidence.page != "vite-production"
        || evidence.journey_evidence.server != "storyos-server"
        || evidence.journey_evidence.store != "postgresql"
    {
        return Err("S1-JRN-001 provenance claim drifted".into());
    }
    if !evidence.s1_req_006.forbidden.contains(&".reference/**")
        || !evidence.s1_evd_006.forbidden.contains(&".reference/**")
    {
        return Err("provenance must refuse .reference/**".into());
    }
    Ok(())
}

fn cargo_workspace_members(repo_root: &Path) -> Result<Vec<String>, String> {
    let path = repo_root.join("Cargo.toml");
    let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let start = text
        .find("members = [")
        .ok_or_else(|| format!("{} has no workspace members list", path.display()))?;
    let rest = &text[start + "members = [".len()..];
    let end = rest
        .find(']')
        .ok_or_else(|| format!("{} workspace members list is unclosed", path.display()))?;
    Ok(rest[..end]
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_end_matches(',').trim_matches('"').to_owned())
        .collect())
}

#[cfg(test)]
#[path = "stage1_provenance_tests.rs"]
mod tests;
