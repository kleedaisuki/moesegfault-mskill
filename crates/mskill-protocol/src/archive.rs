//! Filesystem-independent archive inspection for native and WASM callers.

use crate::{
    is_windows_device_name, validate_skill_name, MAX_ARCHIVE_BYTES, MAX_ARCHIVE_ENTRIES,
    MAX_EXPANDED_BYTES, MAX_SKILL_MD_BYTES,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};

/// Validated manifest and resource usage of a portable skill ZIP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveInspection {
    /// Manifest name, which must agree with the registry route or local target.
    pub name: String,
    /// Nonempty manifest description, used for registry listings.
    pub description: String,
    /// Total actually read regular-file bytes, not merely ZIP size declarations.
    pub expanded_bytes: u64,
    /// Total ZIP entry count, including explicit directories.
    pub entry_count: usize,
}

/// Invalid transport, archive, portable path, or manifest data.
#[derive(Debug, thiserror::Error)]
#[error("invalid skill archive: {0}")]
pub struct ArchiveError(pub String);

/// Only these root frontmatter fields affect the package management contract.
#[derive(Deserialize)]
struct Manifest {
    /// Portable directory and cloud package name.
    name: String,
    /// User-facing summary; not rendered as trusted HTML.
    description: String,
}

/// Validate a ZIP completely without extracting it or executing package code.
///
/// Both the registry and local importer must call this before committing state.
/// ZIP entry bytes are streamed through an 8 KiB buffer; only `SKILL.md` is kept.
/// `name` must additionally match the requested package identity at the caller.
pub fn validate_archive(bytes: &[u8]) -> Result<ArchiveInspection, ArchiveError> {
    ensure(
        !bytes.is_empty() && bytes.len() as u64 <= MAX_ARCHIVE_BYTES,
        "archive must be nonempty and at most 16 MiB",
    )?;
    let raw_count = preflight_zip(bytes)?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(error)?;
    ensure(zip.len() <= MAX_ARCHIVE_ENTRIES, "too many ZIP entries")?;
    ensure(zip.len() == raw_count, "ZIP parser entry count mismatch")?;
    let count = zip.len();
    let mut paths = BTreeMap::new();
    let mut explicit = BTreeSet::new();
    let mut expanded = 0;
    let mut manifest = None;
    for index in 0..count {
        let mut entry = zip.by_index(index).map_err(error)?;
        let raw = std::str::from_utf8(entry.name_raw())
            .map_err(error)?
            .to_owned();
        ensure(entry.name() == raw, "ZIP alternate entry name mismatch")?;
        validate_path(&raw, entry.is_dir(), &mut paths, &mut explicit)?;
        validate_kind(entry.unix_mode(), entry.is_dir())?;
        ensure(!entry.encrypted(), "encrypted entries are unsupported")?;
        if entry.is_dir() {
            ensure(entry.size() == 0, "directory entries must be empty")?;
            continue;
        }
        ensure(
            entry.size() <= MAX_EXPANDED_BYTES.saturating_sub(expanded),
            "expanded archive exceeds 64 MiB",
        )?;
        let mut collected = if raw == "SKILL.md" {
            Some(Vec::new())
        } else {
            None
        };
        let mut buffer = [0_u8; 8192];
        loop {
            let read = entry.read(&mut buffer).map_err(error)?;
            if read == 0 {
                break;
            }
            expanded += read as u64;
            ensure(
                expanded <= MAX_EXPANDED_BYTES,
                "expanded archive exceeds 64 MiB",
            )?;
            if let Some(text) = &mut collected {
                ensure(
                    text.len() as u64 + read as u64 <= MAX_SKILL_MD_BYTES,
                    "SKILL.md exceeds 1 MiB",
                )?;
                text.extend_from_slice(&buffer[..read]);
            }
        }
        if let Some(text) = collected {
            manifest = Some(parse_manifest(&text)?);
        }
    }
    let manifest = manifest
        .ok_or_else(|| ArchiveError("SKILL.md must be a regular file at the ZIP root".into()))?;
    Ok(ArchiveInspection {
        name: manifest.name,
        description: manifest.description,
        expanded_bytes: expanded,
        entry_count: count,
    })
}

