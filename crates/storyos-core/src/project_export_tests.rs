use super::*;
use crate::ProjectPresence;

#[test]
fn absent_project_is_refused() {
    assert_eq!(
        export_project_archive(&ExportProjectArchive {
            presence: ProjectPresence::Absent,
        }),
        ExportProjectArchiveResult::Refused {
            reason: ExportProjectArchiveRefusal::MissingProject,
        }
    );
}

#[test]
fn present_project_is_admitted() {
    assert_eq!(
        export_project_archive(&ExportProjectArchive {
            presence: ProjectPresence::Present,
        }),
        ExportProjectArchiveResult::Admitted
    );
}
