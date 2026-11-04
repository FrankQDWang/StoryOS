//! The command kinds of the Route Catalog: one for each command operation of the registry.

macro_rules! command_kinds {
    ($( $variant:ident => $kind:literal, )*) => {
        /// Names one command operation of the Route Catalog.
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum CommandKind {
            $( $variant, )*
        }

        impl CommandKind {
            /// Lists every command kind in the order of the operation registry.
            pub const ALL: &'static [Self] = &[ $( Self::$variant, )* ];

            /// Returns the operation ID of the Route Catalog, which is also the stored command kind.
            pub const fn as_str(self) -> &'static str {
                match self { $( Self::$variant => $kind, )* }
            }

            /// Resolves a stored command kind, or refuses an unknown one.
            pub fn parse(kind: &str) -> Option<Self> {
                match kind {
                    $( $kind => Some(Self::$variant), )*
                    _ => None,
                }
            }
        }
    };
}

command_kinds! {
    CreateProject => "createProject",
    UpdateProject => "updateProject",
    UpdateProjectAssistance => "updateProjectAssistance",
    SetProjectToolEnablement => "setProjectToolEnablement",
    CreateEditorSession => "createEditorSession",
    TakeOverProjectWriter => "takeOverProjectWriter",
    CreateVolume => "createVolume",
    UpdateVolume => "updateVolume",
    DeleteVolume => "deleteVolume",
    CreateChapter => "createChapter",
    UpdateChapter => "updateChapter",
    DeleteChapter => "deleteChapter",
    SetCurrentChapter => "setCurrentChapter",
    ApplyAuthorEdit => "applyAuthorEdit",
    AcceptProposal => "acceptProposal",
    RejectProposalOperations => "rejectProposalOperations",
    WithdrawProposal => "withdrawProposal",
    ReplanProposal => "replanProposal",
    ReopenWithdrawnProposal => "reopenWithdrawnProposal",
    ReopenRejectedOperations => "reopenRejectedOperations",
    CompleteReadyPartialProposal => "completeReadyPartialProposal",
    ContinueProposalGeneration => "continueProposalGeneration",
    UndoLatestAuthorAction => "undoLatestAuthorAction",
    CreateAgentRun => "createAgentRun",
    SteerAgentRun => "steerAgentRun",
    PauseAgentRun => "pauseAgentRun",
    CancelAgentRun => "cancelAgentRun",
    ExportProjectArchive => "exportProjectArchive",
    ExportHumanReadableManuscript => "exportHumanReadableManuscript",
}

#[cfg(test)]
#[path = "command_kind_tests.rs"]
mod tests;