/// Normalize paths once, reserving implicit parent directories to reject aliases.
fn validate_path(
    raw: &str,
    directory: bool,
    paths: &mut BTreeMap<String, (bool, String)>,
    explicit: &mut BTreeSet<String>,
) -> Result<(), ArchiveError> {
    ensure(
        !raw.is_empty() && raw.len() <= 1024 && !raw.starts_with('/') && !raw.contains('\\'),
        "unsafe ZIP path",
    )?;
    let path = if directory {
        raw.strip_suffix('/').unwrap_or(raw)
    } else {
        raw
    };
    let parts: Vec<_> = path.split('/').collect();
    for part in &parts {
        ensure(
            !part.is_empty() && *part != "." && *part != ".." && part.len() <= 255,
            "unsafe ZIP path component",
        )?;
        ensure(
            !part.ends_with('.') && !part.ends_with(' ') && !is_windows_device_name(part),
            "nonportable ZIP path component",
        )?;
        ensure(
            !part
                .chars()
                .any(|c| c.is_control() || "<>:\"|?*".contains(c)),
            "nonportable ZIP path character",
        )?;
    }
    let normalized = path.to_lowercase();
    ensure(
        explicit.insert(normalized.clone()),
        "duplicate or case-colliding ZIP entry",
    )?;
    for end in 1..parts.len() {
        let original = parts[..end].join("/");
        let parent = original.to_lowercase();
        if let Some((is_directory, spelling)) = paths.get(&parent) {
            ensure(*is_directory, "file used as ZIP parent directory")?;
            ensure(*spelling == original, "case-colliding ZIP parent directory")?;
        }
        paths.insert(parent, (true, original));
    }
    if let Some((previous_directory, spelling)) = paths.get(&normalized) {
        ensure(
            *previous_directory && directory,
            "conflicting ZIP entry type",
        )?;
        ensure(*spelling == path, "case-colliding ZIP directory")?;
    }
    paths.insert(normalized, (directory, path.to_owned()));
    Ok(())
}

/// Bound central-directory allocation before the ZIP library parses metadata.
///
/// ZIP64 and split archives are unnecessary under the 16 MiB product budget.
fn preflight_zip(bytes: &[u8]) -> Result<usize, ArchiveError> {
    ensure(bytes.len() >= 22, "truncated ZIP footer")?;
    let start = bytes.len().saturating_sub(65_557);
    let footer = (start..=bytes.len() - 22)
        .rev()
        .find(|offset| {
            bytes[*offset..].starts_with(b"PK\x05\x06")
                && *offset
                    + 22
                    + u16::from_le_bytes([bytes[*offset + 20], bytes[*offset + 21]]) as usize
                    == bytes.len()
        })
        .ok_or_else(|| ArchiveError("missing ZIP footer or trailing data".into()))?;
    let number =
        |offset: usize| u16::from_le_bytes([bytes[footer + offset], bytes[footer + offset + 1]]);
    ensure(
        number(4) == 0 && number(6) == 0 && number(8) == number(10),
        "split ZIP archives are unsupported",
    )?;
    ensure(
        number(10) as usize <= MAX_ARCHIVE_ENTRIES,
        "too many ZIP entries or unsupported ZIP64 archive",
    )?;
    ensure(
        footer < 20 || &bytes[footer - 20..footer - 16] != b"PK\x06\x07",
        "ZIP64 archives are unsupported",
    )?;
    let count = number(10) as usize;
    let directory_size = read_u32(bytes, footer + 12)? as usize;
    let directory_start = read_u32(bytes, footer + 16)? as usize;
    ensure(
        directory_start.checked_add(directory_size) == Some(footer),
        "invalid ZIP central-directory bounds",
    )?;
    let mut offset = directory_start;
    let mut paths = BTreeMap::new();
    let mut explicit = BTreeSet::new();
    for _ in 0..count {
        offset = validate_central_entry(bytes, offset, footer, &mut paths, &mut explicit)?;
    }
    ensure(offset == footer, "ZIP central-directory count mismatch")?;
    Ok(count)
}

/// Inspect raw records before `ZipArchive` can deduplicate equal file names.
fn validate_central_entry(
    bytes: &[u8],
    offset: usize,
    end: usize,
    paths: &mut BTreeMap<String, (bool, String)>,
    explicit: &mut BTreeSet<String>,
) -> Result<usize, ArchiveError> {
    ensure(
        offset.checked_add(46).is_some_and(|next| next <= end),
        "truncated ZIP central entry",
    )?;
    ensure(
        &bytes[offset..offset + 4] == b"PK\x01\x02",
        "invalid ZIP central entry signature",
    )?;
    let name_length = read_u16(bytes, offset + 28)? as usize;
    let extra_length = read_u16(bytes, offset + 30)? as usize;
    let comment_length = read_u16(bytes, offset + 32)? as usize;
    let next = offset + 46 + name_length + extra_length + comment_length;
    ensure(next <= end, "ZIP central entry exceeds directory bounds")?;
    ensure(
        read_u16(bytes, offset + 34)? == 0,
        "split ZIP archives are unsupported",
    )?;
    let name_bytes = &bytes[offset + 46..offset + 46 + name_length];
    let name = std::str::from_utf8(name_bytes).map_err(error)?;
    validate_path(name, name.ends_with('/'), paths, explicit)?;
    let local = read_u32(bytes, offset + 42)? as usize;
    validate_local_name(bytes, local, name_bytes, offset)?;
    Ok(next)
}

