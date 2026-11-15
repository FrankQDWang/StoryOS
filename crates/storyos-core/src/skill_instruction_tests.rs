use super::{
    SkillInstructionSource, SkillInstructionUnavailable, SkillProvenance, SkillRequirement,
    SkillSelectionUnavailable, admit_skill_selections, assemble_skill_catalog,
    assemble_skill_instructions, skill_reference_well_formed,
};
use crate::assemble_context::{SelectedProjection, count_context_item_tokens};
use crate::{
    ContextSourceClass, CurrentPassageAssemblyRecord, FrontmatterValue, ProjectionMode,
    SKILL_MD_PATH, SkillCatalogEntry, SkillFileInput, SkillPackage, SkillPackageInput,
    SkillPackageReference, SkillSet, ToolResultSource, admit_skill_package, hex_sha256,
};

fn package(name: &str) -> SkillPackage {
    admit_skill_package(SkillPackageInput {
        directory_name: name.to_owned(),
        files: vec![SkillFileInput {
            path: SKILL_MD_PATH.to_owned(),
            bytes: skill_md(name).into_bytes(),
        }],
        frontmatter: Some(vec![
            ("name".to_owned(), FrontmatterValue::Text(name.to_owned())),
            (
                "description".to_owned(),
                FrontmatterValue::Text(format!("Use it for {name}.")),
            ),
        ]),
    })
    .unwrap()
}

fn skill_md(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Use it for {name}.\n---\n用「」写对白。\n")
}

fn requirement(package: &SkillPackage) -> SkillRequirement {
    SkillRequirement {
        skill_name: package.name.clone(),
        package_digest: package.package_digest.clone(),
        provenance: SkillProvenance::AuthorSelected,
    }
}

fn entry(package: &SkillPackage) -> SkillCatalogEntry {
    SkillCatalogEntry {
        reference: package.reference(),
        description: package.description.clone(),
    }
}

fn decision() -> CurrentPassageAssemblyRecord {
    crate::assemble_current_passage_context(&crate::CurrentPassageAssembly {
        operation_requirement_id: "requirement-1".to_owned(),
        input_snapshot_id: "snapshot-1".to_owned(),
        run_id: "run-1".to_owned(),
        owner_user_id: "owner-1".to_owned(),
        project_id: "project-1".to_owned(),
        author_message: "Help with this passage.".to_owned(),
        chapter_id: "chapter-1".to_owned(),
        chapter_revision_id: Some("rev-1".to_owned()),
        proposal_target_block_ids: None,
        chapter_body: "The rain.".to_owned(),
        instruction: crate::InstructionBindingInput::Absent,
        destination_identity: "destination-1".to_owned(),
    })
}

#[test]
fn a_selection_admits_each_offered_reference_in_name_order_and_refuses_any_other() {
    let (alpha, beta) = (package("alpha-voice"), package("beta-notes"));
    let offered = SkillSet::new(vec![alpha.clone(), beta.clone()]).unwrap();
    let foreign = SkillPackageReference {
        name: alpha.name.clone(),
        package_digest: beta.package_digest.clone(),
    };

    assert_eq!(
        [
            admit_skill_selections(&offered, &[beta.reference(), alpha.reference()]),
            admit_skill_selections(&offered, &[alpha.reference(), foreign]),
            admit_skill_selections(&SkillSet::default(), &[alpha.reference()]),
        ],
        [
            Ok(vec![requirement(&alpha), requirement(&beta)]),
            Err(SkillSelectionUnavailable),
            Err(SkillSelectionUnavailable),
        ]
    );
}

#[test]
fn a_reference_is_well_formed_only_with_the_name_rule_and_the_digest_form() {
    let digest = format!("sha256:{}", "0a".repeat(32));
    let reference = |name: &str, package_digest: &str| SkillPackageReference {
        name: name.to_owned(),
        package_digest: package_digest.to_owned(),
    };

    assert_eq!(
        [
            reference("dialogue-style-guide", &digest),
            reference("Dialogue", &digest),
            reference("dialogue--guide", &digest),
            reference("dialogue", &digest.to_uppercase()),
            reference("dialogue", &digest[..70]),
            reference("dialogue", &digest.replace("sha256:", "sha512:")),
        ]
        .map(|reference| skill_reference_well_formed(&reference)),
        [true, false, false, false, false, false]
    );
}

#[test]
fn each_requirement_adds_the_exact_skill_md_text_of_its_snapshot_as_one_item() {
    let (alpha, beta) = (package("alpha-voice"), package("beta-notes"));
    let mut record = decision();
    let before = record.selected.clone();
    let sources = [&alpha, &beta].map(|package| SkillInstructionSource {
        requirement: requirement(package),
        skill_md: Some(skill_md(&package.name).into_bytes()),
    });
    let item = |package: &SkillPackage| SelectedProjection {
        source_class: ContextSourceClass::SkillInstruction,
        source_version: package.package_digest.clone(),
        projection_mode: ProjectionMode::ExactRequired,
        token_count: count_context_item_tokens(&skill_md(&package.name)),
        content: skill_md(&package.name),
    };

    assert_eq!(
        assemble_skill_instructions(&mut record, sources.to_vec()),
        Ok(())
    );
    assert_eq!(
        (
            record.selected.clone(),
            record.operation_requirement.skill_requirements.clone(),
            crate::decode_assembly_record(&crate::encode_assembly_record(&record)),
        ),
        (
            [before, vec![item(&alpha), item(&beta)]].concat(),
            Some(vec![requirement(&alpha), requirement(&beta)]),
            Some(record.clone()),
        )
    );
}

