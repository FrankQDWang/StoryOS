use std::fs;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args([
            "-c",
            "user.name=guard",
            "-c",
            "user.email=guard@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(arguments)
        .status()
        .expect("git starts");
    assert!(status.success(), "git {arguments:?}");
}

#[test]
fn only_added_lines_of_changed_files_fail_the_guard() {
    let root = std::env::temp_dir().join(format!("storyos-literal-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("docs/agents")).expect("create the repository");
    fs::write(
        root.join("docs/agents/rust-literal-exemptions.json"),
        "{\"methods\": [\"enabled\"]}\n",
    )
    .expect("write");
    let call_site = "fn f() { open(true); }\n";
    fs::write(root.join("unchanged.rs"), call_site).expect("write");
    fs::write(root.join("changed.rs"), call_site).expect("write");
    git(&root, &["init", "--quiet", "--initial-branch=main"]);
    git(&root, &["add", "."]);
    git(
        &root,
        &["commit", "--quiet", "--no-verify", "--message=base"],
    );
    git(&root, &["tag", "base"]);
    fs::write(
        root.join("changed.rs"),
        format!("{call_site}fn g() {{ open(false); w.enabled(true); }}\n"),
    )
    .expect("write");
    fs::write(root.join("new.rs"), "fn h() { close(/*code*/ 0, None); }\n").expect("write");

    let output = Command::new(env!("CARGO_BIN_EXE_storyos-literal-guard"))
        .current_dir(&root)
        .arg("base")
        .output()
        .expect("the guard starts");
    fs::remove_dir_all(&root).expect("remove the repository");

    assert_eq!(
        (output.status.code(), String::from_utf8_lossy(&output.stdout).into_owned()),
        (
            Some(1),
            "changed.rs:2:15: positional-literal: argument `false` of `open` has no /*param*/ comment\n\
             new.rs:1:28: positional-literal: argument `None` of `close` has no /*param*/ comment\n"
                .to_owned()
        )
    );
}
