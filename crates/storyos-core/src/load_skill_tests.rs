use serde_json::json;

use super::{LoadSkillRefusal, SkillLoadFacts, admit_skill_load, skill_load_result};
use crate::{
    CallValidationRefusal, ContextSourceClass, CurrentPassageAssembly,
    CurrentPassageAssemblyRecord, InstructionBindingInput, IntakeRefusal, LOAD_SKILL_TOOL_SPEC,
    ProjectionMode, SkillInstructionSource, SkillPackageReference, SkillProvenance,
    SkillRequirement, ToolCallState, ToolIntake, ToolResultCandidate, ToolResultForm,
    ToolResultOutput, assemble_current_passage_context, assemble_skill_instructions,
    assemble_tool_result_continuation, compaction_assembly_record, function_outputs,
    is_function_output, is_skill_load_output,
};

fn reference(name: &str) -> SkillPackageReference {
    SkillPackageReference {
        name: name.to_owned(),
        package_digest: format!("sha256:{}", "d".repeat(64)),
    }
}

#[test]
fn the_intake_refuses_a_reason_that_is_empty_too_long_or_with_a_control_character() {
    let intake = |skill: &str, reason: &str| {
        LOAD_SKILL_TOOL_SPEC.intake(&json!({"skill": skill, "reason": reason}))
    };
    let at_limit = "a".repeat(512);

    assert_eq!(
        (
            intake("dialogue", "The author asks for dialogue."),
            intake("dialogue", &at_limit),
            intake("dialogue", ""),
            intake("dialogue", "  "),
            intake("dialogue", &"a".repeat(513)),
            intake("dialogue", "Two\nlines."),
            intake("dia\0logue", "The author asks for dialogue."),
            LOAD_SKILL_TOOL_SPEC.intake(&json!({"skill": "dialogue"})),
        ),
        (
            Ok(ToolIntake::LoadSkill {
                skill: "dialogue".to_owned(),
                reason: "The author asks for dialogue.".to_owned()
            }),
            Ok(ToolIntake::LoadSkill {
                skill: "dialogue".to_owned(),
                reason: at_limit.clone()
            }),
            Err(IntakeRefusal::Reason),
            Err(IntakeRefusal::Reason),
            Err(IntakeRefusal::Reason),
            Err(IntakeRefusal::Reason),
            Err(IntakeRefusal::UnstorableText),
            Err(IntakeRefusal::Malformed),
        )
    );
    let refusal = CallValidationRefusal::Intake(IntakeRefusal::Reason);
    assert_eq!(
        (refusal.label(), refusal.result_text()),
        (
            "intake_reason",
            "The Host refused this call because its reason is empty, has more than 512 \
             characters, or has a control character, so no result is available, and this is not \
             an empty result."
                .to_owned()
        )
    );
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// ADR 0089 sections 7 and 9: a load of a name that is not in the pinned catalog, of a Skill that
/// the Run has, or above 8 Skills of the Run is a refused call, and the refusals apply in this
/// order. The result of an admitted load is the exact `SKILL.md` text.
#[test]
fn a_load_admits_a_new_catalog_skill_below_the_limit_and_gives_the_exact_skill_md() {
    let catalog: Vec<_> = (0..10)
        .map(|index| reference(&format!("s{index}")))
        .collect();
    let text = "---\nname: dialogue\n---\nKeep <each> line & say \"no\".\n";
    let admit = |selected: &[&str], loaded: &[&str], skill| {
        let (selected, loaded) = (names(selected), names(loaded));
        let facts = SkillLoadFacts {
            catalog: &catalog,
            selected: &selected,
            loaded: &loaded,
        };
        admit_skill_load(&facts, skill).cloned()
    };
    let seven = ["s0", "s1", "s2", "s3", "s4", "s5", "s6"];
    let eight = ["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7"];

    assert_eq!(
        [
            admit(&[], &[], "s9"),
            admit(&[], &[], "S9"),
            admit(&[], &eight, "unknown"),
            admit(&["s1"], &[], "s1"),
            admit(&[], &["s1"], "s1"),
            admit(&[], &eight, "s7"),
            admit(&["s0", "s1"], &eight[2..], "s8"),
            admit(&[], &eight, "s8"),
            admit(&[], &seven, "s8"),
            admit(&["s0", "s1"], &seven[2..], "s8"),
        ],
        [
            Ok(catalog[9].clone()),
            Err(LoadSkillRefusal::NotInCatalog),
            Err(LoadSkillRefusal::NotInCatalog),
            Err(LoadSkillRefusal::AlreadyLoaded),
            Err(LoadSkillRefusal::AlreadyLoaded),
            Err(LoadSkillRefusal::AlreadyLoaded),
            Err(LoadSkillRefusal::LoadLimit),
            Err(LoadSkillRefusal::LoadLimit),
            Ok(catalog[8].clone()),
            Ok(catalog[8].clone()),
        ]
    );
    assert_eq!(
        [LoadSkillRefusal::AlreadyLoaded, LoadSkillRefusal::LoadLimit]
            .map(|refusal| (refusal.label(), refusal.result().text)),
        [
            (
                "skill_already_loaded",
                "The Host refused this call because this run already has the instructions of \
                 this Skill, so no result is available, and this is not an empty result."
                    .to_owned()
            ),
            (
                "skill_load_limit",
                "The Host refused this call because this run already has the instructions of 8 \
                 Skills, so no result is available, and this is not an empty result."
                    .to_owned()
            ),
        ]
    );
    let refused = LoadSkillRefusal::NotInCatalog.result();
    assert_eq!(
        (
            LoadSkillRefusal::NotInCatalog.label(),
            refused.text.as_str()
        ),
        (
            "skill_not_in_catalog",
            "The Host refused this call because the section Skill catalog has no Skill with this \
             name, so no result is available, and this is not an empty result."
        )
    );
    let loaded = skill_load_result(Some(text.as_bytes().to_vec())).unwrap();
    assert_eq!(
        (
            loaded.text.as_str(),
            loaded.bytes,
            skill_load_result(None),
            skill_load_result(Some(vec![0xff])),
        ),
        (text, text.len(), None, None)
    );
}

fn passage() -> CurrentPassageAssemblyRecord {
    assemble_current_passage_context(&CurrentPassageAssembly {
        operation_requirement_id: "req-1".to_owned(),
        input_snapshot_id: "snap-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "user-1".to_owned(),
        project_id: "proj-1".to_owned(),
        author_message: "Write the quarrel.".to_owned(),
        chapter_id: "ch-1".to_owned(),
        chapter_revision_id: Some("ch-1-rev".to_owned()),
        proposal_target_block_ids: Some(vec!["ch-1-block".to_owned()]),
        chapter_body: "They met at the door.".to_owned(),
        instruction: InstructionBindingInput::Absent,
        destination_identity: "dest-1".to_owned(),
    })
}

fn call<'a>(
    tool_call_id: &'a str,
    state: ToolCallState,
    result_text: &'a str,
    skill: Option<&'a SkillPackageReference>,
) -> ToolResultCandidate<'a> {
    ToolResultCandidate {
        tool_call_id,
        call_id: tool_call_id,
        owner_user_id: "user-1",
        project_id: "proj-1",
        run_id: "run-1",
        model_attempt_id: "attempt-1",
        decision_id: "decision-1",
        state,
        result_text: Some(result_text),
        sources: &[],
        output: ToolResultOutput::HostTool,
        function_name: "load_skill",
        arguments: "{}",
        form: ToolResultForm::Stub,
        skill,
        resource_path: None,
    }
}

