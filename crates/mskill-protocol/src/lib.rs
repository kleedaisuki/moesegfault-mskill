//! Wire contracts and portable package constraints shared by the CLI and registry.
//!
//! A skill has one mutable latest state. SHA-256 identifies archive bytes, not a
//! version, publisher signature, or guarantee that the skill is safe to execute.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, str::FromStr};

mod archive;
pub use archive::{validate_archive, ArchiveError, ArchiveInspection};

/// Registry API prefix. Routes are independent of the registry hostname.
pub const API_PREFIX: &str = "/v1";
/// Media type of a `.skill` file: a ZIP with `SKILL.md` at its root.
pub const SKILL_MEDIA_TYPE: &str = "application/vnd.mskill.skill";
/// Maximum compressed archive size accepted by the client and registry.
pub const MAX_ARCHIVE_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum total expanded regular-file size; enforce while actually reading.
pub const MAX_EXPANDED_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum ZIP entries, including explicit directory entries.
pub const MAX_ARCHIVE_ENTRIES: usize = 4096;
/// Maximum UTF-8 root skill manifest size.
pub const MAX_SKILL_MD_BYTES: u64 = 1024 * 1024;
/// Namespace for locally authored skills that have not been published.
pub const LOCAL_OWNER: &str = "local";
/// Header linking CLI diagnostics and Workers request logs.
pub const REQUEST_ID_HEADER: &str = "x-mskill-request-id";

/// A fully qualified skill identity, unambiguous across publisher namespaces.
///
/// ```
/// use mskill_protocol::SkillId;
/// let skill: SkillId = "publisher_123/rust-review".parse().unwrap();
/// assert_eq!(skill.name, "rust-review");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillId {
    /// Immutable opaque registry account ID; `local` is never a cloud publisher.
    pub owner_id: String,
    /// Portable lowercase package name, also used as the project directory name.
    pub name: String,
}

impl SkillId {
    /// Construct an identity after validating both path components.
    pub fn new(
        owner_id: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<Self, ValidationError> {
        let id = Self {
            owner_id: owner_id.into(),
            name: name.into(),
        };
        id.validate()?;
        Ok(id)
    }

    /// Validate identities received from JSON or other untrusted boundaries.
    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_owner_id(&self.owner_id)?;
        validate_skill_name(&self.name)
    }
}

impl fmt::Display for SkillId {
    /// Format the canonical CLI and API reference without percent-encoding.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.owner_id, self.name)
    }
}

impl FromStr for SkillId {
    type Err = ValidationError;

    /// Parse exactly two validated components separated by one slash.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (owner, name) = value
            .split_once('/')
            .ok_or(ValidationError::InvalidReference)?;
        Self::new(owner, name)
    }
}

/// Current public registry state. Unknown future JSON fields may be ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillMetadata {
    /// Immutable publisher namespace, not a display name or Identity subject.
    pub owner_id: String,
    /// Validated skill directory name.
    pub name: String,
    /// Lowercase SHA-256 hex digest of the exact downloadable ZIP bytes.
    pub sha256: String,
    /// Compressed ZIP size in bytes.
    pub size_bytes: u64,
    /// Human-readable description extracted from root SKILL.md frontmatter.
    pub description: String,
    /// Last successful state change, expressed as an RFC 3339 UTC timestamp.
    pub updated_at: String,
}

impl SkillMetadata {
    /// Obtain and validate the full identity at a deserialization boundary.
    pub fn id(&self) -> Result<SkillId, ValidationError> {
        SkillId::new(&self.owner_id, &self.name)
    }

    /// Enforce the portable identity, digest, and archive-size wire invariants.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.id()?;
        validate_sha256(&self.sha256)?;
        if self.size_bytes == 0 || self.size_bytes > MAX_ARCHIVE_BYTES {
            return Err(ValidationError::InvalidArchiveSize);
        }
        Ok(())
    }
}