/// Reject central/local name disagreements instead of relying on parser choice.
fn validate_local_name(
    bytes: &[u8],
    local: usize,
    expected: &[u8],
    central: usize,
) -> Result<(), ArchiveError> {
    ensure(
        local.checked_add(30).is_some_and(|end| end <= central),
        "invalid ZIP local-header bounds",
    )?;
    ensure(
        &bytes[local..local + 4] == b"PK\x03\x04",
        "invalid ZIP local-header signature",
    )?;
    let length = read_u16(bytes, local + 26)? as usize;
    let end = local + 30 + length;
    ensure(end <= central, "ZIP local name exceeds data bounds")?;
    ensure(
        &bytes[local + 30..end] == expected,
        "ZIP central/local name mismatch",
    )
}

/// Read a bounded little-endian ZIP field without unsafe code or unchecked slices.
fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, ArchiveError> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| ArchiveError("truncated ZIP field".into()))?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

/// Read a bounded ZIP32 field; ZIP64 is deliberately outside the product format.
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ArchiveError> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| ArchiveError("truncated ZIP field".into()))?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

/// Reject Unix symlinks, devices, sockets, FIFOs, and mode/type disagreements.
fn validate_kind(mode: Option<u32>, directory: bool) -> Result<(), ArchiveError> {
    let kind = mode.unwrap_or_default() & 0o170000;
    ensure(
        kind == 0 || kind == if directory { 0o040000 } else { 0o100000 },
        "only regular files and directories are supported",
    )
}

/// Parse bounded UTF-8 YAML frontmatter, leaving skill body semantics to agents.
fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ArchiveError> {
    let text = std::str::from_utf8(bytes).map_err(error)?;
    let normalized = text.replace("\r\n", "\n");
    let after = normalized
        .strip_prefix("---\n")
        .ok_or_else(|| ArchiveError("SKILL.md requires YAML frontmatter".into()))?;
    let end = after
        .find("\n---\n")
        .or_else(|| after.strip_suffix("\n---").map(|v| v.len()))
        .ok_or_else(|| ArchiveError("SKILL.md frontmatter is not closed".into()))?;
    ensure(end <= 64 * 1024, "frontmatter exceeds 64 KiB")?;
    let manifest: Manifest = serde_yaml::from_str(&after[..end]).map_err(error)?;
    validate_skill_name(&manifest.name).map_err(error)?;
    ensure(
        !manifest.description.trim().is_empty() && manifest.description.chars().count() <= 1024,
        "description must be nonempty and at most 1024 characters",
    )?;
    Ok(manifest)
}

/// Convert parser and I/O diagnostics into one portable public error type.
fn error(value: impl std::fmt::Display) -> ArchiveError {
    ArchiveError(value.to_string())
}

/// Keep sequential validation flat instead of nesting per-entry branches.
fn ensure(condition: bool, message: &str) -> Result<(), ArchiveError> {
    if condition {
        Ok(())
    } else {
        Err(ArchiveError(message.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{write::SimpleFileOptions, ZipWriter};

    /// Construct in-memory ZIPs without writing outside the task workspace.
    fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, content) in entries {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    /// A portable package is inspected without needing native filesystem APIs.
    #[test]
    fn validates_root_manifest() {
        let bytes = archive(&[(
            "SKILL.md",
            "---\nname: example\ndescription: Example skill\n---\n# Example",
        )]);
        let inspection = validate_archive(&bytes).unwrap();
        assert_eq!(inspection.name, "example");
        assert_eq!(inspection.entry_count, 1);
    }

    /// Malicious paths and Windows aliases fail before any extraction occurs.
    #[test]
    fn rejects_nonportable_entries() {
        for path in [
            "../evil", "a\\evil", "/evil", "AUX.txt", "a:b", "a./x", "a//x",
        ] {
            let bytes = archive(&[(path, "x")]);
            assert!(validate_archive(&bytes).is_err(), "{path}");
        }
        assert!(validate_archive(&archive(&[("a", "x"), ("a/b", "x")])).is_err());
        assert!(validate_archive(&archive(&[("File", "x"), ("file", "x")])).is_err());
        assert!(validate_archive(&archive(&[("A/x", "x"), ("a/y", "x")])).is_err());
    }

    /// Duplicate raw records must fail even if the ZIP library indexes by name.
    #[test]
    fn rejects_duplicate_central_records() {
        let bytes = archive(&[(
            "SKILL.md",
            "---\nname: example\ndescription: Example\n---\n",
        )]);
        let footer = bytes.len() - 22;
        let central = read_u32(&bytes, footer + 16).unwrap() as usize;
        let record = &bytes[central..footer];
        let mut duplicate = bytes[..footer].to_vec();
        duplicate.extend_from_slice(record);
        let mut end = bytes[footer..].to_vec();
        end[8..10].copy_from_slice(&2_u16.to_le_bytes());
        end[10..12].copy_from_slice(&2_u16.to_le_bytes());
        end[12..16].copy_from_slice(&((record.len() * 2) as u32).to_le_bytes());
        duplicate.extend_from_slice(&end);
        let failure = validate_archive(&duplicate).unwrap_err();
        assert!(failure.0.contains("duplicate"), "{failure}");
    }
}
