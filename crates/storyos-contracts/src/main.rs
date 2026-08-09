use std::env;

use storyos_contracts::{
    check_crosswalk, check_release1_artifacts, repository_root, write_crosswalk,
    write_release1_artifacts,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = repository_root();
    match env::args().nth(1).as_deref() {
        Some("generate") => {
            write_crosswalk(repo_root)?;
            write_release1_artifacts(repo_root)?;
        }
        Some("check") => {
            check_crosswalk(repo_root)?;
            check_release1_artifacts(repo_root)?;
        }
        command => {
            return Err(format!("expected `generate` or `check`, received {command:?}").into());
        }
    }
    Ok(())
}
