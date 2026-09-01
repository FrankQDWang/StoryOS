//! Pinned UTF-8/LF human-readable manuscript export profile.

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
    let mut parts = Vec::new();
    for volume in volumes {
        parts.push(format!("# {}", volume.title));
        for chapter in &volume.chapters {
            parts.push(format!("## {}", chapter.title));
            parts.push(
                chapter
                    .body
                    .as_deref()
                    .unwrap_or(READABLE_EXPORT_UNAVAILABLE_MARKER)
                    .to_owned(),
            );
        }
    }
    if parts.is_empty() {
        return "\n".to_owned();
    }
    format!("{}\n", parts.join("\n\n"))
}

#[cfg(test)]
#[path = "readable_export_tests.rs"]
mod tests;
