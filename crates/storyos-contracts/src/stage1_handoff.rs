use serde::Serialize;

use crate::stage1_bundle::MandatoryEvidence;
use crate::stage1_provenance::ProvenanceEvidence;

const PARENT_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/100";
const PARENT_TITLE: &str = "Implement the Stage 1 Production-Shaped Manual-Editor Risk Slice";
const LATER_STAGE_MAPS: &[&str] = &["SMAP-STAGE-2", "SMAP-STAGE-3", "SMAP-STAGE-4"];
const LATER_JOURNEYS: &[&str] = &["S2-JRN-001", "S3-JRN-001", "S4-JRN-001"];
const EXTERNAL_MODEL_PROVIDER: &[&str] = &[
    "FX-FAKE-MODEL",
    "FX-REAL-MODEL-ADVISORY",
    "B-FAKE",
    "B-REAL-ADVISORY",
];
const MODEL_PROVIDER_FIXTURES: &[&str] = &["FX-FAKE-MODEL", "FX-REAL-MODEL-ADVISORY"];
const MODEL_PROVIDER_BUNDLES: &[&str] = &["B-FAKE", "B-REAL-ADVISORY"];
const FORBIDDEN_EMISSIONS: &[&str] = &["EV-SR", "PASS-STAGE", "PASS-CLOUD"];
const PARENT_CLOSEOUT: &str = "performed";
const NEXT_ISSUE: &str = "none; no Stage 2 issue created";

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(super) struct HandoffEvidence {
    pub(super) hnd_003: Hnd003,
    pub(super) hnd_004: DeferredHandoff,
    pub(super) hnd_005: DeferredHandoff,
    pub(super) parent_issue: ParentIssue,
    pub(super) next_issue: &'static str,
    pub(super) residual_risks: ResidualRisks,
    pub(super) out_of_scope: OutOfScope,
    pub(super) emitted: &'static [&'static str],
    pub(super) forbidden_emissions: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(super) struct Hnd003 {
    pub(super) id: &'static str,
    pub(super) evaluated_stage: &'static str,
    pub(super) mandatory_map: &'static str,
    pub(super) disposition: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(super) struct DeferredHandoff {
    pub(super) id: &'static str,
    pub(super) status: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ParentIssue {
    pub(super) issue: &'static str,
    pub(super) title: &'static str,
    pub(super) state: &'static str,
    pub(super) closeout: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(super) struct ResidualRisks {
    pub(super) later_stages: &'static str,
    pub(super) external_model_provider: &'static str,
    pub(super) prototype_only: &'static str,
    pub(super) reference_tree: &'static str,
    pub(super) stage_release: &'static str,
    pub(super) parent_closeout: &'static str,
    pub(super) later_cloud: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(super) struct OutOfScope {
    pub(super) later_stage_maps: &'static [&'static str],
    pub(super) later_journeys: &'static [&'static str],
    pub(super) external_model_provider: &'static [&'static str],
    pub(super) prototype_only: &'static [&'static str],
    pub(super) reference_tree: &'static [&'static str],
}

pub(super) fn handoff_evidence(
    mandatory: &MandatoryEvidence,
    provenance: &ProvenanceEvidence,
) -> Result<HandoffEvidence, String> {
    let evidence = HandoffEvidence {
        hnd_003: Hnd003 {
            id: "HND-003",
            evaluated_stage: "Stage 1",
            mandatory_map: "SMAP-STAGE-1",
            disposition: "implementation-evidence-completeness; no EV-SR; no PASS-STAGE",
        },
        hnd_004: DeferredHandoff {
            id: "HND-004",
            status: "not-emitted",
        },
        hnd_005: DeferredHandoff {
            id: "HND-005",
            status: "not-emitted",
        },
        parent_issue: ParentIssue {
            issue: PARENT_ISSUE,
            title: PARENT_TITLE,
            state: "closed",
            closeout: PARENT_CLOSEOUT,
        },
        next_issue: NEXT_ISSUE,
        residual_risks: ResidualRisks {
            later_stages: "unrun",
            external_model_provider: "unintegrated",
            prototype_only: "unpromoted",
            reference_tree: "not-a-production-dependency",
            stage_release: "not-claimed",
            parent_closeout: PARENT_CLOSEOUT,
            later_cloud: "not-claimed",
        },
        out_of_scope: OutOfScope {
            later_stage_maps: LATER_STAGE_MAPS,
            later_journeys: LATER_JOURNEYS,
            external_model_provider: EXTERNAL_MODEL_PROVIDER,
            prototype_only: &["prototypes/**"],
            reference_tree: &[".reference/**"],
        },
        emitted: &[],
        forbidden_emissions: FORBIDDEN_EMISSIONS,
    };
    verify_handoff_evidence(&evidence, mandatory, provenance)?;
    Ok(evidence)
}

