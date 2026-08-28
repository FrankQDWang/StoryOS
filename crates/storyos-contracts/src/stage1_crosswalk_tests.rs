use serde_json::{Value, json};

use super::{
    CrosswalkError, GENERATED_CROSSWALK_PATH, ROUTE_CATALOG_PATH, generate_crosswalk,
    read_baseline_file, read_baseline_json, repository_root, validate_operations,
    verify_baseline_tree, verify_delivery_baseline_tree, verify_historical_foundation_crosswalk,
};

fn route_catalog() -> Value {
    read_baseline_json(repository_root(), ROUTE_CATALOG_PATH).expect("route catalog should load")
}

#[test]
fn checked_in_crosswalk_matches_fresh_generation() {
    let actual = std::fs::read(repository_root().join(GENERATED_CROSSWALK_PATH))
        .expect("checked-in crosswalk should exist");
    let expected =
        generate_crosswalk(repository_root()).expect("crosswalk generation should succeed");
    assert_eq!(actual, expected);
}

#[test]
fn generated_crosswalk_releases_only_the_historical_web_client_owner() {
    let bytes = generate_crosswalk(repository_root()).expect("crosswalk generation should succeed");
    let value: Value = serde_json::from_slice(&bytes).expect("generated crosswalk is JSON");

    assert_eq!(
        value["schema_id"],
        "storyos.evidence.stage1-contract-crosswalk.v4"
    );
    assert_eq!(
        value["declarations"]["tracker_verification"],
        json!({
            "historical_reusable_owner_responsibility_ids": ["S1-TICKET-09A"]
        })
    );
}

#[test]
fn missing_stage1_operation_is_rejected() {
    let mut catalog = route_catalog();
    catalog["operations"]
        .as_array_mut()
        .expect("operations should be an array")
        .retain(|operation| operation["operation_id"] != "applyAuthorEdit");
    assert!(matches!(
        validate_operations(&catalog),
        Err(CrosswalkError::Invalid(message))
            if message == "missing Stage 1 operation applyAuthorEdit"
    ));
}

#[test]
fn missing_project_scope_precondition_is_rejected() {
    let mut catalog = route_catalog();
    let operation = catalog["operations"]
        .as_array_mut()
        .expect("operations should be an array")
        .iter_mut()
        .find(|operation| operation["operation_id"] == "applyAuthorEdit")
        .expect("applyAuthorEdit should exist");
    operation["preconditions"]
        .as_array_mut()
        .expect("preconditions should be an array")
        .retain(|value| value != "server_derived_project_scope");
    assert!(matches!(
        validate_operations(&catalog),
        Err(CrosswalkError::Invalid(message))
            if message.contains("applyAuthorEdit Project Scope")
    ));
}

#[test]
fn baseline_reader_never_falls_through_to_the_worktree() {
    let path = "crates/storyos-contracts/src/stage1_delivery_tests.rs";
    let worktree = std::fs::read_to_string(repository_root().join(path))
        .expect("worktree delivery tests should exist");
    let baseline =
        read_baseline_file(repository_root(), path).expect("baseline delivery tests should exist");

    assert!(worktree.contains("acknowledgement_loss_ticket_matches_the_current_process_profile"));
    assert!(
        !String::from_utf8(baseline)
            .expect("baseline delivery tests should be UTF-8")
            .contains("acknowledgement_loss_ticket_matches_the_current_process_profile")
    );
}

#[test]
fn wrong_baseline_tree_is_rejected() {
    assert!(matches!(
        verify_baseline_tree(repository_root(), "wrong-tree"),
        Err(CrosswalkError::Invalid(message)) if message.contains("baseline tree drifted")
    ));
}

#[test]
fn wrong_delivery_baseline_tree_is_rejected() {
    assert!(matches!(
        verify_delivery_baseline_tree(repository_root(), "wrong-tree"),
        Err(CrosswalkError::Invalid(message))
            if message.contains("delivery baseline tree drifted")
    ));
}

#[test]
fn wrong_historical_foundation_crosswalk_digest_is_rejected() {
    assert!(matches!(
        verify_historical_foundation_crosswalk(repository_root(), "sha256:wrong"),
        Err(CrosswalkError::Invalid(message))
            if message.contains("PR #102 foundation crosswalk drifted")
    ));
}

