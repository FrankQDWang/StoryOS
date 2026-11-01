use super::CommandKind;
use crate::release1_operation_registry::{OperationKind, RELEASE1_OPERATIONS};

#[test]
fn the_command_kinds_are_the_command_operations_of_the_registry() {
    let mut registered = RELEASE1_OPERATIONS
        .iter()
        .flat_map(|artifacts| artifacts.operations)
        .filter(|registered| registered.kind == OperationKind::Command)
        .map(|registered| registered.operation.operation_id)
        .collect::<Vec<_>>();
    let mut declared = CommandKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>();
    registered.sort_unstable();
    declared.sort_unstable();
    assert_eq!(declared, registered);
}
