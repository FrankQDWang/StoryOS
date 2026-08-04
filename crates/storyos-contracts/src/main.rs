use std::env;
use std::path::Path;

use storyos_contracts::{check_crosswalk, write_crosswalk};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("contracts crate must be nested under the repository root")?;
    match env::args().nth(1).as_deref() {
        Some("generate") => write_crosswalk(repo_root)?,
        Some("check") => check_crosswalk(repo_root)?,
        command => {
            return Err(format!("expected `generate` or `check`, received {command:?}").into());
        }
    }
    Ok(())
}