#[test]
fn generated_crosswalk_proves_the_complete_stage1_mandatory_bundle() {
    let bytes = generate_crosswalk(repository_root()).expect("crosswalk generation should succeed");
    let value: Value = serde_json::from_slice(&bytes).expect("generated crosswalk is JSON");
    assert_eq!(
        value["verifications"]["mandatory_evidence"],
        expected_mandatory_evidence()
    );
}

fn expected_mandatory_evidence() -> Value {
    json!({
        "smap_stage_1": {
            "id": "SMAP-STAGE-1",
            "kind": "mandatory-implementation-evidence",
            "gates": [
                "DVG-01", "DVG-02", "DVG-03", "DVG-07", "DVG-08", "DVG-09", "DVG-11", "DVG-13"
            ],
            "evidence_classes": [
                "EC-01", "EC-02", "EC-03", "EC-04", "EC-05", "EC-07",
                "EV-CP", "EV-IT", "EV-INT", "EV-SE"
            ],
            "fixtures": [
                "FX-CONTRACT-R1", "FX-CORE-PROPOSAL", "FX-EDITOR-IME", "FX-HANDOFF",
                "FX-JOURNAL-GROUP", "FX-RECOVERY-EDITOR", "FX-SCOPE-2U2P"
            ],
            "fault_points": [
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
                "CFP-SCOPE-BEFORE-QUERY"
            ],
            "schedules": [
                "SCH-CRASH", "SCH-DRIFT", "SCH-FENCE", "SCH-NORMAL",
                "SCH-REORDER", "SCH-REPLAY", "SCH-SCOPE"
            ],
            "oracles": [
                "ORC-ATOMIC-AUTHORITY",
                "ORC-CONTRACT",
                "ORC-CROSSWALK-COMPLETENESS",
                "ORC-EDITOR-JOURNAL",
                "ORC-NEGATIVE-CLOSURE",
                "ORC-OUTCOME-UNKNOWN",
                "ORC-RECOVERY-ATOMICITY",
                "ORC-REPLAY-TRUTH",
                "ORC-SCOPE"
            ],
            "aggregate": "B-S1-MANDATORY-SET"
        },
        "dvg_13": {
            "id": "DVG-13",
            "owners": ["OWN-DVG", "OWN-REL"],
            "fixtures": ["FX-CONTRACT-R1", "FX-HANDOFF"],
            "fault_points": [
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
                "CFP-SCOPE-BEFORE-QUERY"
            ],
            "schedules": ["SCH-ABSENT", "SCH-DRIFT", "SCH-NORMAL"],
            "oracles": ["ORC-CROSSWALK-COMPLETENESS"],
            "bundles": ["B-CONTRACT", "B-HANDOFF"],
            "walks": ["WALK-01"],
            "disposition": "no PASS-STAGE; no EV-SR; BLOCK-ALL"
        },
        "b_s1_mandatory_set": {
            "id": "B-S1-MANDATORY-SET",
            "members": [
                "B-CONTRACT", "B-SCOPE", "B-EDITOR", "B-CORE",
                "B-RECOVERY", "B-REPLAY", "B-HANDOFF"
            ],
            "excluded": [
                "B-RESTORE", "B-CONTEXT", "B-FAKE", "B-REAL-ADVISORY",
                "B-ABSENT", "B-EVAL", "B-MEASURE"
            ]
        },
        "fx_handoff": {
            "id": "FX-HANDOFF",
            "delivery_baseline": {
                "commit": "3523494b0b4d934032576b822895b8f3ebc7ef87",
                "tree": "5e155a851b1debf555e1d5f20fde09956b609c5d"
            },
            "execution_baseline": {
                "commit": "bab4c0ac5ca3da20b01ea1d61783aaba414f493f",
                "tree": "b36933e1d9c7d6e7bb19879a728d3a35483937bc"
            },
            "contract_revisions": {
                "parent": "stage1-production-shaped-manual-editor-risk-slice-2026-08-18-v6",
                "delivery": "stage1-ticketed-delivery-2026-08-28-v22",
                "issue": "issue-112-provenance-workspace-isolation-2026-08-23-v4"
            },
            "release_ids": ["storyos.public.release.1"],
            "gate_bundles": [
                "B-CONTRACT", "B-SCOPE", "B-EDITOR", "B-CORE",
                "B-RECOVERY", "B-REPLAY", "B-HANDOFF"
            ],
            "stage_disposition": "implementation-evidence; no PASS-STAGE; no EV-SR",
            "parent_issue": {
                "issue": "https://github.com/FrankQDWang/StoryOS/issues/100",
                "title": "Implement the Stage 1 Production-Shaped Manual-Editor Risk Slice",
                "state": "closed"
            },
            "later_cloud_boundary": "PASS-CLOUD remains out of scope; later controlled-cloud evidence cannot backfill this local Stage 1 gap"
        }
    })
}

