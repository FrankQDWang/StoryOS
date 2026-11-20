//! The shared test support from `agent_run_lease_tests.rs`.

use std::time::Duration;
use storyos_application::GatewayClock;
use tokio::sync::watch;

/// A Deterministic Test Scheduler. A renewal waits until the test advances one step.
pub(crate) struct SteppedClock {
    steps: watch::Sender<u64>,
    waits: watch::Sender<u64>,
}

impl SteppedClock {
    pub(crate) fn new() -> Self {
        Self {
            steps: watch::Sender::new(/*init*/ 0),
            waits: watch::Sender::new(/*init*/ 0),
        }
    }

    pub(crate) fn step(&self) {
        self.steps.send_modify(|steps| *steps += 1);
    }

    /// Completes when the renewal waits for the `count`-th time, after its renewal committed.
    pub(crate) async fn waited(&self, count: u64) {
        let mut waits = self.waits.subscribe();
        tokio::time::timeout(
            Duration::from_secs(/*secs*/ 10),
            waits.wait_for(|waits| *waits >= count),
        )
        .await
        .expect("the Worker renews the lease while the request is held")
        .unwrap();
    }
}

impl GatewayClock for SteppedClock {
    async fn wait(&self, _delay: Duration) {
        let mut steps = self.steps.subscribe();
        self.waits.send_modify(|waits| *waits += 1);
        let _stepped = steps.changed().await;
    }
}
