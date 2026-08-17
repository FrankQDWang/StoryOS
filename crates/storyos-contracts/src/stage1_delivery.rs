use std::collections::BTreeSet;

use serde::Serialize;

use crate::digest::sha256_prefixed;
use crate::stage1_selection::{
    CONTRACT_REVISION, ISSUE_BODY_SHA256, ProofSelection, RequirementBinding,
};

pub(super) const DELIVERY_CONTRACT_REVISION: &str = "stage1-ticketed-delivery-2026-08-17-v16";
pub(super) const DELIVERY_BASELINE_COMMIT: &str = "f5fee494cdcd074b19c7689609a00c67cf620b2d";
pub(super) const DELIVERY_BASELINE_TREE: &str = "044c3aad1a8deb1378d1f729b46c1bda648190e4";
const DELIVERY_TICKET_SET_SHA256: &str =
    "sha256:8572a0163e9196da68ec41ba627b4037e315e4279cb7f3773051fecc8bb6e62c";

const PARENT_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/100";
const FOUNDATION_PULL_REQUEST: &str = "https://github.com/FrankQDWang/StoryOS/pull/102";
pub(super) const FOUNDATION_MERGE_COMMIT: &str = "3468bdaaaa97ef05fd7c6e3f61836d20f1f76496";
pub(super) const FOUNDATION_MERGE_TREE: &str = "485f740ba19616dc4809be3ee50575ef7723432c";
pub(super) const FOUNDATION_CROSSWALK_SHA256: &str =
    "sha256:89bd814824e24e02e6ce0336435880fcf46ec2031bcd11ea3e6aa99a519dae41";
const FOUNDATION_CLAIM_CEILING: &str =
    "contract-foundation-only; no S1 requirement or stage acceptance";

#[derive(Debug, Serialize)]
pub(super) struct DeliveryContract {
    revision: &'static str,
    baseline_commit: &'static str,
    baseline_tree: &'static str,
    ticket_set_sha256: &'static str,
    parent_specification: ParentSpecification,
    historical_foundation: HistoricalFoundation,
    tickets: Vec<DeliveryTicket>,
}

#[derive(Debug, Serialize)]
pub(super) struct DeliveryCoverage {
    requirements: Vec<&'static str>,
    operations: Vec<&'static str>,
    proof_selection: ProofSelection,
}

#[derive(Debug, Serialize)]
struct ParentSpecification {
    issue: &'static str,
    parent: &'static str,
    title: &'static str,
    revision: &'static str,
    issue_body_sha256: &'static str,
    blocked_by: &'static [&'static str],
}

#[derive(Debug, Serialize)]
struct HistoricalFoundation {
    pull_request: &'static str,
    merge_commit: &'static str,
    merge_tree: &'static str,
    crosswalk_sha256: &'static str,
    evidence_role: EvidenceRole,
    claim_ceiling: &'static str,
}

#[derive(Debug, Serialize)]
struct DeliveryTicket {
    sequence: u8,
    responsibility_id: &'static str,
    issue: &'static str,
    parent: &'static str,
    title: &'static str,
    issue_body_sha256: &'static str,
    blocked_by: &'static [&'static str],
    evidence_role: EvidenceRole,
    responsibility: &'static str,
    contract_coverage: TicketCoverage,
}

#[derive(Clone, Copy, Debug)]
struct TicketDefinition {
    responsibility_id: &'static str,
    issue: &'static str,
    title: &'static str,
    issue_body_sha256: &'static str,
    blocked_by: &'static [&'static str],
    evidence_role: EvidenceRole,
    responsibility: &'static str,
    contract_coverage: TicketCoverage,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct TicketCoverage {
    requirements: &'static [&'static str],
    operations: &'static [&'static str],
    gates: &'static [&'static str],
    evidence_classes: &'static [&'static str],
    fixtures: &'static [&'static str],
    fault_points: &'static [&'static str],
    schedules: &'static [&'static str],
    oracles: &'static [&'static str],
    bundles: &'static [&'static str],
    aggregate: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
enum EvidenceRole {
    AcceptanceHandoff,
    ContractBinding,
    HistoricalContractFoundation,
    PlannedRuntimeEvidence,
}

