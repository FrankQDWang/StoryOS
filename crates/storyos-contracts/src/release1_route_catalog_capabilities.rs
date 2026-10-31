//! The Release 1 capability coverage of the route catalog.

use serde_json::{Value, json};

use crate::release1::REQUIRED_CAPABILITIES;

/// One Release 1 journey and the served operations that it needs.
struct Capability {
    id: &'static str,
    journey: &'static str,
    required_operations: &'static [&'static str],
    operation_actions: &'static [(&'static str, &'static [&'static str])],
}

const CAPABILITIES: &[Capability] = &[
    Capability {
        id: "project_lifecycle",
        journey: "Project lifecycle",
        required_operations: &[
            "createProject",
            "listProjects",
            "getProject",
            "updateProject",
            "getProjectAssistance",
            "updateProjectAssistance",
        ],
        operation_actions: &[],
    },
    Capability {
        id: "volume_chapter_lifecycle",
        journey: "Volume/chapter create, rename, reorder, and remove",
        required_operations: &[
            "createVolume",
            "updateVolume",
            "deleteVolume",
            "createChapter",
            "updateChapter",
            "deleteChapter",
        ],
        operation_actions: &[
            ("createVolume", &["create"]),
            ("updateVolume", &["rename", "reorder"]),
            ("deleteVolume", &["remove"]),
            ("createChapter", &["create"]),
            ("updateChapter", &["rename", "reorder"]),
            ("deleteChapter", &["remove"]),
        ],
    },
    Capability {
        id: "chapter_navigation",
        journey: "Chapter navigation and manuscript tree",
        required_operations: &["getManuscriptTree", "getChapter", "setCurrentChapter"],
        operation_actions: &[],
    },
    Capability {
        id: "manuscript_search",
        journey: "Bounded manuscript search",
        required_operations: &["searchManuscript"],
        operation_actions: &[],
    },
    Capability {
        id: "direct_author_edit",
        journey: "Direct author editing through the single ownership-classifying ingress",
        required_operations: &["applyAuthorEdit"],
        operation_actions: &[("applyAuthorEdit", &["direct_author_edit"])],
    },
    Capability {
        id: "acknowledgement_loss_outcome_query",
        journey: "Read one exact Apply Author Edit outcome after a missing acknowledgement",
        required_operations: &["getApplyAuthorEditOutcome"],
        operation_actions: &[],
    },
    Capability {
        id: "single_match_replace",
        journey: "Single current visible-match replacement",
        required_operations: &["applyAuthorEdit"],
        operation_actions: &[("applyAuthorEdit", &["single_current_visible_match_replace"])],
    },
    Capability {
        id: "multi_match_cross_location_proposal",
        journey: "Selected multi-match or cross-location replacement as a Core Proposal",
        required_operations: &["applyAuthorEdit"],
        operation_actions: &[(
            "applyAuthorEdit",
            &["selected_multi_match_replace", "cross_location_replace"],
        )],
    },
    Capability {
        id: "manuscript_statistics",
        journey: "Manuscript and chapter statistics",
        required_operations: &["getStatistics"],
        operation_actions: &[],
    },
    Capability {
        id: "human_readable_manuscript_export",
        journey: "Human-readable manuscript export in deterministic volume/chapter order",
        required_operations: &[
            "exportHumanReadableManuscript",
            "getHumanReadableManuscriptExport",
        ],
        operation_actions: &[],
    },
    Capability {
        id: "portable_project_archive",
        journey: "Versioned portable Project Export Archive import/export",
        required_operations: &["exportProjectArchive", "getExportOperation"],
        operation_actions: &[],
    },
    Capability {
        id: "editor_session_takeover",
        journey: "Editor Session and writer-generation takeover",
        required_operations: &[
            "createEditorSession",
            "getEditorSession",
            "takeOverProjectWriter",
        ],
        operation_actions: &[],
    },
    Capability {
        id: "snapshot_resync",
        journey: "Snapshot and replay-generation resynchronization",
        required_operations: &["getSnapshot", "activityStream"],
        operation_actions: &[],
    },
    Capability {
        id: "proposal_draft_lifecycle",
        journey: "Proposal and Draft lifecycle",
        required_operations: &[
            "applyAuthorEdit",
            "getProposal",
            "listChapterOpenProposals",
            "acceptProposal",
            "rejectProposalOperations",
            "withdrawProposal",
            "replanProposal",
            "reopenWithdrawnProposal",
            "reopenRejectedOperations",
            "completeReadyPartialProposal",
            "continueProposalGeneration",
            "undoLatestAuthorAction",
        ],
        operation_actions: &[],
    },
    Capability {
        id: "project_activity",
        journey: "Project Activity observation",
        required_operations: &["activityStream", "getSnapshot"],
        operation_actions: &[],
    },
];

/// The required capabilities, each with the served operations of its journey.
pub(super) fn capability_coverage() -> Value {
    json!({
        "required_capability_ids": REQUIRED_CAPABILITIES,
        "capabilities": CAPABILITIES.iter().map(capability_json).collect::<Vec<_>>(),
    })
}

fn capability_json(capability: &Capability) -> Value {
    let mut entry = json!({
        "capability_id": capability.id,
        "journey": capability.journey,
        "required_operations": capability.required_operations,
    });
    if !capability.operation_actions.is_empty() {
        entry["operation_actions"] = capability
            .operation_actions
            .iter()
            .map(|(operation_id, actions)| ((*operation_id).to_owned(), json!(actions)))
            .collect();
    }
    entry
}
