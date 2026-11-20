//! The shared test support from `agent_run_tool_recovery_tests.rs`.

use crate::PostgresProjectReader;
use crate::shared_tests::command_sequence_support::CommandCall;
use std::sync::Mutex;
use storyos_application::{CancelAgentRunInput, ContractFaultObserver, ContractFaultPoint};
use tokio_postgres::Client;

pub(crate) enum Fault<'a> {
    /// The author cancels the Run.
    Cancel {
        store: &'a PostgresProjectReader,
        call: Box<CommandCall<CancelAgentRunInput>>,
    },
    /// Another Worker takes the Run over, as after a crash of this Worker.
    TakeOver { admin: &'a Client, run_id: String },
}

/// The Contract Fault Point observer: it injects `fault` when the Tool Gateway reaches `point`
/// for the `nth` time.
pub(crate) struct FaultAt<'a> {
    point: ContractFaultPoint,
    nth: usize,
    fault: Fault<'a>,
    reached: Mutex<usize>,
}

impl<'a> FaultAt<'a> {
    pub(crate) fn new(point: ContractFaultPoint, nth: usize, fault: Fault<'a>) -> Self {
        Self {
            point,
            nth,
            fault,
            reached: Mutex::new(/*value*/ 0),
        }
    }
}

impl ContractFaultObserver for FaultAt<'_> {
    async fn reached(&self, point: ContractFaultPoint) {
        let count = {
            let mut reached = self.reached.lock().unwrap();
            *reached += usize::from(point == self.point);
            *reached
        };
        if point != self.point || count != self.nth {
            return;
        }
        match &self.fault {
            Fault::Cancel { store, call } => {
                store
                    .cancel_agent_run(&call.envelope, &call.input)
                    .await
                    .unwrap();
            }
            Fault::TakeOver { admin, run_id } => {
                admin
                    .execute(
                        "UPDATE storyos.agent_runs SET fence_token = fence_token + 1
                          WHERE run_id = $1::text::uuid",
                        &[run_id],
                    )
                    .await
                    .unwrap();
            }
        }
    }
}
