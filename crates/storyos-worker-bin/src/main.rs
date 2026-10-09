//! The StoryOS Worker binary: the Worker loop with the PostgreSQL store and the fake adapter.

mod contract_fault_holds;

use std::env;

use storyos_adapter_fake_destination::FakeDestination;
use storyos_adapter_postgres::{PostgresProjectReader, require_release1_storage_activation_proof};
use storyos_worker::ModelDestination;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    storyos_adapter_diagnostics::install(
        env::var(storyos_adapter_diagnostics::LOG_LEVEL_VARIABLE)
            .ok()
            .as_deref(),
    )?;
    storyos_application::register_sql_state(storyos_adapter_postgres::sql_state);
    let arguments = env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    if arguments.iter().any(|argument| argument == "--check") {
        return Ok(());
    }
    let database_url = env::var("STORYOS_DATABASE_URL")?;
    require_release1_storage_activation_proof(&database_url).await?;
    let store = PostgresProjectReader::new(database_url)
        .with_readable_export_lease_ttl(storyos_worker::readable_export_lease_ttl_from_env());
    if arguments.iter().any(|argument| argument == "--claim-only") {
        storyos_worker::claim_only(&store).await?;
        std::process::exit(0);
    }
    let destination = ModelDestination {
        adapter: FakeDestination,
        observer: contract_fault_holds::ContractFaultHolds::from_env(),
    };
    if arguments.iter().any(|argument| argument == "--once") {
        storyos_worker::run_once(&store, &destination).await?;
        std::process::exit(0);
    }
    storyos_worker::run(store, destination).await;
    Ok(())
}
