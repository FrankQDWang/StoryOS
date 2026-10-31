use super::*;
use crate::ProjectPresence;

#[test]
fn absent_project_is_refused() {
    assert_eq!(
        export_human_readable_manuscript(&ExportHumanReadableManuscript {
            presence: ProjectPresence::Absent,
        }),
        ExportHumanReadableManuscriptResult::Refused {
            reason: ExportHumanReadableManuscriptRefusal::MissingProject,
        }
    );
}

#[test]
fn present_project_is_admitted() {
    assert_eq!(
        export_human_readable_manuscript(&ExportHumanReadableManuscript {
            presence: ProjectPresence::Present,
        }),
        ExportHumanReadableManuscriptResult::Admitted
    );
}
