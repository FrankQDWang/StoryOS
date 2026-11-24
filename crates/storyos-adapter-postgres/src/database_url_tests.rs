use super::{DatabaseUrlRefusal, check_database_url, redacted_chain};

#[test]
fn a_host_that_is_not_loopback_needs_sslmode_require() {
    let urls = [
        "postgres://storyos:secret@db.example.invalid:5432/postgres",
        "postgres://storyos:secret@db.example.invalid/postgres?sslmode=prefer",
        "postgres://storyos:secret@db.example.invalid/postgres?sslmode=disable",
        "postgres://storyos:secret@10.0.0.5/postgres",
        "postgres://storyos:secret@localhost/postgres",
        "postgres://storyos:secret@127.0.0.1/postgres?hostaddr=10.0.0.5",
        "postgres://storyos:secret@127.0.0.1,db.example.invalid/postgres",
        "postgres://storyos:secret@db.example.invalid/postgres?sslmode=require",
        "host=db.example.invalid user=storyos sslmode=require",
        "not a url",
    ];

    assert_eq!(
        urls.map(check_database_url),
        [
            Err(DatabaseUrlRefusal::TlsRequired),
            Err(DatabaseUrlRefusal::TlsRequired),
            Err(DatabaseUrlRefusal::TlsRequired),
            Err(DatabaseUrlRefusal::TlsRequired),
            Err(DatabaseUrlRefusal::TlsRequired),
            Err(DatabaseUrlRefusal::TlsRequired),
            Err(DatabaseUrlRefusal::TlsRequired),
            Ok(()),
            Ok(()),
            Err(DatabaseUrlRefusal::Unparsable),
        ]
    );
}

#[test]
fn a_loopback_or_socket_url_with_no_sslmode_stays_valid() {
    let urls = [
        "postgres://postgres:admin@127.0.0.1:5432/postgres",
        "postgres://postgres:admin@[::1]:5432/postgres",
        "postgres://postgres:admin@127.0.0.1/postgres?hostaddr=127.0.0.1",
        "postgres://postgres:admin@127.0.0.1/postgres?sslmode=disable",
        "postgres://postgres:admin@localhost/postgres?sslmode=require",
        "host=/var/run/postgresql user=postgres",
    ];

    assert_eq!(urls.map(check_database_url), [Ok(()); 6]);
}

#[derive(Debug)]
struct Caused(&'static str, std::io::Error);

impl std::fmt::Display for Caused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for Caused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.1)
    }
}

#[test]
fn the_cause_chain_names_no_host_user_database_or_password_of_the_url() {
    let error = Caused(
        "db error",
        std::io::Error::other(
            "FATAL: password authentication failed for user \"canary_user\" at \
             db.canary.invalid:6543 to canary_db (canary_db_old, canary-secret), \
             certificate not valid for name \"db.canary.invalid\"",
        ),
    );

    assert_eq!(
        redacted_chain(
            "postgres://canary_user:canary-secret@db.canary.invalid:6543/canary_db?sslmode=require",
            &error,
        ),
        "db error: FATAL: password authentication failed for user \"<user>\" at <host>:6543 \
         to <database> (canary_db_old, <password>), certificate not valid for name \"<host>\""
    );
}
