use super::AgentRunKind;

#[test]
fn each_kind_has_its_persisted_name_and_only_the_author_kind_is_dispatched() {
    assert_eq!(
        (
            AgentRunKind::ALL.map(AgentRunKind::name),
            AgentRunKind::DISPATCHED.map(AgentRunKind::name),
        ),
        (
            ["author", "memory_capture", "memory_consolidation"],
            ["author"]
        )
    );
}
