use std::error::Error;
use std::fmt;

use super::{DiagnosticField, SqlState, register_sql_state};

#[derive(Debug)]
struct StoreError;

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("store error")
    }
}

impl Error for StoreError {}

#[derive(Debug)]
struct Wrapper(StoreError);

impl fmt::Display for Wrapper {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("wrapper")
    }
}

impl Error for Wrapper {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

fn store_code(error: &(dyn Error + 'static)) -> Option<String> {
    error
        .downcast_ref::<StoreError>()
        .map(|_| "40001".to_owned())
}

#[test]
fn sql_state_reads_the_first_code_in_the_source_chain() {
    register_sql_state(store_code);
    assert_eq!(
        SqlState(&Wrapper(StoreError)).diagnostic(),
        Some("40001".to_owned())
    );
    assert_eq!(SqlState(&std::fmt::Error).diagnostic(), None);
}