#[test]
fn a_snapshot_with_no_skill_md_text_is_unavailable() {
    let alpha = package("alpha-voice");
    let assembled = |skill_md: Option<Vec<u8>>| {
        let mut record = decision();
        let source = SkillInstructionSource {
            requirement: requirement(&alpha),
            skill_md,
        };
        assemble_skill_instructions(&mut record, vec![source])
    };

    assert_eq!(
        [assembled(None), assembled(Some(vec![0xff, 0xfe]))],
        [Err(SkillInstructionUnavailable); 2]
    );
}

#[test]
fn the_expected_input_counts_the_skill_and_catalog_text_only_when_the_request_sends_the_current_input()
 {
    let alpha = package("alpha-voice");
    let skill_tokens = count_context_item_tokens(&skill_md(&alpha.name));
    let mut plain = decision();
    let source = SkillInstructionSource {
        requirement: requirement(&alpha),
        skill_md: Some(skill_md(&alpha.name).into_bytes()),
    };
    let mut selected = plain.clone();
    assemble_skill_instructions(&mut selected, vec![source]).unwrap();
    assemble_skill_catalog(&mut selected, &[entry(&alpha)]);
    let catalog_tokens = count_context_item_tokens(&crate::skill_catalog_text(&[entry(&alpha)]));
    let output = SelectedProjection {
        source_class: ContextSourceClass::ToolResult,
        source_version: "tool-call-1".to_owned(),
        projection_mode: ProjectionMode::ExactRequired,
        token_count: 7,
        content: "<tool_result/>".to_owned(),
    };
    for record in [&mut plain, &mut selected] {
        record.selected.push(output.clone());
        record.operation_requirement.tool_results = Some(vec![ToolResultSource {
            tool_call_id: "tool-call-1".to_owned(),
            call_id: "call-1".to_owned(),
            model_attempt_id: "attempt-1".to_owned(),
            decision_id: "decision-1".to_owned(),
            state: "ok".to_owned(),
            trust: "host".to_owned(),
            kept_result_tokens: None,
            source_snapshot_ids: Vec::new(),
            skill: None,
        }]);
    }
    let sent = |record: &CurrentPassageAssemblyRecord, prior_input| {
        crate::CompactionTrigger::from_facts(&crate::CompactionRequestFacts {
            model_registration_revision: crate::AGENT_PLAN_REGISTRATION.revision,
            budget_bounds: crate::BudgetBounds::AgentPlanRequestV2.as_str(),
            author_message: "Help with this passage.",
            decision: Some(record),
            declared_target_texts: &[],
            prior_reported_tokens: Some(1_000),
            prior_decision_id: Some("decision-1"),
            prior_input,
            decision_position: "1",
            compacted_earlier: false,
        })
        .sent_input_tokens
    };
    let full = sent(&plain, crate::PriorInput::Replaced);

    // An incremental request sends only the Tool output. A request that sends its current input
    // again also sends the Skill text and the catalog text.
    assert_eq!(
        [
            sent(&plain, crate::PriorInput::Current),
            sent(&selected, crate::PriorInput::Current),
            sent(&selected, crate::PriorInput::Replaced),
        ],
        [7, 7, full + skill_tokens + catalog_tokens]
    );
}

#[test]
fn the_pinned_catalog_adds_one_exact_catalog_item_after_the_skill_instruction_items() {
    let (alpha, beta) = (package("alpha-voice"), package("beta-notes"));
    let mut record = decision();
    let source = SkillInstructionSource {
        requirement: requirement(&beta),
        skill_md: Some(skill_md(&beta.name).into_bytes()),
    };
    assemble_skill_instructions(&mut record, vec![source]).unwrap();
    let before = record.selected.clone();
    let text = "{\"description\":\"Use it for alpha-voice.\",\"name\":\"alpha-voice\"}\n\
                {\"description\":\"Use it for beta-notes.\",\"name\":\"beta-notes\"}";
    let mut empty = decision();

    assemble_skill_catalog(&mut record, &[entry(&beta), entry(&alpha)]);
    assemble_skill_catalog(&mut empty, &[]);
    assert_eq!(
        (
            record.selected.clone(),
            record.operation_requirement.skill_catalog.clone(),
            crate::decode_assembly_record(&crate::encode_assembly_record(&record)),
            empty,
        ),
        (
            [
                before,
                vec![SelectedProjection {
                    source_class: ContextSourceClass::SkillCatalog,
                    source_version: format!("sha256:{}", hex_sha256(text.as_bytes())),
                    projection_mode: ProjectionMode::ExactRequired,
                    token_count: count_context_item_tokens(text),
                    content: text.to_owned(),
                }],
            ]
            .concat(),
            Some(vec![alpha.reference(), beta.reference()]),
            Some(record.clone()),
            decision(),
        )
    );
}
