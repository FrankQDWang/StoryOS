use std::env;
use std::path::Path;

use storyos_contracts::{
    check_crosswalk, check_release1_artifacts, check_stage2_crosswalk, repository_root,
    write_crosswalk, write_release1_artifacts, write_stage2_crosswalk, write_web_asset_manifest,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = repository_root();
    let arguments = env::args().skip(/*n*/ 1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [command] if command == "generate" => {
            write_crosswalk(repo_root)?;
            write_stage2_crosswalk(repo_root)?;
            write_release1_artifacts(repo_root)?;
        }
        [command] if command == "check" => {
            check_crosswalk(repo_root)?;
            check_stage2_crosswalk(repo_root)?;
            check_release1_artifacts(repo_root)?;
        }
        [command, root, commit, tree] if command == "web-manifest" => {
            println!(
                "{}",
                write_web_asset_manifest(Path::new(root), commit, tree)?
            );
        }
        _ => {
            return Err(
                "usage: storyos-contracts generate|check|web-manifest <root> <commit> <tree>"
                    .into(),
            );
        }
    }
    Ok(())
}
