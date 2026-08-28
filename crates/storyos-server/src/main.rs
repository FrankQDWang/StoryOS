use std::collections::HashMap;
use std::env;
use std::io::{self, Write as _};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    let assets = storyos_server::WebAssetSet::load(web_root)?;
    let listener = TcpListener::bind(bind_address).await?;
    let address = listener.local_addr()?;
    let host = address.to_string();
    let session_users = env::var("STORYOS_BOOTSTRAP_SESSIONS")
        .ok()
        .map(|value| serde_json::from_str::<HashMap<String, String>>(&value))
        .transpose()?
        .unwrap_or_default();
    let issued_at_unix_seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let expires_at_unix_seconds = issued_at_unix_seconds
        .checked_add(Duration::from_secs(8 * 60 * 60).as_secs())
        .ok_or("Client Session Binding lifetime overflow")?;
    let current_session_generation = 1;
    let client_contract_revision = storyos_contracts::release1_protocol_profile()
        .release_identity
        .web_client_contract_revision;
    let security_policy_revision = storyos_server::RELEASE_1_SECURITY_POLICY_REVISION.to_owned();
    let allowed_origin = format!("http://{host}");
    let session_bindings = session_users
        .into_iter()
        .map(|(handle, owner_user_id)| {
            Uuid::parse_str(&owner_user_id)?;
            let binding = storyos_server::ClientSessionBinding {
                owner_user_id: UserId::new(owner_user_id),
                allowed_host: host.clone(),
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
        database_url: env::var("STORYOS_DATABASE_URL").ok(),
        session_bindings,
        current_session_generation,
        accepted_client_contract_revision: Some(client_contract_revision),
        accepted_security_policy_revision: Some(security_policy_revision),
        allowed_host: Some(host.clone()),
        allowed_origin: Some(allowed_origin),
        project_command_challenge_secret: env::var("STORYOS_CHALLENGE_SECRET")
            .ok()
            .map(String::into_bytes),
    };
    println!("STORYOS_SERVER_URL=http://{address}");
    io::stdout().flush()?;
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
