//! The shared test support from `agent_run_read_memory_entry_tests.rs`.

use serde_json::{Value, json};

pub(crate) const NOT_FOUND: &str = "The Host refused this call because no current entry has this id: use an \
id of the section Memory index, so no result is available, and this is not an empty result.";

pub(crate) fn refused() -> Value {
    json!([
        "read_memory_entry",
        "refused",
        "memory_entry_not_found",
        NOT_FOUND,
        true
    ])
}
