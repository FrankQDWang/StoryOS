use super::*;
use crate::ProjectLifecycle;
use crate::ProjectPresence;

#[test]
fn archived_project_is_refused() {
    assert_eq!(
        export_human_readable_manuscript(&ExportHumanReadableManuscript {
            presence: ProjectPresence::Present,
            current_lifecycle: ProjectLifecycle::Archived,
        }),
        ExportHumanReadableManuscriptResult::Refused {
            reason: ExportHumanReadableManuscriptRefusal::ArchivedProject,
        }
    );
}

#[test]
fn active_project_is_admitted() {
    assert_eq!(
        export_human_readable_manuscript(&ExportHumanReadableManuscript {
            presence: ProjectPresence::Present,
            current_lifecycle: ProjectLifecycle::Active,
        }),
        ExportHumanReadableManuscriptResult::Admitted
    );
}
