//! The shared test support from `memory_work_outcome_tests.rs`.

use tokio_postgres::Client;

/// Runs `claim` while a lock holds the start of a capture after the claim of a row, and ends the
/// session of the claim at that point. This is the stop of the Worker. Returns the result of
/// `claim`.
pub(crate) async fn stopped_at_the_start<T>(
    admin: &Client,
    claim: impl std::future::Future<Output = T>,
) -> T {
    // The lock holds the start of the capture after the claim of the row. The end of the session
    // of the claim is the stop of the Worker.
    admin
        .batch_execute("BEGIN; LOCK TABLE storyos.memory_captures IN SHARE ROW EXCLUSIVE MODE")
        .await
        .unwrap();
    let stop = async {
        // A blocked session gives no signal, so this is the one wait with a sleep. The lock goes
        // only after the session of the claim ended.
        let mut stopped: Option<i32> = None;
        for _ in 0..1_000 {
            let session: Option<i32> = match stopped {
                None => admin
                    .query_opt(
                        "SELECT waiting.pid
                           FROM (SELECT pid FROM pg_locks
                                  WHERE NOT granted
                                    AND relation = 'storyos.memory_captures'::regclass
                                 OFFSET 0) AS waiting
                          WHERE pg_terminate_backend(waiting.pid)",
                        &[],
                    )
                    .await
                    .unwrap(),
                Some(pid) => admin
                    .query_opt("SELECT pid FROM pg_stat_activity WHERE pid = $1", &[&pid])
                    .await
                    .unwrap(),
            }
            .map(|row| row.get(/*idx*/ 0));
            match (stopped, session) {
                (None, Some(pid)) => stopped = Some(pid),
                (Some(_), None) => break,
                _ => tokio::time::sleep(std::time::Duration::from_millis(/*millis*/ 10)).await,
            }
        }
        admin.batch_execute("ROLLBACK").await.unwrap();
    };
    let (claimed, ()) = tokio::join!(claim, stop);
    claimed
}