/// A registry list response, sorted by owner and name by the service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillList {
    /// Current package states; no historical releases are exposed.
    pub skills: Vec<SkillMetadata>,
    /// Opaque continuation marker; absent when the listing is complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Product account mapping returned only to the authenticated token holder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserProfile {
    /// Immutable opaque public publisher ID mapped from exact `(issuer, sub)`.
    pub owner_id: String,
    /// Optional presentation-only name, never an authorization input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// RFC 9457-style service error; clients branch on `error_code`, not prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiProblem {
    /// Problem type URI; `about:blank` is valid for generic failures.
    #[serde(rename = "type")]
    pub problem_type: String,
    /// Short user-facing error title.
    pub title: String,
    /// HTTP status repeated for diagnostic transport independence.
    pub status: u16,
    /// Actionable, token-free explanatory message.
    pub detail: String,
    /// Stable machine code such as `skill_not_found` or `invalid_archive`.
    pub error_code: String,
    /// Correlation key also supplied in `x-mskill-request-id`.
    pub request_id: String,
}

/// Portable identity and metadata validation failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// A fully qualified reference must contain one owner and one name.
    #[error("expected a skill reference in owner/name form")]
    InvalidReference,
    /// Owner IDs are opaque but must remain safe URL/path components.
    #[error("owner ID must be 1..128 ASCII letters, digits, underscores or hyphens")]
    InvalidOwner,
    /// Skill names must be portable across supported operating systems.
    #[error("skill name must be 1..64 lowercase letters/digits with internal hyphens and not a reserved device name")]
    InvalidName,
    /// Content digests use one unambiguous textual representation.
    #[error("SHA-256 must be exactly 64 lowercase hexadecimal characters")]
    InvalidDigest,
    /// Transport and Worker memory budgets bound archive inputs.
    #[error("archive must be nonempty and no larger than 16 MiB")]
    InvalidArchiveSize,
}

/// Validate a namespace before putting it into a URL or filesystem path.
pub fn validate_owner_id(owner: &str) -> Result<(), ValidationError> {
    if owner.is_empty()
        || owner.len() > 128
        || !owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(ValidationError::InvalidOwner);
    }
    // Device names also apply to Windows parent directories.
    if is_windows_device_name(owner) {
        return Err(ValidationError::InvalidOwner);
    }
    Ok(())
}

/// Validate the common denominator of skill manifests and project directories.
pub fn validate_skill_name(name: &str) -> Result<(), ValidationError> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && name.as_bytes()[0] != b'-'
        && name.as_bytes()[name.len() - 1] != b'-'
        && !name.contains("--")
        && !is_windows_device_name(name);
    if valid {
        Ok(())
    } else {
        Err(ValidationError::InvalidName)
    }
}

/// Recognize DOS device basenames, including names followed by an extension.
pub fn is_windows_device_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || matches!(stem.as_str(), "CONIN$" | "CONOUT$")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|suffix| matches!(suffix, "¹" | "²" | "³"))
        })
}

/// Reject noncanonical digest strings before comparison or storage lookup.
pub fn validate_sha256(value: &str) -> Result<(), ValidationError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        Ok(())
    } else {
        Err(ValidationError::InvalidDigest)
    }
}

/// Compute the canonical digest of archive bytes without allocating a hex crate.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Portable names cannot create traversal or Windows device paths.
    #[test]
    fn identity_is_portable() {
        assert!("owner_1/rust-review".parse::<SkillId>().is_ok());
        for value in [
            "owner/../x",
            "../name",
            "owner/CON",
            "owner/con",
            "owner/-name",
            "owner/name-",
            "owner/a--b",
            "owner/a/b",
            "con/name",
        ] {
            assert!(value.parse::<SkillId>().is_err(), "{value}");
        }
    }

    /// Hash comparisons use standard SHA-256 with canonical lowercase hex.
    #[test]
    fn digest_is_canonical() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(validate_sha256(&sha256_hex(b"abc")).is_ok());
        assert!(validate_sha256(&"A".repeat(64)).is_err());
    }

    /// Wire metadata remains flat and accepts additive future fields.
    #[test]
    fn metadata_wire_shape() {
        let metadata = SkillMetadata {
            owner_id: "owner_1".into(),
            name: "example".into(),
            sha256: sha256_hex(b"zip"),
            size_bytes: 3,
            description: "Example".into(),
            updated_at: "2026-10-05T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"owner_id\":\"owner_1\""));
        assert_eq!(
            serde_json::from_str::<SkillMetadata>(&json).unwrap(),
            metadata
        );
        metadata.validate().unwrap();
    }
}
