use super::{
    CONTEXT_ITEM_TOKEN_LIMIT, ConsideredSource, ContextBlockReason, ContextCause, ContextPurpose,
    ContextSourceClass, ContextSufficiency, CurrentPassageAssemblyRecord, DestinationIo,
    HostControlRecord, InstructionBindingInput, ManifestCommit, OperationRequirementRecord,
    ProjectionMode, RejectedSource, RejectionReason, SelectedProjection,
    TOKEN_COUNTING_PROFILE_REVISION,
};

pub fn encode_assembly_record(record: &CurrentPassageAssemblyRecord) -> serde_json::Value {
    serde_json::json!({
        "operation_requirement": {
            "operation_requirement_id": record.operation_requirement.operation_requirement_id,
            "input_snapshot_id": record.operation_requirement.input_snapshot_id,
            "run_id": record.operation_requirement.run_id,
            "owner_user_id": record.operation_requirement.owner_user_id,
            "project_id": record.operation_requirement.project_id,
            "chapter_id": record.operation_requirement.chapter_id,
            "chapter_revision_id": record.operation_requirement.chapter_revision_id,
            "instruction": encode_instruction(&record.operation_requirement.instruction),
            "destination_identity": record.operation_requirement.destination_identity,
        },
        "sufficiency": encode_sufficiency(&record.sufficiency),
        "considered": record.considered.iter().map(encode_considered).collect::<Vec<_>>(),
        "selected": record.selected.iter().map(encode_selected).collect::<Vec<_>>(),
        "rejected": record.rejected.iter().map(encode_rejected).collect::<Vec<_>>(),
        "token_counting_profile_revision": record.token_counting_profile_revision,
        "token_counting_algorithm_revision": record.token_counting_algorithm_revision,
        "destination_and_outbound": record.manifests.destination_and_outbound,
    })
}

pub fn decode_assembly_record(value: &serde_json::Value) -> Option<CurrentPassageAssemblyRecord> {
    let requirement = value.get("operation_requirement")?;
    let instruction = decode_instruction(requirement.get("instruction")?)?;
    let considered = value
        .get("considered")?
        .as_array()?
        .iter()
        .map(decode_considered)
        .collect::<Option<Vec<_>>>()?;
    let selected = value
        .get("selected")?
        .as_array()?
        .iter()
        .map(decode_selected)
        .collect::<Option<Vec<_>>>()?;
    let rejected = value
        .get("rejected")?
        .as_array()?
        .iter()
        .map(decode_rejected)
        .collect::<Option<Vec<_>>>()?;
    let destination_and_outbound = value.get("destination_and_outbound")?.as_bool()?;
    Some(CurrentPassageAssemblyRecord {
        operation_requirement: OperationRequirementRecord {
            operation_requirement_id: requirement
                .get("operation_requirement_id")?
                .as_str()?
                .to_owned(),
            input_snapshot_id: requirement.get("input_snapshot_id")?.as_str()?.to_owned(),
            run_id: requirement.get("run_id")?.as_str()?.to_owned(),
            owner_user_id: requirement.get("owner_user_id")?.as_str()?.to_owned(),
            project_id: requirement.get("project_id")?.as_str()?.to_owned(),
            purpose: ContextPurpose::CurrentPassageAssistance,
            cause: ContextCause::AuthorRequest,
            chapter_id: requirement.get("chapter_id")?.as_str()?.to_owned(),
            chapter_revision_id: match requirement.get("chapter_revision_id")? {
                serde_json::Value::Null => None,
                value => Some(value.as_str()?.to_owned()),
            },
            instruction,
            destination_identity: requirement
                .get("destination_identity")?
                .as_str()?
                .to_owned(),
            item_token_limit: CONTEXT_ITEM_TOKEN_LIMIT,
            token_counting_profile_revision: TOKEN_COUNTING_PROFILE_REVISION.to_owned(),
        },
        sufficiency: decode_sufficiency(value.get("sufficiency")?)?,
        considered,
        selected,
        rejected,
        host_control: HostControlRecord {
            distinct_from_destination: true,
            destination_visible: false,
        },
        token_counting_profile_revision: value
            .get("token_counting_profile_revision")?
            .as_str()?
            .to_owned(),
        token_counting_algorithm_revision: value
            .get("token_counting_algorithm_revision")?
            .as_str()?
            .to_owned(),
        manifests: ManifestCommit {
            assembly: true,
            destination_and_outbound,
        },
        destination_io: DestinationIo::None,
    })
}

fn encode_instruction(instruction: &InstructionBindingInput) -> serde_json::Value {
    match instruction {
        InstructionBindingInput::Absent => serde_json::json!({"kind": "absent"}),
        InstructionBindingInput::RequiredRevision {
            revision_id,
            available,
        } => serde_json::json!({
            "kind": "required_revision",
            "revision_id": revision_id,
            "available": available,
        }),
    }
}

fn decode_instruction(value: &serde_json::Value) -> Option<InstructionBindingInput> {
    match value.get("kind")?.as_str()? {
        "absent" => Some(InstructionBindingInput::Absent),
        "required_revision" => Some(InstructionBindingInput::RequiredRevision {
            revision_id: value.get("revision_id")?.as_str()?.to_owned(),
            available: value.get("available")?.as_bool()?,
        }),
        _ => None,
    }
}

