use super::{SubrunAssembly, assemble_subrun_context, next_subrun_context};
use crate::shared_tests::tool_result_candidate;
use crate::{
    BoundInstruction, ChapterListEntry, ContextSourceClass, MemoryIndexEntry, ToolCallState,
};

fn chapter() -> ChapterListEntry {
    ChapterListEntry {
        volume_order: 1,
        volume_title: "Volume one".to_owned(),
        chapter_id: "chapter-1".to_owned(),
        title: "The river".to_owned(),
        length: 12,
    }
}

/// ADR 0103 section 3A: the first RunStep of a Subrun has the Project Instruction of its parent,
/// the brief, the Chapter list, and the Memory Index of the pin of its parent, in this order. With
/// no Memory use of the parent, it has no Memory Index. A later RunStep keeps these items and adds
/// the Tool results of the Subrun.
#[test]
fn a_subrun_has_the_instruction_the_brief_the_chapter_list_and_the_memory_index_of_its_parent() {
    let instruction = BoundInstruction {
        revision_id: "instruction-1".to_owned(),
        text: "Keep the past tense.".to_owned(),
    };
    let entries = [chapter()];
    let index = [MemoryIndexEntry {
        number: 1,
        entry_type: crate::MemoryEntryType::Character,
        title: "Lin Zhou".to_owned(),
        description: "A boatman's son.".to_owned(),
        flagged: false,
        author_part: false,
        intent: None,
    }];
    let assembly = |memory_pin| SubrunAssembly {
        operation_requirement_id: "requirement-1",
        run_id: "run-1",
        owner_user_id: "user-1",
        project_id: "proj-1",
        destination_identity: "destination",
        brief: "Find the first meeting of Lin Zhou and the boatman.",
        instruction: Some(&instruction),
        chapters: &entries,
        memory_pin,
        memory_index: &index,
    };
    let classes = |record: &crate::CurrentPassageAssemblyRecord| {
        (record.selected.iter())
            .map(|item| item.source_class)
            .collect::<Vec<_>>()
    };
    let first = assemble_subrun_context(&assembly(Some(3)));
    let output = "Chapter 1 text";
    let next = next_subrun_context(
        &first,
        "requirement-2".to_owned(),
        &[tool_result_candidate(
            "call-1",
            ToolCallState::Settled,
            Some(output),
        )],
    );
    // A third RunStep keeps only the items of the first RunStep and the new results.
    let third = next_subrun_context(
        &next,
        "requirement-3".to_owned(),
        &[tool_result_candidate(
            "call-2",
            ToolCallState::Settled,
            Some(output),
        )],
    );
    let read = [
        ContextSourceClass::InstructionBinding,
        ContextSourceClass::SubrunBrief,
        ContextSourceClass::ChapterList,
        ContextSourceClass::MemoryIndex,
    ];

    assert_eq!(
        (
            first.operation_requirement.purpose,
            first.operation_requirement.memory_revision,
            classes(&first),
            classes(&assemble_subrun_context(&assembly(None))),
            next.operation_requirement.operation_requirement_id.as_str(),
            classes(&next),
            classes(&third),
        ),
        (
            crate::assemble_context::ContextPurpose::Subrun,
            Some(3),
            read.to_vec(),
            read[..3].to_vec(),
            "requirement-2",
            [&read[..], &[ContextSourceClass::ToolResult]].concat(),
            [&read[..], &[ContextSourceClass::ToolResult]].concat(),
        )
    );
}
