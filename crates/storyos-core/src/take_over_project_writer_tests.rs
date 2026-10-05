use super::{
    CurrentWriter, ProjectLifecycle, StaleWriterObservation, TakeOverProjectWriter,
    TakeOverProjectWriterNoEffect, TransitionOutcome, take_over_project_writer,
};

fn takeover() -> TakeOverProjectWriter {
    TakeOverProjectWriter {
        observed_writer_generation: 3,
        current_writer: Some(CurrentWriter {
            writer_generation: 3,
            is_requesting_session: false,
        }),
        lifecycle: ProjectLifecycle::Active,
        current_chapter_has_head: true,
    }
}

#[test]
fn an_observed_current_writer_of_another_session_is_taken_over() {
    assert_eq!(
        take_over_project_writer(&takeover()),
        Ok(TransitionOutcome::NoEffect(
            TakeOverProjectWriterNoEffect::WriterTakeoverApplied
        ))
    );
}

#[test]
fn every_stale_or_unusable_observation_is_refused_before_admission() {
    let mut changed = takeover();
    changed.observed_writer_generation = 2;
    let mut already_writer = takeover();
    already_writer.current_writer = Some(CurrentWriter {
        writer_generation: 3,
        is_requesting_session: true,
    });
    let mut no_writer = takeover();
    no_writer.current_writer = None;
    let mut archived = takeover();
    archived.lifecycle = ProjectLifecycle::Archived;
    let mut no_chapter = takeover();
    no_chapter.current_chapter_has_head = false;
    assert_eq!(
        [changed, already_writer, no_writer, archived, no_chapter]
            .iter()
            .map(take_over_project_writer)
            .collect::<Vec<_>>(),
        vec![Err(StaleWriterObservation); 5]
    );
}
