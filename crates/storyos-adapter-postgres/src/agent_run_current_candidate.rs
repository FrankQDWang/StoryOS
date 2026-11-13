//! The current revision of each pending candidate that a Run references. When the author
//! changes such a candidate during the Run, the next RunStep reads its current revision (ADR 0085).

#[cfg(test)]
#[path = "agent_run_current_candidate_tests.rs"]
mod tests;
