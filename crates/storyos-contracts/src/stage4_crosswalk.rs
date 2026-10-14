use std::path::Path;

use crate::stage_crosswalk::{Binding, EvidenceClass, StageCrosswalk};
use crate::stage1_crosswalk::CrosswalkError;

/// Tracked deterministic output that this module owns.
pub const GENERATED_STAGE4_CROSSWALK_PATH: &str =
    "generated/evidence/stage4-contract-crosswalk.json";

const SCHEMA_ID: &str = "storyos.evidence.stage4-contract-crosswalk.v1";
const EVALUATED_STAGE: &str = "Stage 4";
const MANDATORY_MAP: &str = "SMAP-STAGE-4";
const CONTRACT_REVISION: &str = "stage-responses-context-memory-alignment-2026-09-15-v1";
const PARENT_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/362";
const TICKET_ISSUE: &str = "https://github.com/FrankQDWang/StoryOS/issues/396";
const BASELINE_COMMIT: &str = "f03cebc0d4aa9baf0be131f686234e4907a5580c";
const BASELINE_TREE: &str = "65aa039bc27a133e5d70fab4c407c5d17b2b8b86";
const REQUIRED_IDS: &[&str] = &[
    "S4-REQ-001",
    "S4-REQ-002",
    "S4-REQ-003",
    "S4-REQ-004",
    "S4-REQ-005",
    "S4-REQ-006",
    "S4-REQ-007",
    "S4-EVD-001",
    "S4-EVD-002",
    "S4-EVD-003",
    "S4-EVD-004",
    "S4-EVD-005",
    "S4-EVD-006",
    "S4-EVD-007",
    "S4-JRN-001",
    "S4-JRN-001/1",
    "S4-JRN-001/2",
    "S4-JRN-001/3",
    "S4-JRN-001/4",
    "S4-JRN-001/5",
    "S4-JRN-001/6",
    "S4-JRN-001/7",
];
const PROOF: &str = "docs/foundation/product-delivery-proof-selection.md";
const ROUTE_DECISION: &str =
    "docs/adr/0033-use-volcengine-responses-for-the-first-real-model-path.md";
const JOURNEY_RECORD: &str = "docs/research/stage4-real-author-journey-2026-10-14.md";
const RUNTIME_RECORD: &str = "docs/research/agent-plan-runtime-qualification-2026-10-11.md";
const PREFLIGHT_RECORD: &str = "docs/research/agent-plan-responses-capability-preflight.md";
const CONTINUATION_RECORD: &str =
    "docs/research/agent-plan-continuation-qualification-2026-10-12.md";
const STREAMING_RECORD: &str = "docs/research/agent-plan-streaming-qualification-2026-10-12.md";
const COMPACTION_RECORD: &str = "docs/research/agent-plan-compaction-qualification-2026-10-12.md";
const THRESHOLD_RECORD: &str =
    "docs/research/agent-plan-compaction-threshold-qualification-2026-10-13.md";
const INPUT_LIMIT_RECORD: &str = "docs/research/agent-plan-input-limit-measurement-2026-10-12.md";
const RECOVERY_RECORD: &str = "docs/research/agent-plan-recovery-qualification-2026-10-13.md";
const OVER_LONG_RECORD: &str = "docs/research/agent-plan-over-long-input-rejection-2026-10-14.md";
const PINNED_RECORD: &str = "docs/research/agent-plan-pinned-version-qualification-2026-10-14.md";
const GRANT_RECORD: &str = "docs/research/standing-grant-scope-live-request-2026-10-12.md";
const CORE_REGISTRATION: &str = "crates/storyos-core/src/model_registration_tests.rs";
const CORE_GRANT: &str = "crates/storyos-core/src/grant_scope_tests.rs";
const CORE_SUCCESSOR: &str = "crates/storyos-core/src/unknown_create_successor_tests.rs";
const CORE_COMPACTION: &str = "crates/storyos-core/src/compact_active_context_tests.rs";
const CORE_ASSEMBLY: &str = "crates/storyos-core/src/assemble_context_tests.rs";
const CORE_ADMISSION: &str = "crates/storyos-core/src/create_admission_tests.rs";
const PG_REGISTRATION: &str = "crates/storyos-adapter-postgres/src/model_registration_tests.rs";
const PG_DESTINATION: &str =
    "crates/storyos-adapter-postgres/src/project_destination_binding_tests.rs";
