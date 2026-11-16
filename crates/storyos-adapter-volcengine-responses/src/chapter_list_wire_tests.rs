use serde_json::json;
use storyos_application::CreateRequest;
use storyos_core::ContextSourceClass;

use crate::skill_wire_tests::{body, closing, first, item, summary};

const V8: &str = storyos_core::AGENT_PLAN_CONTINUITY_MAPPING_REVISION;

/// A request of the mapping `v8` with `context`.
fn v8(context: Vec<storyos_application::RequestContextItem>) -> CreateRequest {
    CreateRequest {
        context,
        ..first(V8, storyos_core::AGENT_PLAN_REGISTRATION.revision)
    }
}

/// ADR 0095 section 1: the `chapter_list` item is one section `Chapter list` with the exact item
/// text, after the Skill sections and before the working text. A decision request with the item
/// adds one paragraph after the Host instructions: the list has each Chapter in its order with
/// its number, its length in characters, and its title, and the Working text is the Chapter with
/// the mark current. The summary request never sends the paragraph.
#[tokio::test]
async fn the_chapter_list_is_one_exact_section_before_the_working_text_with_its_paragraph() {
    let list = "{\"volume\":\"Book <One>\"}\n[1,1204,\"The \\\"Harbor\\\"\"]\n\
                [2,37,\"Fog & rain\",\"current\"]";
    let catalog = "{\"description\":\"Dialogue rules.\",\"name\":\"dialogue\"}";
    let paragraph = "The section Chapter list has each Chapter of the manuscript in its order. A \
         line with a volume gives the title of a Volume before its Chapters. A Chapter line gives \
         the number of the Chapter, its length in characters, and its title. The Working text is \
         the Chapter with the mark current. When the list is long, its last line gives the count \
         of the Chapters and the ranges of numbers that the list does not show.";
    let base = v8(vec![
        item(
            ContextSourceClass::AuthorInstruction,
            "Tighten this paragraph.",
        ),
        item(ContextSourceClass::WorkingTarget, "The rain fell hard."),
    ]);
    let request = v8([
        base.context.clone(),
        vec![
            item(ContextSourceClass::SkillCatalog, catalog),
            item(ContextSourceClass::ChapterList, list),
        ],
    ]
    .concat());
    let instructions = |body: serde_json::Value| body["instructions"].as_str().unwrap().to_owned();
    let without = v8([
        base.context.clone(),
        vec![item(ContextSourceClass::SkillCatalog, catalog)],
    ]
    .concat());

    assert_eq!(
        body(request.clone()).await["input"],
        json!([{"role": "user", "content": [{"type": "input_text", "text": format!(
            "Author request:\nTighten this paragraph.\n\nSkill catalog:\n{catalog}\n\nChapter \
             list:\n{list}\n\nWorking text:\nThe rain fell hard.\n\nTarget blocks:\n\
             [block_id: 018f0000-0000-7001-8000-0000000000b1]\nThe rain fell hard on the old roof."
        )}]}])
    );
    assert_eq!(
        (
            instructions(body(request.clone()).await),
            instructions(body(closing(request.clone())).await),
            instructions(body(summary(request)).await),
        ),
        (
            format!(
                "{}\n\n{paragraph}",
                instructions(body(without.clone()).await)
            ),
            format!(
                "{}\n\n{paragraph}",
                instructions(body(closing(without.clone())).await)
            ),
            instructions(body(summary(without)).await),
        )
    );
}
