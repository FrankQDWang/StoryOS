//! Archive Path Profile `storyos.archive-path.utf8-nfc-unicode-16.0.0.v1`.

pub const ARCHIVE_PATH_PROFILE: &str = "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1";
pub const ARCHIVE_PATH_MAX_BYTES: usize = 512;
pub const ARCHIVE_PATH_MAX_SEGMENTS: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedArchivePath {
    pub path: String,
    pub collision_key: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchivePathRefusal {
    Empty,
    Absolute,
    Backslash,
    RepeatedSeparator,
    TrailingSeparator,
    DotSegment,
    DrivePrefix,
    TooLong,
    TooDeep,
    ControlCode,
    NotAlreadyNfc,
}

/// Admit one logical archive entry path before sort or digest coverage.
pub fn admit_archive_path(path: &str) -> Result<AdmittedArchivePath, ArchivePathRefusal> {
    if path.is_empty() {
        return Err(ArchivePathRefusal::Empty);
    }
    if path.len() > ARCHIVE_PATH_MAX_BYTES {
        return Err(ArchivePathRefusal::TooLong);
    }
    if path.starts_with('/') {
        return Err(ArchivePathRefusal::Absolute);
    }
    if path.ends_with('/') {
        return Err(ArchivePathRefusal::TrailingSeparator);
    }
    if path.contains('\\') {
        return Err(ArchivePathRefusal::Backslash);
    }
    if path.contains("//") {
        return Err(ArchivePathRefusal::RepeatedSeparator);
    }
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() > ARCHIVE_PATH_MAX_SEGMENTS {
        return Err(ArchivePathRefusal::TooDeep);
    }
    for (index, segment) in segments.iter().copied().enumerate() {
        if segment.is_empty() {
            return Err(ArchivePathRefusal::RepeatedSeparator);
        }
        if segment == "." || segment == ".." {
            return Err(ArchivePathRefusal::DotSegment);
        }
        if index == 0 && is_ascii_drive_prefix(segment) {
            return Err(ArchivePathRefusal::DrivePrefix);
        }
    }
    for scalar in path.chars() {
        if is_disallowed_control(scalar) {
            return Err(ArchivePathRefusal::ControlCode);
        }
        if !scalar.is_ascii() {
            return Err(ArchivePathRefusal::NotAlreadyNfc);
        }
    }
    Ok(AdmittedArchivePath {
        collision_key: ascii_default_case_fold(path),
        path: path.to_owned(),
    })
}

fn is_ascii_drive_prefix(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_disallowed_control(scalar: char) -> bool {
    matches!(scalar, '\0'..='\u{1F}' | '\u{7F}'..= '\u{9F}')
}

fn ascii_default_case_fold(path: &str) -> String {
    path.chars()
        .map(|scalar| {
            if scalar.is_ascii_uppercase() {
                scalar.to_ascii_lowercase()
            } else {
                scalar
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "archive_path_tests.rs"]
mod tests;
