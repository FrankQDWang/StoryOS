use std::env;

use storyos_contracts::{check_crosswalk, repository_root, write_crosswalk};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = repository_root();
    match env::args().nth(1).as_deref() {
        Some("generate") => write_crosswalk(repo_root)?,
        Some("check") => check_crosswalk(repo_root)?,
        command => {
            return Err(format!("expected `generate` or `check`, received {command:?}").into());
        }
    }
    Ok(())
}
