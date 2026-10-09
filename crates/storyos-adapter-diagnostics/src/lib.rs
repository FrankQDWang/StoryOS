//! The Diagnostic Projection formatter of the Server and the Worker (ADR 0047).

use std::fmt;
use std::panic::Location;

use storyos_application::DiagnosticField;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::Layer as _;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;

/// The environment variable that selects the level.
pub const LOG_LEVEL_VARIABLE: &str = "STORYOS_LOG";

/// A `STORYOS_LOG` value outside `off`, `error`, `warn`, `info`, and `debug`.
pub struct UnknownLogLevel;

impl fmt::Display for UnknownLogLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("STORYOS_LOG must be off, error, warn, info, or debug")
    }
}

// A binary prints the `Debug` text of the error that `main` returns.
impl fmt::Debug for UnknownLogLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl std::error::Error for UnknownLogLevel {}

/// Installs JSON lines on stderr for `storyos` targets only, and a panic hook that writes the
/// source location only. An absent value selects `info`.
pub fn install(level: Option<&str>) -> Result<(), UnknownLogLevel> {
    let level = match level {
        None | Some("info") => LevelFilter::INFO,
        Some("off") => LevelFilter::OFF,
        Some("error") => LevelFilter::ERROR,
        Some("warn") => LevelFilter::WARN,
        Some("debug") => LevelFilter::DEBUG,
        Some(_) => return Err(UnknownLogLevel),
    };
    let formatter = tracing_subscriber::fmt::layer()
        .json()
        .with_writer(std::io::stderr)
        .with_span_events(FmtSpan::CLOSE)
        .with_filter(Targets::new().with_target("storyos", level));
    tracing_subscriber::registry().with(formatter).init();
    std::panic::set_hook(Box::new(|panic| match panic.location() {
        Some(location) => {
            tracing::error!(location = SourceLocation(location).diagnostic(), "panic")
        }
        None => tracing::error!("panic"),
    }));
    Ok(())
}

struct SourceLocation<'a>(&'a Location<'a>);

impl DiagnosticField for SourceLocation<'_> {
    type Value<'b>
        = String
    where
        Self: 'b;

    fn diagnostic(&self) -> Self::Value<'_> {
        format!("{}:{}", self.0.file(), self.0.line())
    }
}
