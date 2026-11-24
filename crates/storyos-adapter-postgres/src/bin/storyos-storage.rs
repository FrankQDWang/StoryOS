use std::env;
use std::process::ExitCode;

use storyos_adapter_postgres::{StorageActivationError, activate_release1_storage};

#[tokio::main]
async fn main() -> ExitCode {
    match env::var("STORYOS_STORAGE_ADMIN_URL") {
        Ok(admin_url) => match activate_release1_storage(&admin_url).await {
            Ok(activation) => {
                println!("{activation}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        Err(_) => {
            eprintln!("{}", StorageActivationError::AdminUrlMissing);
            ExitCode::from(1)
        }
    }
}
