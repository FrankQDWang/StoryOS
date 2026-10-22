use super::*;
use storyos_application::{DeclaredTarget, DestinationRequest, RequestAttempt, RequestContextItem};
use storyos_core::{
    AdmittedSelection, CompactionRequestFacts, CompactionTrigger, DeclaredTargetKind, TextPoint,
};

/// A Chapter text reference in `chapter` and a reference in the pending candidate of
/// `pending_candidate`.
fn references(chapter: &DeclaredTarget) -> Vec<AdmittedSelection> {
    vec![
        AdmittedSelection {
            reference: SelectionReference::ChapterText {
                chapter_id: chapter.chapter_id.clone(),
                base_authoritative_revision_id: chapter.base_revision_id.clone(),
                start: TextPoint {
                    manuscript_block_id: chapter.block_id.clone(),
                    offset: 4,
                },
                end: TextPoint {
                    manuscript_block_id: chapter.block_id.clone(),
                    offset: 13,
                },
            },
            selected_text: Some("rain fell".to_owned()),
            manuscript_block_ids: vec![chapter.block_id.clone()],
            candidate_text: None,
        },
        AdmittedSelection {
            reference: SelectionReference::ProposalCandidateText {
                chapter_id: chapter.chapter_id.clone(),
                proposal_id: "proposal-1".to_owned(),
                operation_id: "operation-1".to_owned(),
                revision_id: "revision-9".to_owned(),
                start_offset: 4,
                end_offset: 8,
            },
            selected_text: Some("wind".to_owned()),
            manuscript_block_ids: vec!["block-2".to_owned()],
            candidate_text: Some("The wind rose.".to_owned()),
        },
    ]
}

fn pending_candidate(chapter: &DeclaredTarget) -> DeclaredTarget {
    DeclaredTarget {
        block_id: "block-2".to_owned(),
        kind: DeclaredTargetKind::PendingCandidate,
        block_text: "The wind stopped.".to_owned(),
        ..chapter.clone()
    }
}

#[test]
fn the_model_reads_the_exact_references_and_the_pending_candidate_target() {
    let DestinationRequest::Create(request) = crate::tests::create(
        crate::tests::route("http://127.0.0.1:9"),
        RequestAttempt::New,
    ) else {
        unreachable!("create() makes a Create request")
    };
    let chapter = request.declared_targets[0].clone();
    let request = CreateRequest {
        declared_targets: vec![chapter.clone(), pending_candidate(&chapter)],
        selection_references: references(&chapter),
        ..request
    };

    assert_eq!(
        request_text(&request),
        format!(
            "Author request:\nTighten this paragraph.\n\nWorking text:\nThe rain fell hard.\n\n\
             Author references:\n[reference 1] Chapter text of chapter_id {chapter_id}, from \
             block_id {block} character 4 to block_id {block} character 13 (UTF-16 code units):\n\
             rain fell\n\n[reference 2] Pending Proposal candidate text of block_id block-2, from \
             character 4 to character 8 (UTF-16 code units):\nwind\n\n\
             Target blocks:\n[block_id: {block}]\nThe rain fell hard on the old roof.\n\n\
             [block_id: block-2] (pending Proposal candidate)\nThe wind stopped.",
            chapter_id = chapter.chapter_id,
            block = chapter.block_id,
        )
    );
}

#[test]
fn a_joinable_block_is_marked_in_the_target_blocks() {
    let DestinationRequest::Create(request) = crate::tests::create(
        crate::tests::route("http://127.0.0.1:9"),
        RequestAttempt::New,
    ) else {
        unreachable!("create() makes a Create request")
    };
    let chapter = request.declared_targets[0].clone();
    let joinable = DeclaredTarget {
        kind: DeclaredTargetKind::JoinableBlock { distance: -1 },
        ..chapter.clone()
    };
    let request = CreateRequest {
        declared_targets: vec![joinable, pending_candidate(&chapter)],
        ..request
    };

    assert_eq!(
        request_text(&request)
            .split_once("Target blocks:\n")
            .map(|(_, targets)| targets.to_owned()),
        Some(format!(
            "[block_id: {}] (joinable with the pending Proposal candidate)\n\
             The rain fell hard on the old roof.\n\n\
             [block_id: block-2] (pending Proposal candidate)\nThe wind stopped.",
            chapter.block_id
        ))
    );
}

#[test]
fn the_compaction_estimate_counts_each_text_that_the_request_sends() {
    let DestinationRequest::Create(request) = crate::tests::create(
        crate::tests::route("http://127.0.0.1:9"),
        RequestAttempt::New,
    ) else {
        unreachable!("create() makes a Create request")
    };
    let chapter = request.declared_targets[0].clone();
    let mut decision =
        storyos_core::assemble_current_passage_context(&storyos_core::CurrentPassageAssembly {
            operation_requirement_id: "requirement-1".to_owned(),
            input_snapshot_id: "snapshot-1".to_owned(),
            run_id: "run-1".to_owned(),
            owner_user_id: "owner-1".to_owned(),
            project_id: "project-1".to_owned(),
            author_message: request.author_message.clone(),
            chapter_id: chapter.chapter_id.clone(),
            chapter_revision_id: Some(chapter.base_revision_id.clone()),
            proposal_target_block_ids: None,
            chapter_body: chapter.block_text.clone(),
            instruction: storyos_core::InstructionBindingInput::Absent,
            destination_identity: "destination-1".to_owned(),
        });
    decision.operation_requirement.selection_references = Some(references(&chapter));
    // The same mapping as `create_request` of the PostgreSQL dispatch.
    let request = CreateRequest {
        context: decision
            .selected
            .iter()
            .map(|item| RequestContextItem {
                source_class: item.source_class,
                content: item.content.clone(),
            })
            .collect(),
        declared_targets: vec![chapter.clone(), pending_candidate(&chapter)],
        selection_references: decision
            .operation_requirement
            .selection_references
            .clone()
            .unwrap_or_default(),
        ..request
    };
    let target_texts = request
        .declared_targets
        .iter()
        .map(|target| target.block_text.clone())
        .collect::<Vec<_>>();
    let estimate = CompactionTrigger::from_facts(&CompactionRequestFacts {
        model_registration_revision: storyos_core::AGENT_PLAN_REGISTRATION.revision,
        budget_bounds: storyos_core::BudgetBounds::AgentPlanRequestV2.as_str(),
        author_message: &request.author_message,
        decision: Some(&decision),
        declared_target_texts: &target_texts,
        prior_reported_tokens: None,
    })
    .new_turn_tokens;
    // With each sent text empty, the wire text keeps only its framing.
    let framing = CreateRequest {
        context: request
            .context
            .iter()
            .map(|item| RequestContextItem {
                content: String::new(),
                ..item.clone()
            })
            .collect(),
        declared_targets: request
            .declared_targets
            .iter()
            .map(|target| DeclaredTarget {
                block_text: String::new(),
                ..target.clone()
            })
            .collect(),
        selection_references: request
            .selection_references
            .iter()
            .map(|admitted| AdmittedSelection {
                selected_text: Some(String::new()),
                ..admitted.clone()
            })
            .collect(),
        ..request.clone()
    };
    let counted = |request: &CreateRequest| {
        storyos_core::count_stored_text(&request_text(request)).character_count
    };

    assert_eq!(counted(&request) - counted(&framing), estimate);
}