/// ADR 0089 section 3: the output of a settled load is a `skill_instruction` item with the exact
/// text, and the role rule tells it from a selected Skill with the same text by the Operation
/// Requirement. A refused load stays a `tool_result` item. The summary request of a compaction
/// keeps the load output that waits for it.
#[test]
fn the_role_rule_tells_a_loaded_skill_output_from_a_selected_skill_section() {
    let text = "---\nname: dialogue\n---\nKeep <each> line & say \"no\".";
    let dialogue = reference("dialogue");
    let mut record = passage();
    assemble_skill_instructions(
        &mut record,
        vec![SkillInstructionSource {
            requirement: SkillRequirement {
                skill_name: dialogue.name.clone(),
                package_digest: dialogue.package_digest.clone(),
                provenance: SkillProvenance::AuthorSelected,
            },
            skill_md: Some(text.as_bytes().to_vec()),
        }],
    )
    .unwrap();
    let refused = LoadSkillRefusal::NotInCatalog.result().text;
    assemble_tool_result_continuation(
        &mut record,
        &[
            call("load-1", ToolCallState::Settled, text, Some(&dialogue)),
            call("load-2", ToolCallState::Refused, &refused, None),
            call("load-3", ToolCallState::Refused, &refused, Some(&dialogue)),
        ],
    );

    // Each selected `skill_instruction` item and each function output, with its role.
    let roles: Vec<_> = record
        .selected
        .iter()
        .filter(|item| {
            item.source_class == ContextSourceClass::SkillInstruction
                || is_function_output(&record, item)
        })
        .map(|item| {
            let role = match (
                is_skill_load_output(&record, item),
                is_function_output(&record, item),
            ) {
                (true, _) => "load output",
                (false, true) => "tool_result output",
                (false, false) => "section",
            };
            (item.source_version.as_str(), item.projection_mode, role)
        })
        .collect();
    let exact = ProjectionMode::ExactRequired;
    let digest = dialogue.package_digest.as_str();
    assert_eq!(
        roles,
        [
            (digest, exact, "section"),
            ("load-1", exact, "load output"),
            ("load-2", exact, "tool_result output"),
            ("load-3", exact, "tool_result output"),
        ]
    );
    let outputs: Vec<_> = function_outputs(&record)
        .into_iter()
        .map(|(source, item)| {
            (
                source.skill.as_ref(),
                source.trust.as_str(),
                item.content.as_str(),
            )
        })
        .collect();
    let refused_item = |id: &str| {
        format!("<tool_result source=\"{id}\" state=\"rejected\">{refused}</tool_result>")
    };
    assert_eq!(
        outputs,
        [
            (Some(&dialogue), "skill_instruction", text),
            (None, "data_only", refused_item("load-2").as_str()),
            (None, "data_only", refused_item("load-3").as_str()),
        ]
    );

    let summary = compaction_assembly_record(&record, &["load-1"]);
    assert_eq!(
        summary
            .selected
            .iter()
            .map(|item| (item.source_class, item.content.as_str()))
            .collect::<Vec<_>>(),
        [(ContextSourceClass::SkillInstruction, text)]
    );
    assert_eq!(
        function_outputs(&summary)
            .into_iter()
            .map(|(source, _)| source.call_id.as_str())
            .collect::<Vec<_>>(),
        ["load-1"]
    );
}