const COVERAGE_01: TicketCoverage = TicketCoverage {
    requirements: &["S1-EVD-001", "SMAP-STAGE-1"],
    operations: &[],
    gates: &["DVG-01"],
    evidence_classes: &["EC-01", "EV-CP"],
    fixtures: &["FX-CONTRACT-R1"],
    fault_points: &["CFP-CONTRACT-DRIFT"],
    schedules: &["SCH-DRIFT"],
    oracles: &["ORC-CONTRACT", "ORC-CROSSWALK-COMPLETENESS"],
    bundles: &["B-CONTRACT"],
    aggregate: None,
};
const COVERAGE_02: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-001", "S1-REQ-005", "S1-EVD-005"],
    operations: &["getProtocolProfile"],
    gates: &["DVG-01"],
    evidence_classes: &["EC-01", "EV-INT"],
    fixtures: &["FX-CONTRACT-R1"],
    fault_points: &["CFP-CONTRACT-DRIFT"],
    schedules: &["SCH-DRIFT"],
    oracles: &["ORC-CONTRACT"],
    bundles: &["B-CONTRACT"],
    aggregate: None,
};
const COVERAGE_03: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-001", "S1-REQ-005", "S1-EVD-005"],
    operations: &["getProject", "getChapter"],
    gates: &["DVG-07"],
    evidence_classes: &["EC-07", "EV-SE"],
    fixtures: &["FX-SCOPE-2U2P"],
    fault_points: &["CFP-SCOPE-BEFORE-QUERY"],
    schedules: &["SCH-SCOPE"],
    oracles: &["ORC-NEGATIVE-CLOSURE", "ORC-SCOPE"],
    bundles: &["B-SCOPE"],
    aggregate: None,
};
const COVERAGE_03B: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-005", "S1-EVD-005"],
    operations: &["createProjectCommandChallenge"],
    gates: &["DVG-07"],
    evidence_classes: &["EC-05", "EV-IT", "EV-INT", "EV-SE"],
    fixtures: &["FX-SCOPE-2U2P"],
    fault_points: &["CFP-SCOPE-BEFORE-QUERY"],
    schedules: &["SCH-SCOPE"],
    oracles: &["ORC-NEGATIVE-CLOSURE", "ORC-SCOPE"],
    bundles: &["B-SCOPE"],
    aggregate: None,
};
const COVERAGE_04: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-002", "S1-JRN-001", "S1-EVD-002"],
    operations: &["createEditorSession", "getEditorSession"],
    gates: &["DVG-02"],
    evidence_classes: &["EC-02"],
    fixtures: &["FX-JOURNAL-GROUP"],
    fault_points: &[
        "CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP",
        "CFP-EDITOR-BEFORE-JOURNAL-DURABILITY",
    ],
    schedules: &["SCH-NORMAL"],
    oracles: &["ORC-EDITOR-JOURNAL"],
    bundles: &["B-EDITOR"],
    aggregate: None,
};
const COVERAGE_05: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-003", "S1-EVD-003"],
    operations: &["applyAuthorEdit"],
    gates: &["DVG-03"],
    evidence_classes: &["EC-03", "EV-IT"],
    fixtures: &["FX-CORE-PROPOSAL"],
    fault_points: &[
        "CFP-ADMISSION-BEFORE-CORE",
        "CFP-CORE-AFTER-COMMIT-BEFORE-ACK",
        "CFP-CORE-BEFORE-COMMIT",
    ],
    schedules: &["SCH-CRASH"],
    oracles: &["ORC-ATOMIC-AUTHORITY"],
    bundles: &["B-CORE"],
    aggregate: None,
};
const COVERAGE_06: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-002", "S1-JRN-001", "S1-EVD-002"],
    operations: &[
        "createEditorSession",
        "getEditorSession",
        "createProjectCommandChallenge",
        "applyAuthorEdit",
        "getChapter",
    ],
    gates: &["DVG-01", "DVG-02", "DVG-03"],
    evidence_classes: &["EC-01", "EC-02", "EC-03", "EV-IT", "EV-INT"],
    fixtures: &["FX-CONTRACT-R1", "FX-EDITOR-IME", "FX-JOURNAL-GROUP"],
    fault_points: &[
        "CFP-ADMISSION-BEFORE-CORE",
        "CFP-CORE-BEFORE-COMMIT",
        "CFP-EDITOR-AFTER-JOURNAL-BEFORE-GROUP",
        "CFP-EDITOR-BEFORE-GROUP-ADMISSION",
        "CFP-EDITOR-BEFORE-JOURNAL-DURABILITY",
    ],
    schedules: &["SCH-NORMAL"],
    oracles: &["ORC-ATOMIC-AUTHORITY", "ORC-CONTRACT", "ORC-EDITOR-JOURNAL"],
    bundles: &["B-CONTRACT", "B-CORE", "B-EDITOR"],
    aggregate: None,
};
const COVERAGE_07: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-004", "S1-EVD-004"],
    operations: &["getCommand"],
    gates: &["DVG-08"],
    evidence_classes: &["EC-04"],
    fixtures: &["FX-RECOVERY-EDITOR"],
    fault_points: &[
        "CFP-CORE-AFTER-COMMIT-BEFORE-ACK",
        "CFP-EDITOR-AFTER-SETTLEMENT-BEFORE-ACK",
    ],
    schedules: &["SCH-CRASH"],
    oracles: &["ORC-OUTCOME-UNKNOWN", "ORC-RECOVERY-ATOMICITY"],
    bundles: &["B-RECOVERY"],
    aggregate: None,
};
const COVERAGE_08: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-004", "S1-EVD-004"],
    operations: &["getEditorSession", "getCommand"],
    gates: &["DVG-08"],
    evidence_classes: &["EC-04"],
    fixtures: &["FX-RECOVERY-EDITOR"],
    fault_points: &[
        "CFP-ADMISSION-EXPIRY",
        "CFP-EDITOR-AFTER-ADMISSION-BEFORE-CORE",
    ],
    schedules: &["SCH-CRASH"],
    oracles: &["ORC-RECOVERY-ATOMICITY"],
    bundles: &["B-RECOVERY"],
    aggregate: None,
};
const COVERAGE_09: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-004", "S1-EVD-004"],
    operations: &["activityStream", "getSnapshot"],
    gates: &["DVG-09", "DVG-11"],
    evidence_classes: &["EC-05"],
    fixtures: &["FX-RECOVERY-EDITOR"],
    fault_points: &[
        "CFP-FENCE-AFTER-TAKEOVER",
        "CFP-LATE-RESULT",
        "CFP-REPLAY-AFTER-GENERATION-SNAPSHOT",
        "CFP-REPLAY-BELOW-FLOOR",
    ],
    schedules: &["SCH-FENCE", "SCH-REORDER", "SCH-REPLAY"],
    oracles: &["ORC-REPLAY-TRUTH"],
    bundles: &["B-REPLAY"],
    aggregate: None,
};
const COVERAGE_10: TicketCoverage = TicketCoverage {
    requirements: &["S1-REQ-006", "S1-EVD-001", "S1-EVD-006", "SMAP-STAGE-1"],
    operations: &[],
    gates: &["DVG-13"],
    evidence_classes: &[],
    fixtures: &["FX-HANDOFF"],
    fault_points: &[],
    schedules: &[],
    oracles: &[],
    bundles: &["B-HANDOFF"],
    aggregate: Some("B-S1-MANDATORY-SET"),
};

