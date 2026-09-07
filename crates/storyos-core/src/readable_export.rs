//! Pinned UTF-8/LF human-readable manuscript export profile.

use std::fmt::{self, Write};

pub const READABLE_EXPORT_PROFILE: &str = "storyos.readable-export.utf8-lf.v1";
pub const READABLE_EXPORT_UNAVAILABLE_MARKER: &str = "[unavailable]";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadableExportChapter {
    pub title: String,
    pub body: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadableExportVolume {
    pub title: String,
    pub chapters: Vec<ReadableExportChapter>,
}

/// Render one manuscript as UTF-8 with LF separators and a final newline.
///
/// Volume and Chapter order is the caller-supplied order. Missing Chapter body
/// is the explicit non-prose marker, never invented text.
pub fn render_readable_manuscript(volumes: &[ReadableExportVolume]) -> String {
    let mut output = String::new();
    append_readable_manuscript(&mut output, volumes).expect("writing to String cannot fail");
    output
}

fn append_readable_manuscript(
    output: &mut impl Write,
    volumes: &[ReadableExportVolume],
) -> fmt::Result {
    for (volume_index, volume) in volumes.iter().enumerate() {
        if volume_index > 0 {
            output.write_str("\n\n")?;
        }
        write!(output, "# {}", volume.title)?;
        for chapter in &volume.chapters {
            write!(output, "\n\n## {}", chapter.title)?;
            output.write_str("\n\n")?;
            output.write_str(
                chapter
                    .body
                    .as_deref()
                    .unwrap_or(READABLE_EXPORT_UNAVAILABLE_MARKER),
            )?;
        }
    }
    output.write_char('\n')
}

#[cfg(test)]
#[path = "readable_export_tests.rs"]
mod tests;
