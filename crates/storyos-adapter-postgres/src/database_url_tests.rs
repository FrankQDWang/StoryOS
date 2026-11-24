use super::{DatabaseUrlRefusal, check_database_url};

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

#[test]
fn sslmode_require_needs_a_name_for_each_host() {
    let urls = [
        "host='' hostaddr=127.0.0.1 user=storyos sslmode=require",
        "hostaddr=10.0.0.5 user=storyos sslmode=require",
    ];

    assert_eq!(
        urls.map(check_database_url),
        [Err(DatabaseUrlRefusal::TlsHostNameMissing), Ok(()),]
    );
}
