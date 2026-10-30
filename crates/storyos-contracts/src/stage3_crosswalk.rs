use std::path::Path;

use crate::stage_crosswalk::{Binding, EvidenceClass, StageCrosswalk};
use crate::stage1_crosswalk::CrosswalkError;

/// Checked-in deterministic output owned by this module.
pub const GENERATED_STAGE3_CROSSWALK_PATH: &str =
    "generated/evidence/stage3-contract-crosswalk.json";

const SCHEMA_ID: &str = "storyos.evidence.stage3-contract-crosswalk.v1";
const EVALUATED_STAGE: &str = "Stage 3";
const MANDATORY_MAP: &str = "SMAP-STAGE-3";
const CONTRACT_REVISION: &str = "stage3-production-repair-2026-09-30-v1";
const PARENT_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/361";
const TICKET_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/391";
const BASELINE_COMMIT: &str = "9a9471187984b6791ed35870fae80c5addeb85ef";
const BASELINE_TREE: &str = "733bcdda80522ab897c79277f313e05331c8793a";
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
const PROOF: &str = "docs/foundation/product-delivery-proof-selection.md";
const WORKSPACE: &str = "apps/web/test/browser-exact-dist/s2-workspace.integration.test.ts";
const STAGE2_JOURNEY: &str = "apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts";
const PHYSICAL_DRILL: &str =
    "apps/web/test/browser-exact-dist/s2-physical-drill.integration.test.ts";
const PRODUCTION_HOST: &str =
    "apps/web/test/browser-exact-dist/production-host.integration.test.ts";
const FAKE_DECISION: &str =
    "apps/web/test/node-postgresql/complete-fake-model-decision-http.integration.test.ts";
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
const OPEN_BLOCK: &str =
    "apps/web/test/node-postgresql/open-block-proposal-http.integration.test.ts";
const MULTI: &str =
    "apps/web/test/node-postgresql/settle-multi-operation-selections-http.integration.test.ts";
const STREAM: &str =
    "apps/web/test/node-postgresql/stream-proposal-generation-http.integration.test.ts";
const GENERATION_CLAIM: &str =
    "apps/web/test/node-postgresql/continue-proposal-generation-claim-http.integration.test.ts";
const ACCEPT: &str = "apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts";
const REJECT: &str =
    "apps/web/test/node-postgresql/reject-proposal-operations-http.integration.test.ts";
const ACCEPT_REFUSAL: &str =
    "apps/web/test/node-postgresql/acceptance-refusal-http.integration.test.ts";
const UNDO: &str = "crates/storyos-adapter-postgres/src/command_sequence_tests/proposal_acceptance_undo_chain_tests.rs";
const COMPARISON: &str = "crates/storyos-core/src/revision_comparison_tests.rs";
const PHYSICAL_SCRIPT: &str = "scripts/verify-recovery-hold.sh";
const ADMITTED_TARGET: &str =
    "apps/web/test/node-postgresql/admitted-proposal-target-http.integration.test.ts";
const CANDIDATE_EDIT: &str =
    "apps/web/test/node-postgresql/edit-proposal-candidate-http.integration.test.ts";
const CAPTURED_MEMORY: &str =
    "apps/web/test/browser-exact-dist/production-captured-memory.integration.test.ts";
const RUN_EVIDENCE: &str =
    "apps/web/test/browser-exact-dist/production-run-evidence.integration.test.ts";

#[path = "stage3_crosswalk_bindings.rs"]
mod bindings;

use bindings::BINDINGS;

const CROSSWALK: StageCrosswalk = StageCrosswalk {
    generated_path: GENERATED_STAGE3_CROSSWALK_PATH,
    schema_id: SCHEMA_ID,
    evaluated_stage: EVALUATED_STAGE,
    mandatory_map: MANDATORY_MAP,
    contract_revision: CONTRACT_REVISION,
    parent_issue: PARENT_ISSUE,
    ticket_issue: TICKET_ISSUE,
    baseline_commit: BASELINE_COMMIT,
    baseline_tree: BASELINE_TREE,
    required_ids: REQUIRED_IDS,
    bindings: BINDINGS,
};

/// Build the deterministic Stage 3 contract crosswalk bytes.
pub fn generate_stage3_crosswalk(repo_root: &Path) -> Result<Vec<u8>, CrosswalkError> {
    CROSSWALK.generate(repo_root)
}

/// Write the deterministic Stage 3 contract crosswalk to its checked-in path.
pub fn write_stage3_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    CROSSWALK.write(repo_root)
}

/// Check that the checked-in Stage 3 crosswalk equals a fresh deterministic generation.
pub fn check_stage3_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    CROSSWALK.check(repo_root)
}

#[cfg(test)]
#[path = "stage3_crosswalk_tests.rs"]
mod tests;
