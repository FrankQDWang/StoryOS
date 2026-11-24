use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use rustls::CertificateError;
use tokio_postgres::NoTls;

use crate::connection_pool::ConnectionPool;
use crate::shared_tests::author_edit::AUTHOR_EDIT_TEST_LOCK;

const CHILD_ADMIN_URL: &str = "STORYOS_TEST_TLS_ADMIN_URL";
const CERTIFICATES: &str = "/var/lib/postgresql/tls";
// Each server certificate is for `localhost`, except `other-host`. Only `trusted-root` is in SSL_CERT_FILE.
const MAKE_CERTIFICATES: &str = r#"
mkdir -p /var/lib/postgresql/tls && cd /var/lib/postgresql/tls
key="-newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes"
for ca in trusted other; do
  openssl req -x509 $key -days 2 -subj "/CN=storyos-test-$ca-ca" -keyout $ca-root.key -out $ca-root.crt \
    -addext basicConstraints=critical,CA:TRUE -addext keyUsage=critical,keyCertSign
done
leaf() {
  openssl req $key -subj "/CN=$3" -keyout $1.key -out $1.csr
  printf 'basicConstraints=CA:FALSE\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:%s\n' "$3" > $1.ext
  openssl x509 -req -in $1.csr -CA $2-root.crt -CAkey $2-root.key -CAcreateserial -days 2 -extfile $1.ext -out $1.crt
}
leaf trusted trusted localhost
leaf other-ca other localhost
leaf other-host trusted storyos-other-host.invalid
chmod 600 *.key
"#;

#[tokio::test]
#[ignore = "run through make check"]
async fn a_required_tls_connection_verifies_the_certificate_chain_and_the_host_name() {
    let _test_guard = AUTHOR_EDIT_TEST_LOCK.lock().await;
    // SSL_CERT_FILE is process state, so the TLS cases run in a child process of this test binary.
    match std::env::var(CHILD_ADMIN_URL) {
        Ok(admin_url) => verify_the_tls_cases(&admin_url).await,
        Err(_) => run_the_tls_cases_in_a_child(),
    }
}

fn run_the_tls_cases_in_a_child() {
    let container = TlsContainer::start();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "--nocapture", &this_test_name()])
        .env("SSL_CERT_FILE", &container.trusted_ca_file)
        .env_remove("SSL_CERT_DIR")
        .env(CHILD_ADMIN_URL, container.admin_url())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("test result: ok. 1 passed;"),
        "{stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn this_test_name() -> String {
    let module = module_path!()
        .split_once("::")
        .expect("a module path of the crate")
        .1;
    format!("{module}::a_required_tls_connection_verifies_the_certificate_chain_and_the_host_name")
}

async fn verify_the_tls_cases(admin_url: &str) {
    let port = admin_url
        .rsplit_once(':')
        .unwrap()
        .1
        .split('/')
        .next()
        .unwrap();
    let tls_url = format!(
        "postgres://postgres:admin@localhost:{port}/postgres?sslmode=require&hostaddr=127.0.0.1"
    );

    serve_certificate(admin_url, "trusted").await;
    let client = Arc::new(ConnectionPool::new(tls_url.clone()))
        .checkout()
        .await
        .unwrap();
    let row = client
        .query_one(
            "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()",
            &[],
        )
        .await
        .unwrap();
    assert!(row.get::<_, bool>(0));
    // PostgreSQL 17 also accepts a direct TLS connection, which needs the ALPN protocol.
    let version: i32 = connect_plain(admin_url)
        .await
        .query_one("SELECT current_setting('server_version_num')::int", &[])
        .await
        .unwrap()
        .get(0);
    if version >= 170_000 {
        let direct = Arc::new(ConnectionPool::new(format!(
            "{tls_url}&sslnegotiation=direct"
        )))
        .checkout()
        .await
        .unwrap();
        let row = direct
            .query_one(
                "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()",
                &[],
            )
            .await
            .unwrap();
        assert!(row.get::<_, bool>(0));
    }

    serve_certificate(admin_url, "other-ca").await;
    assert!(matches!(
        certificate_error(&tls_url).await,
        Some(CertificateError::UnknownIssuer)
    ));

    serve_certificate(admin_url, "other-host").await;
    assert!(matches!(
        certificate_error(&tls_url).await,
        Some(CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. })
    ));
}

