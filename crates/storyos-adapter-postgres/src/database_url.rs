//! The startup rule of a PostgreSQL URL (ADR 0050, ADR 0104 section 5).

#[cfg(test)]
#[path = "database_url_tests.rs"]
mod tests;

use std::net::IpAddr;

use tokio_postgres::config::{Config, Host, SslMode};

/// Why a binary refuses a PostgreSQL URL at startup. It holds no part of the URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseUrlRefusal {
    Unparsable,
    TlsRequired,
}

impl DatabaseUrlRefusal {
    /// The rule that the value breaks, for the startup message.
    pub fn rule(self) -> &'static str {
        match self {
            Self::Unparsable => "the value must be a PostgreSQL connection string",
            Self::TlsRequired => {
                "a URL whose host is not a loopback IP address or a Unix socket must say sslmode=require"
            }
        }
    }
}

/// Accepts a URL that parses and that says `sslmode=require` when a host is not a loopback
/// IP address or a Unix socket. A binary checks it before it connects.
pub fn check_database_url(url: &str) -> Result<(), DatabaseUrlRefusal> {
    let config = url
        .parse::<Config>()
        .map_err(|_| DatabaseUrlRefusal::Unparsable)?;
    if config.get_ssl_mode() == SslMode::Require || is_local(&config) {
        Ok(())
    } else {
        Err(DatabaseUrlRefusal::TlsRequired)
    }
}

fn is_local(config: &Config) -> bool {
    let hosts = config.get_hosts().iter().all(|host| match host {
        // A host name such as `localhost` is not local: a name can resolve to another address.
        Host::Tcp(name) => name
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback()),
        #[cfg(unix)]
        Host::Unix(_) => true,
    });
    hosts && config.get_hostaddrs().iter().all(IpAddr::is_loopback)
}

/// The message chain of `error`, with each host, user, database name, and password of `url`
/// replaced by a mark. A startup line can then give the cause and no part of the URL.
pub(crate) fn redacted_chain(url: &str, error: &(dyn std::error::Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    let Ok(config) = url.parse::<Config>() else {
        return text;
    };
    let mut parts = Vec::new();
    for host in config.get_hosts() {
        match host {
            Host::Tcp(name) => parts.push((name.clone(), "<host>")),
            #[cfg(unix)]
            Host::Unix(path) => parts.push((path.display().to_string(), "<host>")),
        }
    }
    for address in config.get_hostaddrs() {
        parts.push((address.to_string(), "<host>"));
    }
    if let Some(user) = config.get_user() {
        parts.push((user.to_owned(), "<user>"));
    }
    if let Some(database) = config.get_dbname() {
        parts.push((database.to_owned(), "<database>"));
    }
    if let Some(password) = config.get_password() {
        parts.push((String::from_utf8_lossy(password).into_owned(), "<password>"));
    }
    // A longer part first, so that a part inside another part does not leave the rest of it.
    parts.sort_by_key(|(part, _)| std::cmp::Reverse(part.len()));
    for (part, mark) in parts {
        text = replace_word(&text, &part, mark);
    }
    text
}

/// Replaces each occurrence of `part` that is not inside a longer word.
fn replace_word(text: &str, part: &str, mark: &str) -> String {
    if part.is_empty() {
        return text.to_owned();
    }
    let word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut replaced = String::with_capacity(text.len());
    let mut copied = 0;
    for (index, _) in text.match_indices(part) {
        let end = index + part.len();
        if text[..index].chars().next_back().is_some_and(word)
            || text[end..].chars().next().is_some_and(word)
        {
            continue;
        }
        replaced.push_str(&text[copied..index]);
        replaced.push_str(mark);
        copied = end;
    }
    replaced.push_str(&text[copied..]);
    replaced
}
