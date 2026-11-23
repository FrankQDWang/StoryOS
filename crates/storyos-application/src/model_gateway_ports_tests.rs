//! The Context items that a Create request sends, and its current input digest (ADR 0085,
//! ADR 0089 section 5).

use storyos_core::ContextSourceClass;

use super::{CreatePurpose, CurrentInput, RequestContextItem};

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

/// The purpose of a decision request of each Run kind. A capture with no kind of its input has
/// no purpose.
#[test]
fn each_run_kind_has_the_purpose_of_its_requests() {
    use storyos_core::{AgentRunKind, CaptureInputKind};
    let chapter = Some(CaptureInputKind::Chapter);
    assert_eq!(
        (
            AgentRunKind::ALL.map(|kind| CreatePurpose::of_run(kind, chapter)),
            CreatePurpose::of_run(AgentRunKind::MemoryCapture, None),
        ),
        (
            [
                Some(CreatePurpose::Decision),
                Some(CreatePurpose::MemoryCapture(CaptureInputKind::Chapter)),
                Some(CreatePurpose::MemoryConsolidation),
                Some(CreatePurpose::IntentPass),
                Some(CreatePurpose::Subrun)
            ],
            None
        )
    );
}

/// ADR 0102 section 3: only a request with a full input and no prior response sends the earlier
/// turns. A request that continues a prior response and a qualification probe do not.
#[test]
fn only_a_request_with_no_prior_response_and_no_probe_sends_the_earlier_turns() {
    let earlier = || {
        Some(storyos_core::EarlierTurns {
            turns: Vec::new(),
            older_turns_left_out: true,
        })
    };

    assert_eq!(
        [
            super::sent_earlier_turns(earlier(), None, /*probe*/ false),
            super::sent_earlier_turns(earlier(), Some("resp_1"), /*probe*/ false),
            super::sent_earlier_turns(earlier(), None, /*probe*/ true),
        ],
        [earlier(), None, None]
    );
}
