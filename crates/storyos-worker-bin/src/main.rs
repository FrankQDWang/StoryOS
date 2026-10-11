//! The StoryOS Worker binary: the Worker loop with the PostgreSQL store, the fake and Agent Plan
//! adapters, and the credential resolver of the Worker settings.

mod contract_fault_holds;
mod credential;
mod registered_adapters;
mod settings;

use std::env;

use secrecy::ExposeSecret as _;
use storyos_adapter_diagnostics::or_exit;
use storyos_adapter_fake_destination::FakeDestination;
use storyos_adapter_postgres::{PostgresProjectReader, require_release1_storage_activation_proof};
use storyos_adapter_volcengine_responses::AgentPlanResponses;
use storyos_worker::ModelDestination;

type WorkerAdapters = registered_adapters::RegisteredAdapters<credential::SettingsCredentials>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let storyos_settings::Loaded { settings, env_file } = settings::WorkerSettings::load()?;
    storyos_adapter_diagnostics::install(settings.log.as_deref())?;
    storyos_application::register_sql_state(storyos_adapter_postgres::sql_state);
    let arguments = env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    if arguments.iter().any(|argument| argument == "--check") {
        return Ok(());
    }
    let database_url = settings
        .database_url
        .as_ref()
        .ok_or_else(|| {
            storyos_settings::SettingsError::new("STORYOS_DATABASE_URL", env_file.as_deref())
        })?
        .expose_secret();
    or_exit(
        require_release1_storage_activation_proof(database_url).await,
        "storage_activation",
    );
    let store = PostgresProjectReader::new(database_url).with_readable_export_lease_ttl(
        storyos_worker::readable_export_lease_ttl(settings.export_lease_ttl_secs),
    );
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
                resolver: credential::SettingsCredentials {
                    agent_plan_api_key: settings.agent_plan_api_key,
                },
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