#[test]
fn generated_crosswalk_proves_complete_production_shaped_provenance() {
    let bytes = generate_crosswalk(repository_root()).expect("crosswalk generation should succeed");
    let value: Value = serde_json::from_slice(&bytes).expect("generated crosswalk is JSON");
    assert_eq!(
        value["verifications"]["provenance_evidence"],
        expected_provenance_evidence()
    );
}

fn expected_provenance_evidence() -> Value {
    json!({
        "s1_req_006": {
            "id": "S1-REQ-006",
            "owners": [
                "OWN-GOV", "OWN-WEB", "OWN-PROTO", "OWN-CORE", "OWN-PG", "OWN-ADM", "OWN-RET"
            ],
            "claim": "production-shaped",
            "forbidden": [
                "prototype",
                "in-memory-authority",
                "local-file-authority",
                "test-adapter",
                ".reference/**"
            ]
        },
        "s1_evd_006": {
            "id": "S1-EVD-006",
            "owners": ["OWN-GOV"],
            "claim": "no-contamination",
            "forbidden": [
                "prototype",
                "in-memory",
                "test-adapter",
                "local-authority",
                ".reference/**"
            ]
        },
        "workspace": {
            "crates": [
                "crates/storyos-adapter-postgres",
                "crates/storyos-application",
                "crates/storyos-contracts",
                "crates/storyos-core",
                "crates/storyos-server"
            ],
            "excluded_trees": ["prototypes/**", ".reference/**"]
        },
        "journey_evidence": {
            "requirement": "S1-JRN-001",
            "test": "apps/web/test/browser-exact-dist/s1-jrn-001.integration.test.ts",
            "page": "vite-production",
            "server": "storyos-server",
            "store": "postgresql"
        }
    })
}

#[test]
fn generated_crosswalk_proves_complete_residual_risk_handoff() {
    let bytes = generate_crosswalk(repository_root()).expect("crosswalk generation should succeed");
    let value: Value = serde_json::from_slice(&bytes).expect("generated crosswalk is JSON");
    assert_eq!(
        value["verifications"]["handoff_evidence"],
        expected_handoff_evidence()
    );
}

fn expected_handoff_evidence() -> Value {
    json!({
        "hnd_003": {
            "id": "HND-003",
            "evaluated_stage": "Stage 1",
            "mandatory_map": "SMAP-STAGE-1",
            "disposition": "implementation-evidence-completeness; no EV-SR; no PASS-STAGE"
        },
        "hnd_004": {
            "id": "HND-004",
            "status": "not-emitted"
        },
        "hnd_005": {
            "id": "HND-005",
            "status": "not-emitted"
        },
        "parent_issue": {
            "issue": "https://github.com/FrankQDWang/StoryOS/issues/100",
            "title": "Implement the Stage 1 Production-Shaped Manual-Editor Risk Slice",
            "state": "closed",
            "closeout": "performed"
        },
        "next_issue": "none; no Stage 2 issue created",
        "residual_risks": {
            "later_stages": "unrun",
            "external_model_provider": "unintegrated",
            "prototype_only": "unpromoted",
            "reference_tree": "not-a-production-dependency",
            "stage_release": "not-claimed",
            "parent_closeout": "performed",
            "later_cloud": "not-claimed"
        },
        "out_of_scope": {
            "later_stage_maps": ["SMAP-STAGE-2", "SMAP-STAGE-3", "SMAP-STAGE-4"],
            "later_journeys": ["S2-JRN-001", "S3-JRN-001", "S4-JRN-001"],
            "external_model_provider": [
                "FX-FAKE-MODEL", "FX-REAL-MODEL-ADVISORY", "B-FAKE", "B-REAL-ADVISORY"
            ],
            "prototype_only": ["prototypes/**"],
            "reference_tree": [".reference/**"]
        },
        "emitted": [],
        "forbidden_emissions": ["EV-SR", "PASS-STAGE", "PASS-CLOUD"]
    })
}
