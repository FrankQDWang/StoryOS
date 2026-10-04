use std::path::PathBuf;
use std::time::Duration;

use storyos_application::{ContractFaultObserver, ContractFaultPoint};

/// Holds the Worker process at a Contract Fault Point while the named test hold file exists.
pub(crate) struct ContractFaultHolds {
    dispatch: Option<PathBuf>,
    stream: Option<PathBuf>,
}

impl ContractFaultHolds {
    pub(crate) fn from_env() -> Self {
        Self {
            dispatch: std::env::var_os("STORYOS_TEST_FAKE_DISPATCH_HOLD_PATH").map(PathBuf::from),
            stream: std::env::var_os("STORYOS_TEST_FAKE_STREAM_HOLD_PATH").map(PathBuf::from),
        }
    }
}

impl ContractFaultObserver for ContractFaultHolds {
    async fn reached(&self, point: ContractFaultPoint) {
        let hold = match point {
            ContractFaultPoint::DispatchClaimed => &self.dispatch,
            ContractFaultPoint::StreamCommitted => &self.stream,
        };
        let Some(path) = hold else {
            return;
        };
        while path.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}
