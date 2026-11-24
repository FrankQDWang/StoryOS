//! A new PostgreSQL container of the controlled image for a test that needs its own cluster.

use std::process::Command;
use std::time::Duration;

/// A running container whose admin login is `postgres` with the password `admin`; drop removes it.
pub(crate) struct PostgresContainer {
    pub(crate) name: String,
}

impl PostgresContainer {
    pub(crate) fn start(name_prefix: &str) -> Self {
        let name = format!("{name_prefix}-{}", uuid::Uuid::now_v7().simple());
        let mut run = Command::new("docker");
        run.args(["run", "--detach", "--name", &name]);
        if let Ok(check_run) = std::env::var("STORYOS_CHECK_RUN") {
            run.args(["--label", &format!("storyos.check-run={check_run}")]);
        }
        run.args([
            "--env",
            "POSTGRES_PASSWORD=admin",
            "--publish",
            "127.0.0.1::5432",
            &controlled_postgres_image(),
        ]);
        let container = Self { name };
        succeed(&mut run);
        container.wait_until_ready();
        container
    }

    fn wait_until_ready(&self) {
        for _attempt in 0..120 {
            let logs = Command::new("docker")
                .args(["logs", &self.name])
                .output()
                .unwrap();
            let initialized = String::from_utf8_lossy(&logs.stdout)
                .contains("PostgreSQL init process complete")
                || String::from_utf8_lossy(&logs.stderr)
                    .contains("PostgreSQL init process complete");
            let ready = initialized
                && Command::new("docker")
                    .args(["exec", &self.name, "pg_isready", "-U", "postgres"])
                    .output()
                    .unwrap()
                    .status
                    .success();
            if ready {
                return;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        panic!("PostgreSQL did not become ready: {}", self.name);
    }

    /// The URL of the admin login, in the form of `postgres_admin_url` of
    /// `scripts/lib/controlled-postgres.sh`.
    pub(crate) fn admin_url(&self) -> String {
        let published = succeed(Command::new("docker").args(["port", &self.name, "5432/tcp"]));
        let published = String::from_utf8(published).unwrap();
        let port = published.trim().rsplit_once(':').unwrap().1;
        format!("postgres://postgres:admin@127.0.0.1:{port}/postgres")
    }
}

impl Drop for PostgresContainer {
    fn drop(&mut self) {
        let _removed = Command::new("docker")
            .args(["rm", "--force", "--volumes", &self.name])
            .output();
    }
}

fn controlled_postgres_image() -> String {
    include_str!("../../../../scripts/lib/controlled-postgres.sh")
        .lines()
        .find_map(|line| line.strip_prefix("storyos_postgres_image="))
        .expect("scripts/lib/controlled-postgres.sh names the PostgreSQL image")
        .to_owned()
}

pub(crate) fn succeed(command: &mut Command) -> Vec<u8> {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
