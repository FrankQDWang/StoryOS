use super::admit_intent_pass;
use crate::CaptureRefusal;

/// The start uses the admission rule of a capture with the Purpose `intent_pass`, which the scope
/// `project_writing_assistance_v6` names first.
#[test]
fn the_start_of_an_intent_pass_needs_the_scope_that_names_its_purpose() {
    let scopes = [
        (false, true, Some("project_writing_assistance_v6")),
        (true, false, Some("project_writing_assistance_v6")),
        (true, true, Some("project_writing_assistance_v5")),
        (true, true, Some("project_writing_assistance_v6")),
    ];
    assert_eq!(
        scopes.map(|(binding, available, scope)| admit_intent_pass(binding, available, scope)),
        [
            Err(CaptureRefusal::NoModelUseBinding),
            Err(CaptureRefusal::AssistanceUnavailable),
            Err(CaptureRefusal::PurposeNotGranted),
            Ok(()),
        ]
    );
}

/// The source class and the content of each selected item of the context of `file`, with the
/// Purpose and the cause of the context.
fn context(file: super::CharacterFile<'_>) -> (Vec<(crate::ContextSourceClass, String)>, String) {
    let record = super::assemble_intent_pass_context(&super::IntentPassAssembly {
        operation_requirement_id: "requirement".to_owned(),
        run_id: "run".to_owned(),
        owner_user_id: "owner".to_owned(),
        project_id: "project".to_owned(),
        destination_identity: "destination".to_owned(),
        file,
    });
    let requirement = &record.operation_requirement;
    let selected = (record.selected.iter())
        .map(|item| (item.source_class, item.content.clone()))
        .collect();
    let kind = format!("{:?} {:?}", requirement.purpose, requirement.cause);
    (selected, kind)
}

/// ADR 0101 section 4: the context of a pass has one item of the class `character_file` and no
/// other item. The item has the name line and each part with text under its heading, and a file
/// with no entry of the author has no part `Told by the author`.
#[test]
fn the_context_of_a_pass_has_only_the_file_item() {
    let file = super::CharacterFile {
        name: "林舟",
        author_text: Some("林舟怕水。"),
        manuscript_text: "林舟是船夫。",
    };
    let class = crate::ContextSourceClass::CharacterFile;
    let kind = "IntentPass HostSchedule".to_owned();
    assert_eq!(
        [
            context(file),
            context(super::CharacterFile {
                author_text: None,
                ..file
            })
        ],
        [
            (
                vec![(
                    class,
                    "{\"name\":\"林舟\"}\n\nTold by the author:\n林舟怕水。\n\n\
                     From the manuscript:\n林舟是船夫。"
                        .to_owned()
                )],
                kind.clone()
            ),
            (
                vec![(
                    class,
                    "{\"name\":\"林舟\"}\n\nFrom the manuscript:\n林舟是船夫。".to_owned()
                )],
                kind
            )
        ]
    );
}
