pub fn project_command_kind(
    method: &str,
    route_template: &str,
    command_schema: &str,
) -> Option<&'static str> {
    PROJECT_COMMAND_TARGETS
        .iter()
        .find(|target| {
            target.method == method
                && target.route_template == route_template
                && target.command_schema == command_schema
        })
        .map(|target| target.command_kind)
}

pub(super) struct ProjectCommandTarget {
    pub(super) command_kind: &'static str,
    pub(super) method: &'static str,
    pub(super) route_template: &'static str,
    pub(super) command_schema: &'static str,
}

pub(super) const PROJECT_COMMAND_TARGETS: &[ProjectCommandTarget] = &[
    target(
        "updateProject",
        "PATCH",
        "/api/v1/projects/{project_id}",
        "storyos.command.update-project.request.v1",
    ),
    target(
        "archiveProject",
        "PUT",
        "/api/v1/projects/{project_id}/archival",
        "storyos.command.archive-project.request.v1",
    ),
    target(
        "deleteProject",
        "DELETE",
        "/api/v1/projects/{project_id}",
        "storyos.command.delete-project.request.v1",
    ),
    target(
        "createEditorSession",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
    ),
    target(
        "takeOverProjectWriter",
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers",
        "storyos.command.take-over-project-writer.request.v1",
    ),
    target(
        "createVolume",
        "POST",
        "/api/v1/projects/{project_id}/volumes",
        "storyos.command.create-volume.request.v1",
    ),
    target(
        "updateVolume",
        "PATCH",
        "/api/v1/projects/{project_id}/volumes/{volume_id}",
        "storyos.command.update-volume.request.v1",
    ),
    target(
        "deleteVolume",
        "DELETE",
        "/api/v1/projects/{project_id}/volumes/{volume_id}",
        "storyos.command.delete-volume.request.v1",
    ),
    target(
        "createChapter",
        "POST",
        "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
        "storyos.command.create-chapter.request.v1",
    ),
    target(
        "updateChapter",
        "PATCH",
        "/api/v1/projects/{project_id}/chapters/{chapter_id}",
        "storyos.command.update-chapter.request.v1",
    ),
    target(
        "setCurrentChapter",
        "PUT",
        "/api/v1/projects/{project_id}/current-chapter",
        "storyos.command.set-current-chapter.request.v1",
    ),
    target(
        "deleteChapter",
        "DELETE",
        "/api/v1/projects/{project_id}/chapters/{chapter_id}",
        "storyos.command.delete-chapter.request.v1",
    ),
    target(
        "applyAuthorEdit",
        "POST",
        "/api/v1/projects/{project_id}/manuscript/author-edits",
        "storyos.command.apply-author-edit.request.v1",
    ),
    target(
        "createReplacementProposal",
        "POST",
        "/api/v1/projects/{project_id}/manuscript/replacement-proposals",
        "storyos.command.create-replacement-proposal.request.v1",
    ),
    target(
        "acceptProposal",
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
        "storyos.command.accept-proposal.request.v1",
    ),
    target(
        "rejectProposalOperations",
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
        "storyos.command.reject-proposal-operations.request.v1",
    ),
    target(
        "withdrawProposal",
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
        "storyos.command.withdraw-proposal.request.v1",
    ),
    target(
        "replanProposal",
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans",
        "storyos.command.replan-proposal.request.v1",
    ),
    target(
        "reopenWithdrawnProposal",
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/reopenings",
        "storyos.command.reopen-withdrawn-proposal.request.v1",
    ),
    target(
        "supersedeProposal",
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/supersessions",
        "storyos.command.supersede-proposal.request.v1",
    ),
    target(
        "reopenRejectedOperations",
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/operation-reopenings",
        "storyos.command.reopen-rejected-operations.request.v1",
    ),
    target(
        "completeReadyPartialProposal",
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-completions",
        "storyos.command.complete-ready-partial-proposal.request.v1",
    ),
    target(
        "continueProposalGeneration",
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-continuations",
        "storyos.command.continue-proposal-generation.request.v1",
    ),
    target(
        "expandRefusedEditDraftToProposal",
        "POST",
        "/api/v1/projects/{project_id}/drafts/{draft_id}/proposal-expansions",
        "storyos.command.expand-refused-edit-draft-to-proposal.request.v1",
    ),
    target(
        "closeEditorFlowDraft",
        "POST",
        "/api/v1/projects/{project_id}/drafts/{draft_id}/closures",
        "storyos.command.close-editor-flow-draft.request.v1",
    ),
    target(
        "undoLatestAuthorAction",
        "POST",
        "/api/v1/projects/{project_id}/author-actions/undo",
        "storyos.command.undo-latest-author-action.request.v1",
    ),
    target(
        "createAgentRun",
        "POST",
        "/api/v1/projects/{project_id}/agent-runs",
        "storyos.command.create-agent-run.request.v1",
    ),
    target(
        "pauseAgentRun",
        "POST",
        "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause",
        "storyos.command.pause-agent-run.request.v1",
    ),
    target(
        "resumeAgentRun",
        "POST",
        "/api/v1/projects/{project_id}/agent-runs/{run_id}/resume",
        "storyos.command.resume-agent-run.request.v1",
    ),
    target(
        "cancelAgentRun",
        "POST",
        "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
        "storyos.command.cancel-agent-run.request.v1",
    ),
    target(
        "resolveWait",
        "POST",
        "/api/v1/projects/{project_id}/waits/{wait_id}/resolutions",
        "storyos.command.resolve-wait.request.v1",
    ),
    target(
        "decideApproval",
        "POST",
        "/api/v1/projects/{project_id}/approvals/{approval_id}/decisions",
        "storyos.command.decide-approval.request.v1",
    ),
    target(
        "updateContextControls",
        "POST",
        "/api/v1/projects/{project_id}/context-controls",
        "storyos.command.update-context-controls.request.v1",
    ),
    target(
        "importProjectArchive",
        "POST",
        "/api/v1/projects/{project_id}/imports",
        "storyos.command.import-project-archive.request.v1",
    ),
    target(
        "exportProjectArchive",
        "POST",
        "/api/v1/projects/{project_id}/exports",
        "storyos.command.export-project-archive.request.v1",
    ),
    target(
        "exportHumanReadableManuscript",
        "POST",
        "/api/v1/projects/{project_id}/manuscript/exports",
        "storyos.command.export-human-readable-manuscript.request.v1",
    ),
];

const fn target(
    command_kind: &'static str,
    method: &'static str,
    route_template: &'static str,
    command_schema: &'static str,
) -> ProjectCommandTarget {
    ProjectCommandTarget {
        command_kind,
        method,
        route_template,
        command_schema,
    }
}
