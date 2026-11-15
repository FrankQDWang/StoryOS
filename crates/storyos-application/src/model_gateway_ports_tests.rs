//! The Context items that a Create request sends, and its current input digest (ADR 0085,
//! ADR 0089 section 5).

use storyos_core::ContextSourceClass;

use super::{CurrentInput, RequestContextItem};

fn item(
    source_class: ContextSourceClass,
    content: &str,
    full_input_only: bool,
) -> RequestContextItem {
    RequestContextItem {
        source_class,
        content: content.to_owned(),
        full_input_only,
    }
}

fn digest(context: &[RequestContextItem]) -> String {
    CurrentInput {
        author_message: "Help again.",
        chapter_id: "chapter-1",
        passage_resolution: None,
        passage_input: None,
        context,
        declared_targets: &[],
        selection_references: &[],
    }
    .digest()
}

#[test]
fn only_a_full_input_sends_a_skill_of_the_conversation_and_no_digest_counts_it() {
    let own = item(ContextSourceClass::SkillInstruction, "Own Skill.", false);
    let conversation = item(ContextSourceClass::SkillInstruction, "Earlier Skill.", true);
    let selected = vec![own.clone(), conversation.clone()];

    assert_eq!(
        (
            RequestContextItem::sent(selected.clone(), None),
            RequestContextItem::sent(selected.clone(), Some("resp_1")),
            digest(&selected) == digest(std::slice::from_ref(&own)),
        ),
        (selected, vec![own], true)
    );
}
