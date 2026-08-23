use serde::Serialize;

use crate::release1::PUBLIC_PROTOCOL_RELEASE;
use crate::stage1_delivery::{
    DELIVERY_BASELINE_COMMIT, DELIVERY_BASELINE_TREE, DELIVERY_CONTRACT_REVISION,
};
use crate::stage1_selection::{BASELINE_COMMIT, BASELINE_TREE, CONTRACT_REVISION};

const ISSUE_112_REVISION: &str = "issue-112-provenance-workspace-isolation-2026-08-23-v4";
const PARENT_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/100";
const PARENT_TITLE: &str = "Implement the Stage 1 Production-Shaped Manual-Editor Risk Slice";
const MANDATORY_SET_MEMBERS: &[&str] = &[
    "B-CONTRACT",
    "B-SCOPE",
    "B-EDITOR",
    "B-CORE",
    "B-RECOVERY",
    "B-REPLAY",
    "B-HANDOFF",
];
const MANDATORY_SET_EXCLUDED: &[&str] = &[
    "B-RESTORE",
    "B-CONTEXT",
    "B-FAKE",
    "B-REAL-ADVISORY",
    "B-ABSENT",
    "B-EVAL",
    "B-MEASURE",
];
const SMAP_GATES: &[&str] = &[
    "DVG-01", "DVG-02", "DVG-03", "DVG-07", "DVG-08", "DVG-09", "DVG-11", "DVG-13",
];
const SMAP_EVIDENCE_CLASSES: &[&str] = &[
    "EC-01", "EC-02", "EC-03", "EC-04", "EC-05", "EC-07", "EV-CP", "EV-IT", "EV-INT", "EV-SE",
];
const SMAP_FIXTURES: &[&str] = &[
    "FX-CONTRACT-R1",
    "FX-CORE-PROPOSAL",
    "FX-EDITOR-IME",
    "FX-HANDOFF",
    "FX-JOURNAL-GROUP",
    "FX-RECOVERY-EDITOR",
    "FX-SCOPE-2U2P",
];
const SMAP_FAULT_POINTS: &[&str] = &[
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
];
const SMAP_SCHEDULES: &[&str] = &[
    "SCH-CRASH",
    "SCH-DRIFT",
    "SCH-FENCE",
    "SCH-NORMAL",
    "SCH-REORDER",
    "SCH-REPLAY",
    "SCH-SCOPE",
];
const SMAP_ORACLES: &[&str] = &[
    "ORC-ATOMIC-AUTHORITY",
    "ORC-CONTRACT",
    "ORC-CROSSWALK-COMPLETENESS",
    "ORC-EDITOR-JOURNAL",
    "ORC-NEGATIVE-CLOSURE",
    "ORC-OUTCOME-UNKNOWN",
    "ORC-RECOVERY-ATOMICITY",
    "ORC-REPLAY-TRUTH",
    "ORC-SCOPE",
];

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct MandatoryEvidence {
    pub(super) smap_stage_1: SmapStage1,
    pub(super) dvg_13: Dvg13,
    pub(super) b_s1_mandatory_set: MandatorySet,
    pub(super) fx_handoff: FxHandoff,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct SmapStage1 {
    pub(super) id: &'static str,
    pub(super) kind: &'static str,
    pub(super) gates: &'static [&'static str],
    pub(super) evidence_classes: &'static [&'static str],
    pub(super) fixtures: &'static [&'static str],
    pub(super) fault_points: &'static [&'static str],
    pub(super) schedules: &'static [&'static str],
    pub(super) oracles: &'static [&'static str],
    pub(super) aggregate: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct Dvg13 {
    pub(super) id: &'static str,
    pub(super) owners: &'static [&'static str],
    pub(super) fixtures: &'static [&'static str],
    pub(super) fault_points: &'static [&'static str],
    pub(super) schedules: &'static [&'static str],
    pub(super) oracles: &'static [&'static str],
    pub(super) bundles: &'static [&'static str],
    pub(super) walks: &'static [&'static str],
    pub(super) disposition: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct MandatorySet {
    pub(super) id: &'static str,
    pub(super) members: &'static [&'static str],
    pub(super) excluded: &'static [&'static str],
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct FxHandoff {
    pub(super) id: &'static str,
    pub(super) delivery_baseline: GitRev,
    pub(super) execution_baseline: GitRev,
    pub(super) contract_revisions: ContractRevisions,
    pub(super) release_ids: &'static [&'static str],
    pub(super) gate_bundles: &'static [&'static str],
    pub(super) stage_disposition: &'static str,
    pub(super) parent_issue: ParentIssue,
    pub(super) later_cloud_boundary: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct GitRev {
    pub(super) commit: &'static str,
    pub(super) tree: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct ContractRevisions {
    pub(super) parent: &'static str,
    pub(super) delivery: &'static str,
    pub(super) issue: &'static str,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub(super) struct ParentIssue {
    pub(super) issue: &'static str,
    pub(super) title: &'static str,
    pub(super) state: &'static str,
}

pub(super) fn mandatory_evidence() -> Result<MandatoryEvidence, String> {
    let evidence = MandatoryEvidence {
        smap_stage_1: SmapStage1 {
            id: "SMAP-STAGE-1",
            kind: "mandatory-implementation-evidence",
            gates: SMAP_GATES,
            evidence_classes: SMAP_EVIDENCE_CLASSES,
            fixtures: SMAP_FIXTURES,
            fault_points: SMAP_FAULT_POINTS,
            schedules: SMAP_SCHEDULES,
            oracles: SMAP_ORACLES,
            aggregate: "B-S1-MANDATORY-SET",
        },
        dvg_13: Dvg13 {
            id: "DVG-13",
            owners: &["OWN-DVG", "OWN-REL"],
            fixtures: &["FX-CONTRACT-R1", "FX-HANDOFF"],
            fault_points: SMAP_FAULT_POINTS,
            schedules: &["SCH-ABSENT", "SCH-DRIFT", "SCH-NORMAL"],
            oracles: &["ORC-CROSSWALK-COMPLETENESS"],
            bundles: &["B-CONTRACT", "B-HANDOFF"],
            walks: &["WALK-01"],
            disposition: "no PASS-STAGE; no EV-SR; BLOCK-ALL",
        },
        b_s1_mandatory_set: MandatorySet {
            id: "B-S1-MANDATORY-SET",
            members: MANDATORY_SET_MEMBERS,
            excluded: MANDATORY_SET_EXCLUDED,
        },
        fx_handoff: FxHandoff {
            id: "FX-HANDOFF",
            delivery_baseline: GitRev {
                commit: DELIVERY_BASELINE_COMMIT,
                tree: DELIVERY_BASELINE_TREE,
            },
            execution_baseline: GitRev {
                commit: BASELINE_COMMIT,
                tree: BASELINE_TREE,
            },
            contract_revisions: ContractRevisions {
                parent: CONTRACT_REVISION,
                delivery: DELIVERY_CONTRACT_REVISION,
                issue: ISSUE_112_REVISION,
            },
            release_ids: &[PUBLIC_PROTOCOL_RELEASE],
            gate_bundles: MANDATORY_SET_MEMBERS,
            stage_disposition: "implementation-evidence; no PASS-STAGE; no EV-SR",
            parent_issue: ParentIssue {
                issue: PARENT_ISSUE,
                title: PARENT_TITLE,
                state: "closed",
            },
            later_cloud_boundary: "PASS-CLOUD remains out of scope; later controlled-cloud evidence cannot backfill this local Stage 1 gap",
        },
    };
    verify_mandatory_evidence(&evidence)?;
    Ok(evidence)
}

pub(super) fn verify_mandatory_evidence(evidence: &MandatoryEvidence) -> Result<(), String> {
    let members = evidence.b_s1_mandatory_set.members;
    if let Some(bundle) = members
        .iter()
        .find(|bundle| MANDATORY_SET_EXCLUDED.contains(bundle))
    {
        return Err(format!(
            "B-S1-MANDATORY-SET contains excluded bundle {bundle}"
        ));
    }
    if members != MANDATORY_SET_MEMBERS {
        return Err(format!(
            "B-S1-MANDATORY-SET missing member: expected {MANDATORY_SET_MEMBERS:?}, found {members:?}"
        ));
    }
    if evidence.b_s1_mandatory_set.excluded != MANDATORY_SET_EXCLUDED {
        return Err(format!(
            "B-S1-MANDATORY-SET excluded set drifted: expected {MANDATORY_SET_EXCLUDED:?}, found {:?}",
            evidence.b_s1_mandatory_set.excluded
        ));
    }
    if evidence.fx_handoff.gate_bundles != members {
        return Err("FX-HANDOFF gate bundles drifted from B-S1-MANDATORY-SET members".into());
    }
    if evidence.smap_stage_1.aggregate != evidence.b_s1_mandatory_set.id
        || evidence.b_s1_mandatory_set.id != "B-S1-MANDATORY-SET"
    {
        return Err("SMAP-STAGE-1 aggregate drifted from B-S1-MANDATORY-SET".into());
    }
    if !evidence.smap_stage_1.gates.contains(&"DVG-13") {
        return Err("SMAP-STAGE-1 missing gate DVG-13".into());
    }
    if !evidence.smap_stage_1.fixtures.contains(&"FX-HANDOFF") {
        return Err("SMAP-STAGE-1 missing fixture FX-HANDOFF".into());
    }
    if evidence.smap_stage_1.evidence_classes.contains(&"EV-SR") {
        return Err("SMAP-STAGE-1 must not contain EV-SR".into());
    }
    if evidence.dvg_13.id != "DVG-13" || !evidence.dvg_13.fixtures.contains(&"FX-HANDOFF") {
        return Err("DVG-13 is incomplete".into());
    }
    if evidence.fx_handoff.delivery_baseline.commit != DELIVERY_BASELINE_COMMIT
        || evidence.fx_handoff.delivery_baseline.tree != DELIVERY_BASELINE_TREE
    {
        return Err("FX-HANDOFF delivery baseline drifted".into());
    }
    if evidence.fx_handoff.execution_baseline.commit != BASELINE_COMMIT
        || evidence.fx_handoff.execution_baseline.tree != BASELINE_TREE
    {
        return Err("FX-HANDOFF execution baseline drifted".into());
    }
    if evidence.fx_handoff.parent_issue.issue != PARENT_ISSUE
        || evidence.fx_handoff.parent_issue.state != "closed"
    {
        return Err("FX-HANDOFF must name the closed parent #100".into());
    }
    if !evidence
        .fx_handoff
        .later_cloud_boundary
        .contains("PASS-CLOUD")
    {
        return Err("FX-HANDOFF later-cloud boundary drifted".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "stage1_bundle_tests.rs"]
mod tests;
