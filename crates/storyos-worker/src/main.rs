use std::env;

use storyos_adapter_postgres::PostgresProjectReader;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let arguments = env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    if arguments.iter().any(|argument| argument == "--check") {
        return Ok(());
    }
    let database_url = env::var("STORYOS_DATABASE_URL")?;
    let store = PostgresProjectReader::new(database_url)
        .with_readable_export_lease_ttl(storyos_worker::readable_export_lease_ttl_from_env());
    if arguments.iter().any(|argument| argument == "--claim-only") {
        storyos_worker::claim_only(&store).await?;
        std::process::exit(0);
    }
    if arguments.iter().any(|argument| argument == "--once") {
        storyos_worker::run_once(&store).await?;
        std::process::exit(0);
    }
    storyos_worker::run(store).await;
    Ok(())
}
