use super::*;
use crate::ProjectLifecycle;
use crate::ProjectPresence;

#[test]
fn archived_project_is_refused() {
    assert_eq!(
        export_project_archive(&ExportProjectArchive {
            presence: ProjectPresence::Present,
            current_lifecycle: ProjectLifecycle::Archived,
        }),
        ExportProjectArchiveResult::Refused {
            reason: ExportProjectArchiveRefusal::ArchivedProject,
        }
    );
}

#[test]
fn active_project_is_admitted() {
    assert_eq!(
        export_project_archive(&ExportProjectArchive {
            presence: ProjectPresence::Present,
            current_lifecycle: ProjectLifecycle::Active,
        }),
        ExportProjectArchiveResult::Admitted
    );
}
