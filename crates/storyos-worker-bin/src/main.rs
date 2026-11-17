//! The StoryOS Worker binary: the Worker loop with the PostgreSQL store, the fake and Agent Plan
//! adapters, and the credential resolver of the Worker settings. `--discover-exa` runs one
//! `Discover` exchange of the MCP client adapter. `--rebuild-passage-index` marks the passage index
//! work of each Chapter with a work row, and sends no request (ADR 0095 section 4).

mod contract_fault_holds;
mod credential;
mod discover_tools;
mod registered_adapters;
mod registered_embeddings;
mod settings;

use std::env;

use secrecy::ExposeSecret as _;
use storyos_adapter_diagnostics::or_exit;
use storyos_adapter_fake_destination::FakeDestination;
use storyos_adapter_postgres::{PostgresProjectReader, require_release1_storage_activation_proof};
use storyos_adapter_volcengine_responses::{AgentPlanEmbeddings, AgentPlanResponses};
use storyos_worker::{ModelDestination, WorkerStore};

/// The bound of one complete Tool exchange: the session, the drift check, and the call.
const TOOL_EXCHANGE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(/*secs*/ 60);

type WorkerAdapters = registered_adapters::RegisteredAdapters<credential::SettingsCredentials>;
type WorkerEmbeddings =
    registered_embeddings::RegisteredEmbeddings<credential::SettingsCredentials>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let storyos_settings::Loaded { settings, env_file } = settings::WorkerSettings::load()?;
    storyos_adapter_diagnostics::install(settings.log.as_deref())?;
    storyos_application::register_sql_state(storyos_adapter_postgres::sql_state);
    let arguments = env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    if arguments.iter().any(|argument| argument == "--check") {
        return Ok(());
    }
    if arguments
        .iter()
        .any(|argument| argument == "--discover-exa")
    {
        let resolver = credential::SettingsCredentials {
            agent_plan_api_key: None,
            exa_api_key: settings.exa_api_key,
        };
        std::process::exit(discover_tools::discover_exa(resolver).await);
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
    let store = PostgresProjectReader::new(database_url)
        .with_readable_export_lease_ttl(storyos_worker::readable_export_lease_ttl(
            settings.export_lease_ttl_secs,
        ))
        .with_embedding_registration(settings.embedding_registration());
    if let Some((result, stage)) = one_shot(&arguments, &store).await {
        or_exit(result, stage);
        std::process::exit(0);
    }
    let destination = ModelDestination {
        adapter: WorkerAdapters {
            fake: FakeDestination,
            agent_plan: AgentPlanResponses {
                resolver: credential::SettingsCredentials {
                    agent_plan_api_key: settings.agent_plan_api_key.clone(),
                    exa_api_key: None,
                },
            },
        },
        observer: contract_fault_holds::ContractFaultHolds::from_env(),
        tools: storyos_adapter_mcp_client::McpClient {
            resolver: credential::SettingsCredentials {
                agent_plan_api_key: None,
                exa_api_key: settings.exa_api_key,
            },
            timeout: TOOL_EXCHANGE_TIMEOUT,
        },
        embeddings: WorkerEmbeddings {
            fake: storyos_adapter_fake_destination::FakeEmbeddings,
            agent_plan: AgentPlanEmbeddings {
                resolver: credential::SettingsCredentials {
                    agent_plan_api_key: settings.agent_plan_api_key,
                    exa_api_key: None,
                },
            },
        },
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

/// The result of a one-shot command, and the stage name of its failure.
type OneShot = (
    Result<(), Box<dyn std::error::Error + Send + Sync>>,
    &'static str,
);

/// Runs the one-shot command of `arguments` with `store`, or gives `None` for the loop.
/// `--claim-only` claims one unit of work with the adapters of the binary.
/// `--rebuild-passage-index` marks the passage index work of each Chapter with a work row, and it
/// sends no request.
async fn one_shot(arguments: &[String], store: &impl WorkerStore) -> Option<OneShot> {
    if arguments.iter().any(|argument| argument == "--claim-only") {
        let claimed = storyos_worker::claim_only::<WorkerAdapters, WorkerEmbeddings>(store).await;
        return Some((claimed, "claim_only"));
    }
    if arguments
        .iter()
        .any(|argument| argument == "--rebuild-passage-index")
    {
        let marked = store.mark_each_passage_index_work().await;
        return Some((
            marked.map(|_| ()).map_err(Into::into),
            "rebuild_passage_index",
        ));
    }
    None
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