const STAGE_1_DELIVERY_TICKET_COUNT: usize = 12;

const TICKET_DEFINITIONS: [TicketDefinition; STAGE_1_DELIVERY_TICKET_COUNT] = [
    TicketDefinition {
        responsibility_id: "S1-TICKET-01",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/103",
        title: "Rebind the Stage 1 Contract Spine to Ticketed Delivery",
        issue_body_sha256: "de4a4fda4ad2de60d8bc9f3c8caf8724070ca5b6e36c8aaa74454f7e634abf30",
        blocked_by: &[],
        evidence_role: EvidenceRole::ContractBinding,
        responsibility: "bind the accepted Stage 1 specification and ticket sequence",
        contract_coverage: COVERAGE_01,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-02",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/104",
        title: "Boot the Protected Web Client Against the Release 1 Protocol Profile",
        issue_body_sha256: "cd1a9130c041f01be000ce87cef1b43df46afdbcc94fc4bf697b3061b78d1650",
        blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/103"],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "validate the protected client protocol identity",
        contract_coverage: COVERAGE_02,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-03",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/105",
        title: "Open the Controlled Project and Current Chapter End to End",
        issue_body_sha256: "2ea7e9fdcaec104c9cc3911ca3104e8d0c8392987fa29306d273147e53803d4f",
        blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/104"],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "open one isolated Project and current Chapter",
        contract_coverage: COVERAGE_03,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-03A",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/119",
        title: "Harden Sensitive Read Origin Admission with Standard URL Parsing",
        issue_body_sha256: "e28c601251b1fd0905f0b1bf1d38a249102f41613a6cfc7096eec869097d9f6e",
        blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/105"],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "harden sensitive read origin admission",
        contract_coverage: COVERAGE_03,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-03B",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/121",
        title: "Implement Project-Scoped Anti-Forgery Challenge Admission",
        issue_body_sha256: "f9fa15fd8b28fcc6857f16c48cbca164db571bd945369a9204210e7789b67f0f",
        blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/119"],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "establish the Project-scoped command challenge and pre-domain idempotency fence",
        contract_coverage: COVERAGE_03B,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-04",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/106",
        title: "Create an Editor Session and Persist One Pending Intent",
        issue_body_sha256: "db44ffa60cf52d0e59c54e11c6496ac4c48ea3d1ef4c038dc40e00a3e2b1a9f9",
        blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/121"],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "persist one pending editor intent",
        contract_coverage: COVERAGE_04,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-05",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/107",
        title: "Settle One Direct Author Edit into an Authoritative Revision",
        issue_body_sha256: "755e6d56422445da42ce4ab904f1c2894e5ab1552e67f52497ba74fd07bf3b77",
        blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/106"],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "settle one Direct Author Action as an authoritative Revision",
        contract_coverage: COVERAGE_05,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-06",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/108",
        title: "Complete Bounded Manual Input and IME Semantics",
        issue_body_sha256: "faeaf13ab0a8689797523b4e4b3ced6d2776080876f67e60d974bf7642984a4c",
        blocked_by: &[],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "complete bounded manual input, IME, and same-session base roll-forward semantics",
        contract_coverage: COVERAGE_06,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-07",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/109",
        title: "Reconcile Acknowledgement Loss without Duplicate Authority",
        issue_body_sha256: "1fd3e245a51e3eed5da7943793fa633098d5a4870756d7e6bf2b522867d83529",
        blocked_by: &[
            "https://github.com/FrankQDWang/StoryOS/issues/108",
            "https://github.com/FrankQDWang/StoryOS/issues/56",
        ],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "reconcile acknowledgement loss without duplicate authority",
        contract_coverage: COVERAGE_07,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-08",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/110",
        title: "Recover Settled and Unsettled Edits across Reload and Restart",
        issue_body_sha256: "526e75701f2b9acb1461fc99a6e68c6bf533f96aaa86c4e7e5a18244109ce196",
        blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/109"],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "recover settled and unsettled edits after reload and restart",
        contract_coverage: COVERAGE_08,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-09",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/111",
        title: "Fence Stale Writers and Resync across Replay Generations",
        issue_body_sha256: "78ac6a7fbec0886fc4c993140b1b3dd4c9da13db3b15fb36706a4f4021c7839f",
        blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/110"],
        evidence_role: EvidenceRole::PlannedRuntimeEvidence,
        responsibility: "fence stale writers and resync replay generations",
        contract_coverage: COVERAGE_09,
    },
    TicketDefinition {
        responsibility_id: "S1-TICKET-10",
        issue: "https://github.com/FrankQDWang/StoryOS/issues/112",
        title: "Complete the Stage 1 Mandatory Evidence and Handoff",
        issue_body_sha256: "1b708c255900282ae043c879c11b16a99c5e6fe271c541fee004ac38dad68373",
        blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/111"],
        evidence_role: EvidenceRole::AcceptanceHandoff,
        responsibility: "assemble the mandatory Stage 1 evidence and handoff",
        contract_coverage: COVERAGE_10,
    },
];