pub(super) fn verify_handoff_evidence(
    evidence: &HandoffEvidence,
    mandatory: &MandatoryEvidence,
    provenance: &ProvenanceEvidence,
) -> Result<(), String> {
    if evidence.hnd_003.mandatory_map != mandatory.smap_stage_1.id {
        return Err("HND-003 map drifted from SMAP-STAGE-1".into());
    }
    if LATER_STAGE_MAPS.contains(&mandatory.smap_stage_1.id) {
        return Err("Stage 1 handoff must not evaluate a later-stage map".into());
    }
    if evidence.out_of_scope.later_stage_maps != LATER_STAGE_MAPS {
        return Err("handoff later-stage maps drifted".into());
    }
    if evidence.out_of_scope.later_journeys != LATER_JOURNEYS {
        return Err("handoff later journeys drifted".into());
    }
    if evidence.parent_issue.issue != mandatory.fx_handoff.parent_issue.issue
        || evidence.parent_issue.state != "closed"
        || mandatory.fx_handoff.parent_issue.state != "closed"
        || evidence.parent_issue.closeout != PARENT_CLOSEOUT
        || evidence.residual_risks.parent_closeout != PARENT_CLOSEOUT
        || evidence.next_issue != NEXT_ISSUE
    {
        return Err("handoff must record performed parent #100 closeout".into());
    }
    if !evidence.emitted.is_empty() {
        return Err(format!(
            "handoff must not emit {}",
            evidence.emitted.join(", ")
        ));
    }
    if evidence.forbidden_emissions != FORBIDDEN_EMISSIONS {
        return Err("handoff forbidden emissions drifted".into());
    }
    if !mandatory.dvg_13.disposition.contains("no EV-SR")
        || !mandatory.dvg_13.disposition.contains("no PASS-STAGE")
        || !mandatory
            .fx_handoff
            .stage_disposition
            .contains("no PASS-STAGE")
        || !mandatory.fx_handoff.stage_disposition.contains("no EV-SR")
    {
        return Err("handoff must not claim stage release".into());
    }
    if evidence.hnd_004.status != "not-emitted" || evidence.hnd_005.status != "not-emitted" {
        return Err("HND-004 and HND-005 must remain unemitted".into());
    }
    if MODEL_PROVIDER_FIXTURES
        .iter()
        .any(|item| mandatory.smap_stage_1.fixtures.contains(item))
    {
        return Err("SMAP-STAGE-1 must not include external model-provider fixtures".into());
    }
    if evidence.out_of_scope.external_model_provider != EXTERNAL_MODEL_PROVIDER {
        return Err("handoff must keep external model-provider integration out of scope".into());
    }
    if !MODEL_PROVIDER_BUNDLES
        .iter()
        .all(|item| mandatory.b_s1_mandatory_set.excluded.contains(item))
    {
        return Err("B-S1-MANDATORY-SET must exclude B-FAKE and B-REAL-ADVISORY".into());
    }
    if !provenance
        .workspace
        .excluded_trees
        .contains(&"prototypes/**")
        || !evidence
            .out_of_scope
            .prototype_only
            .contains(&"prototypes/**")
    {
        return Err("handoff must keep prototype-only paths out of scope".into());
    }
    if !provenance
        .workspace
        .excluded_trees
        .contains(&".reference/**")
        || !evidence
            .out_of_scope
            .reference_tree
            .contains(&".reference/**")
    {
        return Err("handoff must keep .reference/** out of scope".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "stage1_handoff_tests.rs"]
mod tests;
