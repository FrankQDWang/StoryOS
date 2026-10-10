//! The StoryOS Worker binary: the Worker loop with the PostgreSQL store, the fake and Agent Plan
//! adapters, and the Keychain credential resolver.

mod contract_fault_holds;
mod keychain;
mod registered_adapters;

use std::env;

use storyos_adapter_diagnostics::or_exit;
use storyos_adapter_fake_destination::FakeDestination;
use storyos_adapter_postgres::{PostgresProjectReader, require_release1_storage_activation_proof};
use storyos_adapter_volcengine_responses::AgentPlanResponses;
use storyos_worker::ModelDestination;

type WorkerAdapters = registered_adapters::RegisteredAdapters<keychain::KeychainResolver>;

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
    or_exit(
        require_release1_storage_activation_proof(&database_url).await,
        "storage_activation",
    );
    let store = PostgresProjectReader::new(database_url)
        .with_readable_export_lease_ttl(storyos_worker::readable_export_lease_ttl_from_env());
    if arguments.iter().any(|argument| argument == "--claim-only") {
        or_exit(
            storyos_worker::claim_only::<WorkerAdapters>(&store).await,
            "claim_only",
        );
        std::process::exit(0);
    }
    let destination = ModelDestination {
        adapter: WorkerAdapters {
            fake: FakeDestination,
            agent_plan: AgentPlanResponses {
                resolver: keychain::KeychainResolver,
            },
        },
        observer: contract_fault_holds::ContractFaultHolds::from_env(),
    };
    if arguments.iter().any(|argument| argument == "--once") {
        or_exit(
            storyos_worker::run_once(&store, &destination).await,
            "run_once",
        );
        std::process::exit(0);
    }
    storyos_worker::run(store, destination).await;
    Ok(())
}