pub(super) fn delivery_contract() -> DeliveryContract {
    DeliveryContract {
        revision: DELIVERY_CONTRACT_REVISION,
        baseline_commit: DELIVERY_BASELINE_COMMIT,
        baseline_tree: DELIVERY_BASELINE_TREE,
        ticket_set_sha256: DELIVERY_TICKET_SET_SHA256,
        parent_specification: ParentSpecification {
            issue: PARENT_ISSUE,
            parent: "https://github.com/FrankQDWang/StoryOS/issues/1",
            title: "Implement the Stage 1 Production-Shaped Manual-Editor Risk Slice",
            revision: CONTRACT_REVISION,
            issue_body_sha256: ISSUE_BODY_SHA256,
            blocked_by: &["https://github.com/FrankQDWang/StoryOS/issues/77"],
        },
        historical_foundation: HistoricalFoundation {
            pull_request: FOUNDATION_PULL_REQUEST,
            merge_commit: FOUNDATION_MERGE_COMMIT,
            merge_tree: FOUNDATION_MERGE_TREE,
            crosswalk_sha256: FOUNDATION_CROSSWALK_SHA256,
            evidence_role: EvidenceRole::HistoricalContractFoundation,
            claim_ceiling: FOUNDATION_CLAIM_CEILING,
        },
        tickets: TICKET_DEFINITIONS
            .iter()
            .enumerate()
            .map(|(index, definition)| DeliveryTicket {
                sequence: u8::try_from(index + 1).expect("Stage 1 ticket sequence must fit u8"),
                responsibility_id: definition.responsibility_id,
                issue: definition.issue,
                parent: PARENT_ISSUE,
                title: definition.title,
                issue_body_sha256: definition.issue_body_sha256,
                blocked_by: definition.blocked_by,
                evidence_role: definition.evidence_role,
                responsibility: definition.responsibility,
                contract_coverage: definition.contract_coverage,
            })
            .collect(),
    }
}

