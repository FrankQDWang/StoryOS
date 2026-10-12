use super::{GrantFacts, GrantScope, admit_disclosure};
use crate::assemble_context::{ProjectionMode, SelectedProjection};
use crate::{
    ContextSourceClass, CurrentPassageAssembly, CurrentPassageAssemblyRecord,
    InstructionBindingInput, PassageContextTarget, assemble_current_passage_context,
    assemble_passage_collection, decode_assembly_record, encode_assembly_record,
};

fn passage(chapter_id: &str, body: &str) -> CurrentPassageAssembly {
    CurrentPassageAssembly {
        operation_requirement_id: "req-1".to_owned(),
        input_snapshot_id: "snap-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        author_message: "Help with this passage.".to_owned(),
        chapter_id: chapter_id.to_owned(),
        chapter_revision_id: Some(format!("{chapter_id}-rev")),
        proposal_target_block_ids: Some(vec![format!("{chapter_id}-block")]),
        chapter_body: body.to_owned(),
        instruction: InstructionBindingInput::Absent,
        destination_identity: "dest-1".to_owned(),
    }
}

fn facts(request: &CurrentPassageAssemblyRecord) -> GrantFacts<'_> {
    GrantFacts {
        owner_user_id: "user-1",
        project_id: "proj-1",
        grant_scope: Some("project_writing_assistance_v1"),
        grant_identity: Some("dest-1"),
        binding_identity: Some("dest-1"),
        request: Some(request),
    }
}

#[test]
fn the_scope_covers_the_current_chapter_and_a_collection_of_other_chapters() {
    let current = assemble_current_passage_context(&passage("ch-1", "The tide turned."));
    let collection = assemble_passage_collection(
        &passage("ch-1", "The tide turned."),
        ["ch-1", "ch-2"]
            .map(|chapter_id| PassageContextTarget {
                chapter_id: chapter_id.to_owned(),
                base_authoritative_revision_id: format!("{chapter_id}-rev"),
                manuscript_block_ids: vec![format!("{chapter_id}-block")],
            })
            .to_vec(),
        &[
            passage("ch-1", "The tide turned."),
            passage("ch-2", "A lantern crossed the river."),
        ],
    );

    assert_eq!(
        [
            admit_disclosure(&facts(&current)),
            admit_disclosure(&facts(&collection))
        ],
        [Ok(GrantScope::ProjectWritingAssistanceV1); 2]
    );
}

#[test]
fn each_request_outside_the_scope_refuses() {
    let request = assemble_current_passage_context(&passage("ch-1", "The tide turned."));
    let mut host_control = request.clone();
    host_control.selected.push(SelectedProjection {
        source_class: ContextSourceClass::HostControl,
        source_version: "req-1".to_owned(),
        projection_mode: ProjectionMode::ExactRequired,
        token_count: 4,
        content: "host".to_owned(),
    });
    let mut other_destination = request.clone();
    other_destination.operation_requirement.destination_identity = "dest-2".to_owned();
    let base = facts(&request);

    assert_eq!(
        [
            GrantFacts {
                grant_scope: None,
                ..base
            },
            GrantFacts {
                grant_scope: Some("project_writing_assistance_v0"),
                ..base
            },
            GrantFacts {
                project_id: "proj-2",
                ..base
            },
            GrantFacts {
                owner_user_id: "user-2",
                ..base
            },
            facts(&other_destination),
            GrantFacts {
                binding_identity: Some("dest-2"),
                ..base
            },
            GrantFacts {
                grant_identity: None,
                ..base
            },
            facts(&host_control),
            GrantFacts {
                request: None,
                ..base
            },
        ]
        .map(|facts| admit_disclosure(&facts)),
        [Err("disclosure_outside_grant"); 9]
    );
}

#[test]
fn a_stored_purpose_that_the_scope_does_not_name_decodes_to_no_request() {
    let request = assemble_current_passage_context(&passage("ch-1", "The tide turned."));
    let mut stored = encode_assembly_record(&request);
    let current = decode_assembly_record(&stored);
    stored["operation_requirement"]["purpose"] = serde_json::json!("research");

    assert_eq!(current.as_ref(), Some(&request));
    assert_eq!(decode_assembly_record(&stored), None);
    assert_eq!(
        admit_disclosure(&GrantFacts {
            request: None,
            ..facts(&request)
        }),
        Err("disclosure_outside_grant")
    );
}