async fn certificate_error(tls_url: &str) -> Option<CertificateError> {
    let error = Arc::new(ConnectionPool::new(tls_url.to_owned()))
        .checkout()
        .await
        .err()?;
    let mut cause = std::error::Error::source(&error);
    while let Some(next) = cause {
        if let Some(rustls::Error::InvalidCertificate(certificate)) = next
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            .and_then(|inner| inner.downcast_ref::<rustls::Error>())
        {
            return Some(certificate.clone());
        }
        cause = next.source();
    }
    None
}

/// Turns TLS on with the certificate `name`, and waits until a new connection sees it.
async fn serve_certificate(admin_url: &str, name: &str) {
    let admin = connect_plain(admin_url).await;
    for statement in [
        format!("ALTER SYSTEM SET ssl_cert_file = '{CERTIFICATES}/{name}.crt'"),
        format!("ALTER SYSTEM SET ssl_key_file = '{CERTIFICATES}/{name}.key'"),
        "ALTER SYSTEM SET ssl = on".to_owned(),
        "SELECT pg_reload_conf()".to_owned(),
    ] {
        admin.batch_execute(&statement).await.unwrap();
    }
    let expected = format!("{CERTIFICATES}/{name}.crt");
    for _attempt in 0..100 {
        let row = connect_plain(admin_url)
            .await
            .query_one("SELECT current_setting('ssl_cert_file')", &[])
            .await
            .unwrap();
        if row.get::<_, String>(0) == expected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("PostgreSQL did not load the certificate {name}");
}

async fn connect_plain(url: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(url, NoTls).await.unwrap();
    tokio::spawn(async move {
        let _connection_result = connection.await;
    });
    client
}

/// A PostgreSQL container of the controlled image with the test certificates; drop removes it.
struct TlsContainer {
    name: String,
    trusted_ca_file: std::path::PathBuf,
}

impl TlsContainer {
    fn start() -> Self {
        let name = format!("storyos-tls-test-{}", uuid::Uuid::now_v7().simple());
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
        let trusted_ca_file = std::env::temp_dir().join(format!("{name}-trusted-ca.crt"));
        let container = Self {
            name,
            trusted_ca_file,
        };
        succeed(&mut run);
        container.wait_until_ready();
        succeed(Command::new("docker").args([
            "exec",
            "--user",
            "postgres",
            &container.name,
            "sh",
            "-ec",
            MAKE_CERTIFICATES,
        ]));
        let trusted_ca = succeed(Command::new("docker").args([
            "exec",
            &container.name,
            "cat",
            &format!("{CERTIFICATES}/trusted-root.crt"),
        ]));
        std::fs::write(&container.trusted_ca_file, trusted_ca).unwrap();
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

    fn admin_url(&self) -> String {
        let published = succeed(Command::new("docker").args(["port", &self.name, "5432/tcp"]));
        let published = String::from_utf8(published).unwrap();
        let port = published.trim().rsplit_once(':').unwrap().1;
        format!("postgres://postgres:admin@127.0.0.1:{port}/postgres")
    }
}

impl Drop for TlsContainer {
    fn drop(&mut self) {
        let _removed = Command::new("docker")
            .args(["rm", "--force", "--volumes", &self.name])
            .output();
        let _deleted = std::fs::remove_file(&self.trusted_ca_file);
    }
}

fn controlled_postgres_image() -> String {
    include_str!("../../../scripts/lib/controlled-postgres.sh")
        .lines()
        .find_map(|line| line.strip_prefix("storyos_postgres_image="))
        .expect("scripts/lib/controlled-postgres.sh names the PostgreSQL image")
        .to_owned()
}

fn succeed(command: &mut Command) -> Vec<u8> {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