pub(super) fn delivery_coverage(
    contract: &DeliveryContract,
    requirements: &[RequirementBinding],
    operations: &'static [&'static str],
    proof: &ProofSelection,
) -> Result<DeliveryCoverage, String> {
    if contract.tickets.len() != STAGE_1_DELIVERY_TICKET_COUNT {
        return Err(format!(
            "Stage 1 delivery must contain {STAGE_1_DELIVERY_TICKET_COUNT} tickets, found {}",
            contract.tickets.len()
        ));
    }
    let requirements = exact_coverage(
        "requirement",
        contract
            .tickets
            .iter()
            .flat_map(|ticket| ticket.contract_coverage.requirements.iter().copied()),
        requirements.iter().map(|requirement| requirement.id),
    )?;
    let operations = exact_coverage(
        "operation",
        contract
            .tickets
            .iter()
            .flat_map(|ticket| ticket.contract_coverage.operations.iter().copied()),
        operations.iter().copied(),
    )?;
    proof_coverage(contract, "gate", proof.gates, |coverage| coverage.gates)?;
    proof_coverage(
        contract,
        "evidence class",
        proof.evidence_classes,
        |coverage| coverage.evidence_classes,
    )?;
    proof_coverage(contract, "fixture", proof.fixtures, |coverage| {
        coverage.fixtures
    })?;
    proof_coverage(contract, "fault point", proof.fault_points, |coverage| {
        coverage.fault_points
    })?;
    proof_coverage(contract, "schedule", proof.schedules, |coverage| {
        coverage.schedules
    })?;
    proof_coverage(contract, "oracle", proof.oracles, |coverage| {
        coverage.oracles
    })?;
    proof_coverage(contract, "bundle", proof.bundles, |coverage| {
        coverage.bundles
    })?;
    let aggregates = contract
        .tickets
        .iter()
        .filter_map(|ticket| ticket.contract_coverage.aggregate);
    exact_coverage("aggregate", aggregates, std::iter::once(proof.aggregate))?;
    let actual_ticket_set_sha256 = ticket_set_sha256(&contract.tickets)?;
    if contract.ticket_set_sha256 != DELIVERY_TICKET_SET_SHA256
        || actual_ticket_set_sha256 != DELIVERY_TICKET_SET_SHA256
    {
        return Err(format!(
            "Stage 1 delivery ticket binding drifted: expected {DELIVERY_TICKET_SET_SHA256}, found {actual_ticket_set_sha256}"
        ));
    }
    Ok(DeliveryCoverage {
        requirements,
        operations,
        proof_selection: *proof,
    })
}

fn ticket_set_sha256(tickets: &[DeliveryTicket]) -> Result<String, String> {
    let bytes = serde_json::to_vec(tickets)
        .map_err(|error| format!("Stage 1 delivery ticket binding is not serializable: {error}"))?;
    Ok(sha256_prefixed(bytes))
}

fn proof_coverage(
    contract: &DeliveryContract,
    kind: &str,
    expected: &'static [&'static str],
    selected: fn(&TicketCoverage) -> &'static [&'static str],
) -> Result<Vec<&'static str>, String> {
    exact_coverage(
        kind,
        contract
            .tickets
            .iter()
            .flat_map(|ticket| selected(&ticket.contract_coverage).iter().copied()),
        expected.iter().copied(),
    )
}

fn exact_coverage(
    kind: &str,
    actual: impl Iterator<Item = &'static str>,
    expected: impl Iterator<Item = &'static str>,
) -> Result<Vec<&'static str>, String> {
    let actual = actual.collect::<BTreeSet<_>>();
    let expected = expected.collect::<Vec<_>>();
    let expected_set = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected_set {
        return Err(format!(
            "Stage 1 delivery {kind} coverage drifted: expected {expected_set:?}, found {actual:?}"
        ));
    }
    Ok(expected)
}

#[cfg(test)]
#[path = "stage1_delivery_tests.rs"]
mod tests;
