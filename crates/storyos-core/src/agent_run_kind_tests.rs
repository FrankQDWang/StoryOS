use super::AgentRunKind;

#[test]
fn each_kind_has_its_name_its_step_facts_and_a_consolidation_is_not_dispatched() {
    assert_eq!(
        (
            AgentRunKind::ALL.map(AgentRunKind::name),
            AgentRunKind::DISPATCHED.map(AgentRunKind::name),
            AgentRunKind::ALL.map(|kind| (kind.has_conversation(), kind.requires_tool_call())),
        ),
        (
            ["author", "memory_capture", "memory_consolidation"],
            ["author", "memory_capture"],
            [(true, false), (false, true), (false, true)]
        )
    );
}
