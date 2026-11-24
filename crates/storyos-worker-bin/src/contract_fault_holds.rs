use std::env::VarError;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use storyos_application::{ContractFaultObserver, ContractFaultPoint};

/// Holds the Worker process at a Contract Fault Point while the named test hold file exists. The
/// dispatch hold applies from the `dispatch_from`-th `DispatchClaimed` of the process.
pub(crate) struct ContractFaultHolds {
    dispatch: Option<PathBuf>,
    dispatch_from: NonZeroUsize,
    dispatches: AtomicUsize,
    stream: Option<PathBuf>,
    compaction: Option<PathBuf>,
}

impl ContractFaultHolds {
    /// The holds of the test variables. `STORYOS_TEST_FAKE_DISPATCH_HOLD_FROM` is 1 when it is
    /// absent, and a value that is not a positive integer is an error.
    pub(crate) fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let dispatch_from = match std::env::var("STORYOS_TEST_FAKE_DISPATCH_HOLD_FROM") {
            Err(VarError::NotPresent) => NonZeroUsize::MIN,
            from => from?.parse()?,
        };
        Ok(Self {
            dispatch: std::env::var_os("STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH").map(PathBuf::from),
            dispatch_from,
            dispatches: AtomicUsize::new(0),
            stream: std::env::var_os("STORYOS_TEST_FAKE_STREAM_HOLD_PATH").map(PathBuf::from),
            compaction: std::env::var_os("STORYOS_TEST_FAKE_COMPACTION_STAGE_HOLD_PATH")
                .map(PathBuf::from),
        })
    }
}

impl ContractFaultObserver for ContractFaultHolds {
    async fn reached(&self, point: ContractFaultPoint) {
        let hold = match point {
            ContractFaultPoint::DispatchClaimed => {
                let reached = self.dispatches.fetch_add(1, Ordering::Relaxed) + 1;
                if reached < self.dispatch_from.get() {
                    return;
                }
                &self.dispatch
            }
            ContractFaultPoint::StreamCommitted => &self.stream,
            ContractFaultPoint::CompactionStaged => &self.compaction,
            ContractFaultPoint::ToolCallsCommitted
            | ContractFaultPoint::ToolContextCommitted
            | ContractFaultPoint::ToolDispatchClaimed => {
                return;
            }
        };
        let Some(path) = hold else {
            return;
        };
        while path.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}
