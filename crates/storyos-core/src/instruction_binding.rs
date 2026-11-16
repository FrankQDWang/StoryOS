//! The Project Instruction Binding of one AgentRun and its Context item (ADR 0094 section 2).

use crate::assemble_context::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextBlockReason, ContextSourceClass,
    ContextSufficiency, CurrentPassageAssemblyRecord, InstructionBindingInput, ProjectionMode,
    RejectedSource, RejectionReason, SelectedProjection, count_context_item_tokens,
};

/// One Revision of the Project Instruction with its exact text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundInstruction {
    pub revision_id: String,
    pub text: String,
}

/// The binding that a new AgentRun records from the current Revision of its Project: the
/// identity of that Revision, or `None` for "none configured". A Project with no Revision and a
/// Revision whose text has only Unicode `White_Space` characters give "none configured".
pub fn project_instruction_binding(current: Option<&BoundInstruction>) -> Option<String> {
    current
        .filter(|revision| !revision.text.chars().all(char::is_whitespace))
        .map(|revision| revision.revision_id.clone())
}

/// Records the bound Revision `bound` in `record`: the Operation Requirement value
/// `required_revision`, the considered `instruction_binding` source with its counted tokens, and
/// one `exact_required` item with the exact text after the author instruction item. With no
/// binding, `record` keeps the value `absent` and has no item.
pub fn assemble_instruction_binding(
    record: &mut CurrentPassageAssemblyRecord,
    bound: Option<&BoundInstruction>,
) {
    let Some(bound) = bound else {
        return;
    };
    let token_count = count_context_item_tokens(&bound.text);
    let within_limit = token_count <= CONTEXT_ITEM_TOKEN_LIMIT;
    record.operation_requirement.instruction = InstructionBindingInput::RequiredRevision {
        revision_id: bound.revision_id.clone(),
        available: true,
    };
    let considered = ConsideredSource {
        source_class: ContextSourceClass::InstructionBinding,
        source_version: bound.revision_id.clone(),
        token_count,
        eligible: within_limit,
    };
    match record
        .considered
        .iter_mut()
        .find(|source| source.source_class == ContextSourceClass::InstructionBinding)
    {
        Some(source) => *source = considered,
        None => record.considered.push(considered),
    }
    if !within_limit {
        record.rejected.push(RejectedSource {
            source_class: ContextSourceClass::InstructionBinding,
            source_version: bound.revision_id.clone(),
            token_count,
            reason: RejectionReason::OverItemTokenLimit,
        });
        let reason = ContextBlockReason::ExactRequiredOverLimit {
            source_class: ContextSourceClass::InstructionBinding,
        };
        match &mut record.sufficiency {
            ContextSufficiency::Complete => {
                record.sufficiency = ContextSufficiency::Blocked {
                    reasons: vec![reason],
                };
            }
            ContextSufficiency::Blocked { reasons } => reasons.push(reason),
        }
        return;
    }
    let position = record
        .selected
        .iter()
        .position(|item| item.source_class == ContextSourceClass::AuthorInstruction)
        .map_or(0, |index| index + 1);
    record.selected.insert(
        position,
        SelectedProjection {
            source_class: ContextSourceClass::InstructionBinding,
            source_version: bound.revision_id.clone(),
            projection_mode: ProjectionMode::ExactRequired,
            token_count,
            content: bound.text.clone(),
        },
    );
}

#[cfg(test)]
#[path = "instruction_binding_tests.rs"]
mod tests;
