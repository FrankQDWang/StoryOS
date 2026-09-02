use super::ProjectActivityKind;

#[test]
fn an_unknown_persisted_kind_fails_closed() {
    assert_eq!(
        ProjectActivityKind::from_persisted("proposal_created"),
        None
    );
    assert_eq!(
        ProjectActivityKind::from_persisted("agent_run_created"),
        None
    );
}
