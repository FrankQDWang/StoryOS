use std::collections::HashMap;
use std::env;
use std::io::{self, Write as _};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use storyos_adapter_postgres::{PostgresProjectReader, require_release1_storage_activation_proof};
use storyos_application::UserId;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    if let [flag, root] = arguments.as_slice()
        && flag == "--check-web-root"
    {
        storyos_server::WebAssetSet::load(Path::new(root))?;
        return Ok(());
    }
    let (bind_address, web_root) = server_options(&arguments)?;
    let transport = storyos_server::packaged_transport_plan(
        env::var("STORYOS_PUBLIC_ORIGIN").ok().as_deref(),
        bind_address,
    )
    .map_err(|error| error.to_string())?;
    let multiple_mapping_allowance =
        if env::var(storyos_server::TEST_ALLOW_MULTIPLE_BOOTSTRAP_SESSIONS).as_deref() == Ok("1") {
            storyos_server::MultipleMappingAllowance::IsolationTestsDisableIssuance
        } else {
            storyos_server::MultipleMappingAllowance::Refuse
        };
    let (session_users, trusted_local_session_bootstrap) =
        storyos_server::packaged_session_mappings(
            env::var("STORYOS_BOOTSTRAP_SESSIONS").ok().as_deref(),
            multiple_mapping_allowance,
        )?;
    let assets = storyos_server::WebAssetSet::load(web_root)?;
    let database_url = env::var("STORYOS_DATABASE_URL").map_err(|_| {
        "STORYOS_DATABASE_URL is required for Release 1 Storage Activation".to_owned()
    })?;
    require_release1_storage_activation_proof(&database_url).await?;
    let listener = TcpListener::bind(bind_address).await?;
    let address = listener.local_addr()?;
    let (allowed_host, allowed_origin, printed_server_url) = match &transport {
        storyos_server::PackagedTransportPlan::LocalHttp => {
            let host = address.to_string();
            (
                host.clone(),
                format!("http://{host}"),
                format!("http://{address}"),
            )
        }
        storyos_server::PackagedTransportPlan::PublicHttps(public) => (
            public.allowed_host.clone(),
            public.allowed_origin.clone(),
            public.allowed_origin.clone(),
        ),
    };
    let issued_at_unix_seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let expires_at_unix_seconds = issued_at_unix_seconds
        .checked_add(storyos_server::CLIENT_SESSION_BINDING_LIFETIME_SECS)
        .ok_or("Client Session Binding lifetime overflow")?;
    let current_session_generation = 1;
    let client_contract_revision = storyos_contracts::release1_protocol_profile()
        .release_identity
        .web_client_contract_revision;
    let security_policy_revision = storyos_server::RELEASE_1_SECURITY_POLICY_REVISION.to_owned();
    let session_bindings = session_users
        .into_iter()
        .map(|(handle, owner_user_id)| {
            Uuid::parse_str(&owner_user_id)?;
            let binding = storyos_server::ClientSessionBinding {
                owner_user_id: UserId::new(owner_user_id),
                allowed_host: allowed_host.clone(),
                allowed_origin: allowed_origin.clone(),
                session_generation: current_session_generation,
                issued_at_unix_seconds,
                expires_at_unix_seconds,
                client_contract_revision: client_contract_revision.clone(),
                security_policy_revision: security_policy_revision.clone(),
            };
            Ok((handle, binding))
        })
        .collect::<Result<HashMap<_, _>, uuid::Error>>()?;
    let config = storyos_server::ServerConfig {
        database_url: Some(database_url),
        session_bindings,
        current_session_generation,
        accepted_client_contract_revision: Some(client_contract_revision),
        accepted_security_policy_revision: Some(security_policy_revision),
        allowed_host: Some(allowed_host),
        allowed_origin: Some(allowed_origin),
        project_command_challenge_secret: env::var("STORYOS_CHALLENGE_SECRET")
            .ok()
            .map(String::into_bytes),
        trusted_local_session_bootstrap,
        session_cookie_secure: match transport {
            storyos_server::PackagedTransportPlan::LocalHttp => {
                storyos_server::SessionCookieSecure::Omit
            }
            storyos_server::PackagedTransportPlan::PublicHttps(_) => {
                storyos_server::SessionCookieSecure::Include
            }
        },
    };
    println!("STORYOS_SERVER_URL={printed_server_url}");
    io::stdout().flush()?;
    if storyos_worker::in_process_loop_enabled()
        && let Some(database_url) = config.database_url.clone()
    {
        let store = PostgresProjectReader::new(database_url)
            .with_readable_export_lease_ttl(storyos_worker::readable_export_lease_ttl_from_env());
        let _worker = tokio::spawn(storyos_worker::run(store));
    }
    axum::serve(listener, storyos_server::router_with_web(config, assets)).await?;
    Ok(())
}

fn server_options(arguments: &[String]) -> Result<(&str, &Path), String> {
    match arguments {
        [flag, root] if flag == "--web-root" => Ok(("127.0.0.1:3000", Path::new(root))),
        [bind_flag, address, root_flag, root] if bind_flag == "--bind" && root_flag == "--web-root" => {
            Ok((address, Path::new(root)))
        }
        [root_flag, root, bind_flag, address] if root_flag == "--web-root" && bind_flag == "--bind" => {
            Ok((address, Path::new(root)))
        }
        _ => Err(
            "usage: storyos-server --web-root <directory> [--bind <address>] | --check-web-root <directory>".to_owned(),
        ),
    }
}
