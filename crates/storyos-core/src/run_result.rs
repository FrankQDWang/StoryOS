//! The order of the results of a Run (ADR 0091 section 6). Each result of a call goes at the
//! position of its call: the RunStep position, then the position of the call in its response. The
//! final answer of a completed Run is the last result. No result replaces another.

/// The place of one call in its Run.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RunCallPosition {
    pub decision_position: i64,
    pub call_position: i32,
}

/// The results of a Run in the order in which the model made them: the result of each call in the
/// order of its position, then `answer`.
pub fn run_results<T>(mut calls: Vec<(RunCallPosition, T)>, answer: Option<T>) -> Vec<T> {
    calls.sort_by_key(|(position, _)| *position);
    calls
        .into_iter()
        .map(|(_, result)| result)
        .chain(answer)
        .collect()
}

#[cfg(test)]
#[path = "run_result_tests.rs"]
mod tests;