fn encode_sufficiency(sufficiency: &ContextSufficiency) -> serde_json::Value {
    match sufficiency {
        ContextSufficiency::Complete => serde_json::json!({"kind": "complete"}),
        ContextSufficiency::Blocked { reasons } => serde_json::json!({
            "kind": "blocked",
            "reasons": reasons.iter().map(encode_block_reason).collect::<Vec<_>>(),
        }),
    }
}

fn decode_sufficiency(value: &serde_json::Value) -> Option<ContextSufficiency> {
    match value.get("kind")?.as_str()? {
        "complete" => Some(ContextSufficiency::Complete),
        "blocked" => Some(ContextSufficiency::Blocked {
            reasons: value
                .get("reasons")?
                .as_array()?
                .iter()
                .map(decode_block_reason)
                .collect::<Option<Vec<_>>>()?,
        }),
        _ => None,
    }
}

fn encode_block_reason(reason: &ContextBlockReason) -> serde_json::Value {
    match reason {
        ContextBlockReason::ExactRequiredOverLimit { source_class } => serde_json::json!({
            "kind": "exact_required_over_limit",
            "source_class": encode_source_class(*source_class),
        }),
        ContextBlockReason::RequiredInstructionRevisionUnavailable => {
            serde_json::json!({"kind": "required_instruction_revision_unavailable"})
        }
        ContextBlockReason::WorkingTargetRevisionUnavailable => {
            serde_json::json!({"kind": "working_target_revision_unavailable"})
        }
    }
}

fn decode_block_reason(value: &serde_json::Value) -> Option<ContextBlockReason> {
    match value.get("kind")?.as_str()? {
        "exact_required_over_limit" => Some(ContextBlockReason::ExactRequiredOverLimit {
            source_class: decode_source_class(value.get("source_class")?.as_str()?)?,
        }),
        "required_instruction_revision_unavailable" => {
            Some(ContextBlockReason::RequiredInstructionRevisionUnavailable)
        }
        "working_target_revision_unavailable" => {
            Some(ContextBlockReason::WorkingTargetRevisionUnavailable)
        }
        _ => None,
    }
}

fn encode_considered(source: &ConsideredSource) -> serde_json::Value {
    serde_json::json!({
        "source_class": encode_source_class(source.source_class),
        "source_version": source.source_version,
        "token_count": source.token_count,
        "eligible": source.eligible,
    })
}

fn decode_considered(value: &serde_json::Value) -> Option<ConsideredSource> {
    Some(ConsideredSource {
        source_class: decode_source_class(value.get("source_class")?.as_str()?)?,
        source_version: value.get("source_version")?.as_str()?.to_owned(),
        token_count: value.get("token_count")?.as_u64()?,
        eligible: value.get("eligible")?.as_bool()?,
    })
}

fn encode_selected(source: &SelectedProjection) -> serde_json::Value {
    serde_json::json!({
        "source_class": encode_source_class(source.source_class),
        "source_version": source.source_version,
        "projection_mode": match source.projection_mode {
            ProjectionMode::ExactRequired => "exact_required",
        },
        "token_count": source.token_count,
        "content": source.content,
    })
}

fn decode_selected(value: &serde_json::Value) -> Option<SelectedProjection> {
    Some(SelectedProjection {
        source_class: decode_source_class(value.get("source_class")?.as_str()?)?,
        source_version: value.get("source_version")?.as_str()?.to_owned(),
        projection_mode: match value.get("projection_mode")?.as_str()? {
            "exact_required" => ProjectionMode::ExactRequired,
            _ => return None,
        },
        token_count: value.get("token_count")?.as_u64()?,
        content: value.get("content")?.as_str()?.to_owned(),
    })
}

fn encode_rejected(source: &RejectedSource) -> serde_json::Value {
    serde_json::json!({
        "source_class": encode_source_class(source.source_class),
        "source_version": source.source_version,
        "token_count": source.token_count,
        "reason": match source.reason {
            RejectionReason::OverItemTokenLimit => "over_item_token_limit",
            RejectionReason::RequiredRevisionUnavailable => "required_revision_unavailable",
            RejectionReason::WorkingTargetRevisionUnavailable => {
                "working_target_revision_unavailable"
            }
        },
    })
}

fn decode_rejected(value: &serde_json::Value) -> Option<RejectedSource> {
    Some(RejectedSource {
        source_class: decode_source_class(value.get("source_class")?.as_str()?)?,
        source_version: value.get("source_version")?.as_str()?.to_owned(),
        token_count: value.get("token_count")?.as_u64()?,
        reason: match value.get("reason")?.as_str()? {
            "over_item_token_limit" => RejectionReason::OverItemTokenLimit,
            "required_revision_unavailable" => RejectionReason::RequiredRevisionUnavailable,
            "working_target_revision_unavailable" => {
                RejectionReason::WorkingTargetRevisionUnavailable
            }
            _ => return None,
        },
    })
}

fn encode_source_class(class: ContextSourceClass) -> &'static str {
    match class {
        ContextSourceClass::HostControl => "host_control",
        ContextSourceClass::AuthorInstruction => "author_instruction",
        ContextSourceClass::WorkingTarget => "working_target",
        ContextSourceClass::InstructionBinding => "instruction_binding",
    }
}

fn decode_source_class(value: &str) -> Option<ContextSourceClass> {
    match value {
        "host_control" => Some(ContextSourceClass::HostControl),
        "author_instruction" => Some(ContextSourceClass::AuthorInstruction),
        "working_target" => Some(ContextSourceClass::WorkingTarget),
        "instruction_binding" => Some(ContextSourceClass::InstructionBinding),
        _ => None,
    }
}
