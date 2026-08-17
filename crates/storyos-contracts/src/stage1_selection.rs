pub(super) const CONTRACT_REVISION: &str =
    "stage1-production-shaped-manual-editor-risk-slice-2026-08-18-v6";
pub(super) const BASELINE_COMMIT: &str = "bab4c0ac5ca3da20b01ea1d61783aaba414f493f";
pub(super) const BASELINE_TREE: &str = "b36933e1d9c7d6e7bb19879a728d3a35483937bc";
pub(super) const ISSUE_BODY_SHA256: &str =
    "5910f72dadd69b1b9292c623290770fdcbaaac039d980a02812d901b75ac96c6";
pub(super) const ROUTE_CATALOG_PATH: &str =
    "docs/foundation/versioned-protocol-release-1-route-catalog.json";
pub(super) const PERSISTENCE_CATALOG_PATH: &str =
    "docs/foundation/postgresql-release-1-persistence-catalog.json";

pub(super) const SOURCE_PATHS: &[&str] = &[
    "AGENTS.md",
    "CONTEXT.md",
    "GOAL.md",
    "docs/agents/issue-tracker.md",
    "docs/adr/0004-adopt-postgresql-service-and-project-isolation-boundary.md",
    "docs/adr/0006-adopt-foundation-monorepo-governance.md",
    "docs/adr/0007-preserve-process-separable-server-worker-boundary.md",
    "docs/adr/0009-require-snapshot-resync-at-replay-generation-boundaries.md",
    "docs/adr/0010-require-lifecycle-proof-before-recovery-visibility.md",
    "docs/adr/0012-adopt-deterministic-contract-verification.md",
    "docs/adr/0013-trust-the-storyos-web-client-for-author-command-admission.md",
    "docs/foundation/ai-independent-editor-first-release-baseline-and-handoff-criteria.md",
    "docs/foundation/author-command-admission.md",
    "docs/foundation/deterministic-verification-and-failure-recovery-gates.md",
    "docs/foundation/manuscript-revision-proposal-state-machine.md",
    "docs/foundation/modular-monolith-and-repository-governance-boundaries.md",
    "docs/foundation/postgresql-project-storage-isolation-and-migration-contract.md",
    PERSISTENCE_CATALOG_PATH,
    "docs/foundation/run-event-mailbox-snapshot-retention-and-archival-semantics.md",
    "docs/foundation/storyos-service-client-external-trust-boundaries-threat-model.md",
    ROUTE_CATALOG_PATH,
    "docs/foundation/versioned-command-query-artifact-event-protocol.md",
    "docs/foundation/web-editor-session-synchronization-and-recovery-semantics.md",
];

pub(super) const OPERATION_IDS: &[&str] = &[
    "getProtocolProfile",
    "getProject",
    "getChapter",
    "createEditorSession",
    "getEditorSession",
    "createProjectCommandChallenge",
    "applyAuthorEdit",
    "getApplyAuthorEditOutcome",
    "getCommand",
    "activityStream",
    "getSnapshot",
];

#[derive(Debug, Serialize)]
pub(super) struct RequirementBinding {
    pub(super) id: &'static str,
    owners: &'static [&'static str],
    project_scope: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct ProofSelection {
    pub(super) gates: &'static [&'static str],
    pub(super) evidence_classes: &'static [&'static str],
    pub(super) fixtures: &'static [&'static str],
    pub(super) fault_points: &'static [&'static str],
    pub(super) schedules: &'static [&'static str],
    pub(super) oracles: &'static [&'static str],
    pub(super) bundles: &'static [&'static str],
    pub(super) aggregate: &'static str,
    disposition: &'static str,
}

pub(super) fn requirement_bindings() -> Vec<RequirementBinding> {
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

pub(super) fn proof_selection() -> ProofSelection {
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
            "CFP-EDITOR-AFTER-OUTCOME-RESPONSE-BEFORE-JOURNAL",
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
            "SCH-UNKNOWN",
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
use serde::Serialize;
