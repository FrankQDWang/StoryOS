use crate::CommandKind;

pub fn project_command_kind(
    method: &str,
    route_template: &str,
    command_schema: &str,
) -> Option<CommandKind> {
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
    pub(super) command_kind: CommandKind,
    pub(super) method: &'static str,
    pub(super) route_template: &'static str,
    pub(super) command_schema: &'static str,
}

pub(super) const PROJECT_COMMAND_TARGETS: &[ProjectCommandTarget] = &[
    target(
        CommandKind::UpdateProject,
        "PATCH",
        "/api/v1/projects/{project_id}",
        "storyos.command.update-project.request.v1",
    ),
    target(
        CommandKind::UpdateProjectAssistance,
        "PUT",
        "/api/v1/projects/{project_id}/assistance",
        "storyos.command.update-project-assistance.request.v1",
    ),
    target(
        CommandKind::CreateEditorSession,
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions",
        "storyos.command.create-editor-session.request.v1",
    ),
    target(
        CommandKind::TakeOverProjectWriter,
        "POST",
        "/api/v1/projects/{project_id}/editor-sessions/{editor_session_id}/takeovers",
        "storyos.command.take-over-project-writer.request.v1",
    ),
    target(
        CommandKind::CreateVolume,
        "POST",
        "/api/v1/projects/{project_id}/volumes",
        "storyos.command.create-volume.request.v1",
    ),
    target(
        CommandKind::UpdateVolume,
        "PATCH",
        "/api/v1/projects/{project_id}/volumes/{volume_id}",
        "storyos.command.update-volume.request.v1",
    ),
    target(
        CommandKind::DeleteVolume,
        "DELETE",
        "/api/v1/projects/{project_id}/volumes/{volume_id}",
        "storyos.command.delete-volume.request.v1",
    ),
    target(
        CommandKind::CreateChapter,
        "POST",
        "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters",
        "storyos.command.create-chapter.request.v1",
    ),
    target(
        CommandKind::UpdateChapter,
        "PATCH",
        "/api/v1/projects/{project_id}/chapters/{chapter_id}",
        "storyos.command.update-chapter.request.v1",
    ),
    target(
        CommandKind::SetCurrentChapter,
        "PUT",
        "/api/v1/projects/{project_id}/current-chapter",
        "storyos.command.set-current-chapter.request.v1",
    ),
    target(
        CommandKind::DeleteChapter,
        "DELETE",
        "/api/v1/projects/{project_id}/chapters/{chapter_id}",
        "storyos.command.delete-chapter.request.v1",
    ),
    target(
        CommandKind::ApplyAuthorEdit,
        "POST",
        "/api/v1/projects/{project_id}/manuscript/author-edits",
        "storyos.command.apply-author-edit.request.v1",
    ),
    target(
        CommandKind::AcceptProposal,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/acceptances",
        "storyos.command.accept-proposal.request.v1",
    ),
    target(
        CommandKind::RejectProposalOperations,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/rejections",
        "storyos.command.reject-proposal-operations.request.v1",
    ),
    target(
        CommandKind::WithdrawProposal,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/withdrawals",
        "storyos.command.withdraw-proposal.request.v1",
    ),
    target(
        CommandKind::ReplanProposal,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/replans",
        "storyos.command.replan-proposal.request.v1",
    ),
    target(
        CommandKind::ReopenWithdrawnProposal,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/reopenings",
        "storyos.command.reopen-withdrawn-proposal.request.v1",
    ),
    target(
        CommandKind::ReopenRejectedOperations,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/operation-reopenings",
        "storyos.command.reopen-rejected-operations.request.v1",
    ),
    target(
        CommandKind::CompleteReadyPartialProposal,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-completions",
        "storyos.command.complete-ready-partial-proposal.request.v1",
    ),
    target(
        CommandKind::ContinueProposalGeneration,
        "POST",
        "/api/v1/projects/{project_id}/proposals/{proposal_id}/generation-continuations",
        "storyos.command.continue-proposal-generation.request.v1",
    ),
    target(
        CommandKind::UndoLatestAuthorAction,
        "POST",
        "/api/v1/projects/{project_id}/author-actions/undo",
        "storyos.command.undo-latest-author-action.request.v1",
    ),
    target(
        CommandKind::CreateAgentRun,
        "POST",
        "/api/v1/projects/{project_id}/agent-runs",
        "storyos.command.create-agent-run.request.v2",
    ),
    target(
        CommandKind::SteerAgentRun,
        "POST",
        "/api/v1/projects/{project_id}/agent-runs/{run_id}/steering-inputs",
        "storyos.command.steer-agent-run.request.v1",
    ),
    target(
        CommandKind::PauseAgentRun,
        "POST",
        "/api/v1/projects/{project_id}/agent-runs/{run_id}/pause",
        "storyos.command.pause-agent-run.request.v1",
    ),
    target(
        CommandKind::CancelAgentRun,
        "POST",
        "/api/v1/projects/{project_id}/agent-runs/{run_id}/cancel",
        "storyos.command.cancel-agent-run.request.v1",
    ),
    target(
        CommandKind::ExportProjectArchive,
        "POST",
        "/api/v1/projects/{project_id}/exports",
        "storyos.command.export-project-archive.request.v1",
    ),
    target(
        CommandKind::ExportHumanReadableManuscript,
        "POST",
        "/api/v1/projects/{project_id}/manuscript/exports",
        "storyos.command.export-human-readable-manuscript.request.v1",
    ),
];

const fn target(
    command_kind: CommandKind,
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