const PG_QUALIFICATION: &str = "crates/storyos-adapter-postgres/src/runtime_qualification_tests.rs";
const PG_PROVIDER_EVIDENCE: &str = "crates/storyos-adapter-postgres/src/provider_evidence_tests.rs";
const PG_GRANT: &str = "crates/storyos-adapter-postgres/src/grant_scope_tests.rs";
const PG_RETRIEVAL: &str = "crates/storyos-adapter-postgres/src/agent_run_retrieval_tests.rs";
const PG_ABORT: &str = "crates/storyos-adapter-postgres/src/agent_run_abort_tests.rs";
const PG_SUCCESSOR: &str = "crates/storyos-adapter-postgres/src/agent_run_successor_route_tests.rs";
const PG_SUMMARY: &str =
    "crates/storyos-adapter-postgres/src/agent_run_compaction_summary_tests.rs";
const PG_LEASE: &str = "crates/storyos-adapter-postgres/src/agent_run_lease_tests.rs";
const PG_CONTENTION: &str = "crates/storyos-adapter-postgres/src/stream_commit_contention_tests.rs";
const PG_CREATE: &str = "crates/storyos-adapter-postgres/src/agent_run_create_dispatch_tests.rs";
const PG_GATEWAY: &str = "crates/storyos-adapter-postgres/src/model_gateway_tests.rs";
const LEASE_RENEWAL: &str = "crates/storyos-application/src/model_lease_renewal_tests.rs";
const STREAM_COMMIT: &str = "crates/storyos-application/src/model_stream_commit_tests.rs";
const ROUTE_EXCHANGE: &str = "crates/storyos-adapter-volcengine-responses/src/exchange_tests.rs";
const ROUTE_STREAM: &str = "crates/storyos-adapter-volcengine-responses/src/stream_tests.rs";
const SETTINGS: &str = "crates/storyos-settings/src/settings_tests.rs";
const WORKER_ADAPTERS: &str = "crates/storyos-worker-bin/src/registered_adapters_tests.rs";
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
const ACCEPT: &str = "apps/web/test/node-postgresql/accept-proposal-http.integration.test.ts";
const REJECT: &str =
    "apps/web/test/node-postgresql/reject-proposal-operations-http.integration.test.ts";
const CANDIDATE_EDIT: &str =
    "apps/web/test/node-postgresql/edit-proposal-candidate-http.integration.test.ts";
const UNDO: &str = "apps/web/test/node-postgresql/undo-acceptance-http.integration.test.ts";
const ACCEPT_REFUSAL: &str =
    "apps/web/test/node-postgresql/acceptance-refusal-http.integration.test.ts";
const ASSISTANCE: &str =
    "apps/web/test/node-postgresql/update-project-assistance-http.integration.test.ts";
const ARCHIVE: &str = "apps/web/test/node-postgresql/archive-project-http.integration.test.ts";
const DISCLOSURE_REFUSAL: &str =
    "apps/web/test/browser-source/assistant-disclosure-refusal.integration.test.ts";
const STOPPED_REQUEST: &str =
    "apps/web/test/browser-source/assistant-stopped-request.integration.test.ts";
const RUN_REFRESH: &str = "apps/web/test/browser-source/assistant-run-refresh.integration.test.ts";
const PRODUCTION_HOST: &str =
    "apps/web/test/browser-exact-dist/production-host.integration.test.ts";
const STAGE2_JOURNEY: &str = "apps/web/test/browser-exact-dist/s2-jrn-001.integration.test.ts";
const WORKSPACE: &str = "apps/web/test/browser-exact-dist/s2-workspace.integration.test.ts";
const PHYSICAL_DRILL: &str =
    "apps/web/test/browser-exact-dist/s2-physical-drill.integration.test.ts";
const INLINE_PRODUCTION: &str =
    "apps/web/test/browser-exact-dist/inline-proposal.integration.test.ts";
const RUN_EVIDENCE: &str =
    "apps/web/test/browser-exact-dist/production-run-evidence.integration.test.ts";
const PHYSICAL_SCRIPT: &str = "scripts/verify-recovery-hold.sh";

#[path = "stage4_crosswalk_bindings.rs"]
mod bindings;

use bindings::BINDINGS;

const CROSSWALK: StageCrosswalk = StageCrosswalk {
    generated_path: GENERATED_STAGE4_CROSSWALK_PATH,
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

/// Build the deterministic Stage 4 contract crosswalk bytes.
pub fn generate_stage4_crosswalk(repo_root: &Path) -> Result<Vec<u8>, CrosswalkError> {
    CROSSWALK.generate(repo_root)
}

/// Write the deterministic Stage 4 contract crosswalk to its tracked path.
pub fn write_stage4_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    CROSSWALK.write(repo_root)
}

/// Examine whether the tracked Stage 4 crosswalk is equal to a fresh deterministic generation.
pub fn check_stage4_crosswalk(repo_root: &Path) -> Result<(), CrosswalkError> {
    CROSSWALK.check(repo_root)
}

#[cfg(test)]
#[path = "stage4_crosswalk_tests.rs"]
mod tests;
