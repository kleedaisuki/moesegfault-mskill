//! Portable skill archives and serialized local/project lifecycle operations.
use anyhow::{bail, ensure, Context, Result};
use fs2::FileExt;
use mskill_protocol::SkillId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

/// Maximum compressed archive size, expanded size, and entry count.
pub const MAX_ARCHIVE_BYTES: u64 = mskill_protocol::MAX_ARCHIVE_BYTES;
const MAX_EXPANDED_BYTES: u64 = mskill_protocol::MAX_EXPANDED_BYTES;
const MAX_ENTRIES: usize = mskill_protocol::MAX_ARCHIVE_ENTRIES;

/// A latest-only skill and its stable local paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkillInfo {
    /// Namespace and validated frontmatter name.
    pub id: SkillId,
    /// Frontmatter name, convenient for display.
    pub name: String,
    /// SHA-256 of the exact canonical archive bytes.
    pub sha256: String,
    /// Compressed archive length.
    pub size_bytes: u64,
    /// Frontmatter description.
    pub description: String,
    /// Portable ZIP archive ending in `.skill`.
    pub archive: PathBuf,
    /// Stable expanded directory used by project links.
    pub directory: PathBuf,
}

/// Library rooted at the CLI-selected home; operations serialize through a file lock.
pub struct LocalStore {
    root: PathBuf,
}
#[derive(Deserialize)]
struct Frontmatter {
    name: String,
    description: String,
}

/// Private proof that these exact immutable bytes passed the complete shared inspector.
/// Keeping inspection and bytes together prevents internal lifecycle calls from
/// repeating decompression or accidentally applying inspection to different bytes.
struct ValidatedArchive<B> {
    /// Owned source bytes or borrowed public-boundary input; never mutated.
    bytes: B,
    /// Manifest and bounds established by the one complete validation pass.
    inspection: mskill_protocol::ArchiveInspection,
}

impl<B: AsRef<[u8]>> ValidatedArchive<B> {
    /// Construct only by fully validating the associated bytes.
    fn new(bytes: B) -> Result<Self> {
        let inspection = mskill_protocol::validate_archive(bytes.as_ref())?;
        #[cfg(test)]
        VALIDATION_PASSES.with(|count| count.set(count.get() + 1));
        Ok(Self { bytes, inspection })
    }
}

#[cfg(test)]
thread_local! {
    /// Per-test-thread instrumentation; it has no production runtime cost.
    static VALIDATION_PASSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[derive(Default, Serialize, Deserialize)]
struct ProjectManifest {
    entries: BTreeMap<String, ProjectEntry>,
}
#[derive(Serialize, Deserialize)]
struct ProjectEntry {
    id: SkillId,
    sha256: String,
    linked: bool,
}

/// Recovery intent is persisted before any library path changes.
#[derive(Serialize, Deserialize)]
struct InstallIntent {
    id: SkillId,
    existed: [bool; 3],
}

impl LocalStore {
    /// Open or initialize a library. The CLI normally supplies `~/.mskill`.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        fs::create_dir_all(root.as_ref())?;
        let root = fs::canonicalize(root)?;
        for dir in ["archives", "library", "metadata", ".temp"] {
            fs::create_dir_all(root.join(dir))?;
        }
        let store = Self { root };
        let _lock = store.lock()?;
        store.recover_install()?;
        Ok(store)
    }
    fn lock(&self) -> Result<File> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join(".lock"))?;
        file.lock_exclusive()?;
        Ok(file)
    }
    fn paths(&self, id: &SkillId) -> Result<(PathBuf, PathBuf, PathBuf)> {
        validate_id(id)?;
        Ok((
            self.root
                .join("archives")
                .join(&id.owner_id)
                .join(format!("{}.skill", id.name)),
            self.root.join("library").join(&id.owner_id).join(&id.name),
            self.root
                .join("metadata")
                .join(&id.owner_id)
                .join(format!("{}.json", id.name)),
        ))
    }
    fn recover_install(&self) -> Result<()> {
        let stage = self.root.join(".temp/install");
        let intent_path = stage.join("intent.json");
        if !intent_path.exists() {
            return Ok(());
        }
        let intent: InstallIntent = serde_json::from_slice(&fs::read(intent_path)?)?;
        let (archive, tree, metadata) = self.paths(&intent.id)?;
        if !stage.join("committed").exists() {
            for (index, target) in [tree, archive, metadata].iter().enumerate() {
                let backup = stage.join(format!("backup-{index}"));
                if backup.exists() {
                    remove_path(target)?;
                    fs::rename(backup, target)?;
                } else if !intent.existed[index] {
                    remove_path(target)?;
                }
            }
        }
        remove_path(&stage)
    }
    fn commit_install(&self, stage: &Path, info: &SkillInfo, metadata: &Path) -> Result<()> {
        let targets = [&info.directory, &info.archive, metadata];
        let intent = InstallIntent {
            id: info.id.clone(),
            existed: targets.map(|path| path.exists()),
        };
        fs::write(stage.join("metadata"), serde_json::to_vec_pretty(info)?)?;
        write_json(&stage.join("intent.json"), &intent)?;
        let result = (|| -> Result<()> {
            for (index, (target, source)) in targets
                .iter()
                .zip(["tree", "archive", "metadata"])
                .enumerate()
            {
                fs::create_dir_all(target.parent().unwrap())?;
                if intent.existed[index] {
                    fs::rename(target, stage.join(format!("backup-{index}")))?;
                }
                fs::rename(stage.join(source), target)?;
            }
            let marker = File::create(stage.join("committed"))?;
            marker.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            self.recover_install()
                .context("failed to roll back library transaction")?;
        }
        result?;
        remove_path(stage)
    }
    /// Pack a standard directory or import an archive under the `local` namespace.
    pub fn add(&self, source: impl AsRef<Path>) -> Result<SkillInfo> {
        let validated = validated_source(source.as_ref())?;
        self.install_validated(
            &SkillId {
                owner_id: "local".into(),
                name: validated.inspection.name.clone(),
            },
            &validated,
        )
    }
    /// Install latest bytes after validating the complete ZIP; unchanged SHA-256 is a no-op.
    pub fn install_bytes(&self, id: &SkillId, bytes: &[u8]) -> Result<SkillInfo> {
        self.install_validated(id, &ValidatedArchive::new(bytes)?)
    }
    /// Commit an internal archive whose exact bytes already passed full validation.
    fn install_validated<B: AsRef<[u8]>>(
        &self,
        id: &SkillId,
        validated: &ValidatedArchive<B>,
    ) -> Result<SkillInfo> {
        let _lock = self.lock()?;
        self.recover_install()?;
        let (archive, directory, metadata) = self.paths(id)?;
        let front = &validated.inspection;
        let bytes = validated.bytes.as_ref();
        ensure!(
            front.name == id.name,
            "archive name does not match requested skill"
        );
        let hash = digest(bytes);
        if metadata.exists() {
            let old = self.get_inner(id)?;
            if old.sha256 == hash && archive.exists() && directory.exists() {
                return Ok(old);
            }
        }
        let stage = self.root.join(".temp").join("install");
        if stage.exists() {
            fs::remove_dir_all(&stage)?;
        }
        fs::create_dir_all(stage.join("tree"))?;
        extract(bytes, &stage.join("tree"))?;
        fs::write(stage.join("archive"), bytes)?;
        let info = SkillInfo {
            id: id.clone(),
            name: front.name.clone(),
            description: front.description.clone(),
            sha256: hash,
            size_bytes: bytes.len() as u64,
            archive,
            directory,
        };
        self.commit_install(&stage, &info, &metadata)?;
        Ok(info)
    }
    /// Read metadata for one namespace-qualified skill.
    pub fn get(&self, id: &SkillId) -> Result<SkillInfo> {
        let _lock = self.lock()?;
        self.recover_install()?;
        self.get_inner(id)
    }
    fn get_inner(&self, id: &SkillId) -> Result<SkillInfo> {
        let (archive, directory, path) = self.paths(id)?;
        let mut info: SkillInfo =
            serde_json::from_slice(&fs::read(path).context("skill is not installed")?)
                .context("invalid local metadata")?;
        ensure!(info.id == *id, "local metadata identity mismatch");
        // Paths are derived, never trusted from a serialized metadata file.
        info.archive = archive;
        info.directory = directory;
        Ok(info)
    }
    /// List all installed namespaces in lexical order.
    pub fn list(&self) -> Result<Vec<SkillInfo>> {
        let _lock = self.lock()?;
        self.recover_install()?;
        let mut out = Vec::new();
        for owner in fs::read_dir(self.root.join("metadata"))? {
            let owner = owner?;
            if !owner.file_type()?.is_dir() {
                continue;
            }
            for file in fs::read_dir(owner.path())? {
                let file = file?;
                if file.path().extension().is_some_and(|e| e == "json") {
                    let info: SkillInfo = serde_json::from_slice(&fs::read(file.path())?)?;
                    out.push(self.get_inner(&info.id)?);
                }
            }
        }
        out.sort_by(|a, b| (&a.id.owner_id, &a.id.name).cmp(&(&b.id.owner_id, &b.id.name)));
        Ok(out)
    }
    /// Return canonical bytes suitable for publishing.
    pub fn read_archive(&self, id: &SkillId) -> Result<Vec<u8>> {
        let _lock = self.lock()?;
        self.recover_install()?;
        read_bounded(&self.get_inner(id)?.archive)
    }
    /// Remove local storage only; existing project copies are independent.
    pub fn remove_local(&self, id: &SkillId) -> Result<()> {
        let _lock = self.lock()?;
        self.recover_install()?;
        let (archive, tree, metadata) = self.paths(id)?;
        ensure!(metadata.exists(), "skill is not installed");
        remove_path(&tree)?;
        remove_path(&archive)?;
        remove_path(&metadata)
    }
    /// Materialize an independent project copy; refuse unmanaged destinations.
    /// A supplied alias must equal the manifest name to preserve Agent Skills layout.
    pub fn clone_to_project(
        &self,
        project: &Path,
        id: &SkillId,
        alias: Option<&str>,
    ) -> Result<PathBuf> {
        self.attach(project, id, alias, false)
    }
    /// Link to the stable library tree; Windows may require Developer Mode or elevation.
    /// A supplied alias must equal the manifest name, never rename the installed skill.
    pub fn link_to_project(
        &self,
        project: &Path,
        id: &SkillId,
        alias: Option<&str>,
    ) -> Result<PathBuf> {
        self.attach(project, id, alias, true)
    }
    fn attach(
        &self,
        project: &Path,
        id: &SkillId,
        alias: Option<&str>,
        linked: bool,
    ) -> Result<PathBuf> {
        validate_id(id)?;
        let alias = alias.unwrap_or(&id.name);
        ensure!(
            alias == id.name,
            "project directory name must match SKILL.md name '{}'; omit --alias or use --alias {}",
            id.name,
            id.name
        );
        let _lock = self.lock()?;
        self.recover_install()?;
        let info = self.get_inner(id)?;
        let root = project.join(".agents/skills");
        fs::create_dir_all(&root)?;
        let _project_lock = project_lock(&root)?;
        let path = root.join(alias);
        let manifest_path = root.join(".mskill.json");
        let mut manifest = read_manifest(&manifest_path)?;
        ensure!(
            manifest
                .entries
                .get(alias)
                .is_none_or(|entry| entry.id == *id),
            "another publisher owns this project's skill name; use another project or explicitly remove the existing managed install first"
        );
        ensure!(
            fs::symlink_metadata(&path).is_err() || manifest.entries.contains_key(alias),
            "destination is unmanaged; preserve it and use another project, or explicitly move/remove it outside mskill"
        );
        let stage = root.join(".mskill-stage");
        remove_path(&stage)?;
        if linked {
            create_link(&info.directory, &stage).context(
                "cannot create directory link; use clone instead (Windows requires Developer Mode)",
            )?;
        } else {
            copy_tree(&info.directory, &stage)?;
        }
        replace(&stage, &path)?;
        manifest.entries.insert(
            alias.into(),
            ProjectEntry {
                id: id.clone(),
                sha256: info.sha256,
                linked,
            },
        );
        write_json(&manifest_path, &manifest)?;
        Ok(path)
    }
    /// Remove a managed project directory without following its symbolic link.
    pub fn remove_project(&self, project: &Path, alias: &str) -> Result<()> {
        mskill_protocol::validate_skill_name(alias)?;
        let root = project.join(".agents/skills");
        let _lock = project_lock(&root)?;
        let manifest_path = root.join(".mskill.json");
        let mut manifest = read_manifest(&manifest_path)?;
        ensure!(
            manifest.entries.remove(alias).is_some(),
            "project skill is unmanaged or not installed"
        );
        remove_path(&root.join(alias))?;
        write_json(&manifest_path, &manifest)
    }
}

/// Revalidate a wire identity before deriving filesystem paths.
fn validate_id(id: &SkillId) -> Result<()> {
    id.validate()?;
    Ok(())
}
/// Produce the standard lowercase digest used for latest-state comparison.
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
/// Bound archive memory before and after reading a potentially changing file.
fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    ensure!(
        fs::metadata(path)?.len() <= MAX_ARCHIVE_BYTES,
        "archive exceeds 16 MiB"
    );
    let bytes = fs::read(path)?;
    ensure!(
        bytes.len() as u64 <= MAX_ARCHIVE_BYTES,
        "archive exceeds 16 MiB"
    );
    Ok(bytes)
}
/// Check source manifests early before spending time compressing the directory.
fn frontmatter(text: &str) -> Result<Frontmatter> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.lines();
    ensure!(
        lines.next() == Some("---"),
        "SKILL.md must begin with YAML frontmatter"
    );
    let mut yaml = String::new();
    let mut ended = false;
    for line in lines {
        if line == "---" {
            ended = true;
            break;
        }
        yaml.push_str(line);
        yaml.push('\n');
    }
    ensure!(ended, "unterminated YAML frontmatter");
    let front: Frontmatter = serde_yaml::from_str(&yaml)?;
    mskill_protocol::validate_skill_name(&front.name)?;
    ensure!(
        !front.description.trim().is_empty(),
        "description must not be empty"
    );
    Ok(front)
}
/// Reject path reinterpretation instead of sanitizing attacker-controlled names.
fn safe_entry(name: &str) -> Result<PathBuf> {
    ensure!(
        !name.contains('\\') && !name.contains(':') && !name.contains('\0'),
        "non-portable ZIP path"
    );
    let path = PathBuf::from(name);
    ensure!(!path.is_absolute(), "absolute ZIP path");
    for component in path.components() {
        match component {
            Component::Normal(s) => {
                let s = s.to_string_lossy();
                ensure!(
                    !s.ends_with('.')
                        && !s.ends_with(' ')
                        && !s.contains(['<', '>', '"', '|', '?', '*']),
                    "non-portable ZIP filename"
                );
                let base = s.split('.').next().unwrap().to_ascii_uppercase();
                ensure!(
                    !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                        && !(base.len() == 4
                            && (base.starts_with("COM") || base.starts_with("LPT"))
                            && base.as_bytes()[3].is_ascii_digit()),
                    "reserved ZIP filename"
                );
            }
            _ => bail!("unsafe ZIP path"),
        }
    }
    ensure!(!path.as_os_str().is_empty(), "empty ZIP path");
    Ok(path)
}
/// Materialize previously validated entries into a fresh private staging directory.
fn extract(bytes: &[u8], target: &Path) -> Result<()> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let path = target.join(safe_entry(entry.name())?);
        if entry.is_dir() {
            fs::create_dir_all(path)?;
            continue;
        }
        fs::create_dir_all(path.parent().unwrap())?;
        let mut out = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        let size = entry.size();
        let copied = std::io::copy(&mut (&mut entry).take(size + 1), &mut out)?;
        ensure!(copied == size, "ZIP entry size mismatch");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if entry.unix_mode().unwrap_or(0) & 0o111 != 0 {
                0o755
            } else {
                0o644
            };
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
        }
    }
    Ok(())
}
/// Create reproducible portable bytes from a standard skill directory; symbolic links are rejected.
pub fn pack(source: &Path) -> Result<Vec<u8>> {
    Ok(ValidatedArchive::new(pack_unvalidated(source)?)?.bytes)
}

/// Produce ZIP bytes without claiming validation; only private checked callers use this.
fn pack_unvalidated(source: &Path) -> Result<Vec<u8>> {
    frontmatter(&fs::read_to_string(source.join("SKILL.md"))?)?;
    let mut files = Vec::new();
    collect_files(source, source, &mut files)?;
    files.sort();
    ensure!(files.len() <= MAX_ENTRIES, "too many skill files");
    let mut total = 0u64;
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for path in files {
        let name = path
            .strip_prefix(source)?
            .to_string_lossy()
            .replace('\\', "/");
        safe_entry(&name)?;
        let size = fs::metadata(&path)?.len();
        total = total.checked_add(size).context("skill size overflow")?;
        ensure!(total <= MAX_EXPANDED_BYTES, "skill exceeds 64 MiB");
        let mode = portable_mode(&path)?;
        zip.start_file(
            name,
            SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(mode),
        )?;
        let copied = std::io::copy(&mut File::open(path)?.take(size + 1), &mut zip)?;
        ensure!(copied == size, "source changed while packaging; retry");
    }
    Ok(zip.finish()?.into_inner())
}

/// Pack a directory or read validated portable archive bytes without changing local state.
///
/// Callers updating an explicit identity should validate `validate_archive(&bytes).name`
/// against that identity before calling `install_bytes`; the latter also enforces it.
pub fn pack_or_read(source: &Path) -> Result<Vec<u8>> {
    Ok(validated_source(source)?.bytes)
}

/// Acquire owned source bytes and attach one complete inspection for the whole lifecycle.
fn validated_source(source: &Path) -> Result<ValidatedArchive<Vec<u8>>> {
    let bytes = if source.is_dir() {
        pack_unvalidated(source)?
    } else {
        read_bounded(source)?
    };
    ValidatedArchive::new(bytes)
}
/// Preserve only executable intent; never carry ownership or privileged mode bits.
fn portable_mode(path: &Path) -> Result<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return Ok(if fs::metadata(path)?.permissions().mode() & 0o111 != 0 {
            0o755
        } else {
            0o644
        });
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(0o644)
    }
}
/// Enumerate regular files and reject symlinks rather than follow external content.
fn collect_files(root: &Path, path: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        ensure!(!ty.is_symlink(), "skill must not contain symbolic links");
        if ty.is_dir() {
            collect_files(root, &entry.path(), out)?;
        } else {
            ensure!(ty.is_file(), "skill contains a special file");
            out.push(entry.path());
            ensure!(out.len() <= MAX_ENTRIES, "too many skill files");
        }
    }
    let _ = root;
    Ok(())
}
/// Copy the validated library tree without dereferencing unexpected symbolic links.
fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        ensure!(
            !ty.is_symlink(),
            "library contains unexpected symbolic link"
        );
        let dest = target.join(entry.file_name());
        if ty.is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else {
            ensure!(ty.is_file(), "library contains special file");
            fs::copy(entry.path(), dest)?;
        }
    }
    Ok(())
}
/// Remove a path itself; symbolic links must never expand into recursive targets.
fn remove_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
                const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
                let attributes = meta.file_attributes();
                // Reparse-point attributes classify directory symlinks and
                // junctions even when dangling. Never recurse into these paths.
                if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                    if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
                        fs::remove_dir(path)?;
                    } else {
                        fs::remove_file(path)?;
                    }
                    return Ok(());
                }
            }
            if meta.is_dir() && !meta.file_type().is_symlink() {
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}
/// Retain the old path until a same-filesystem staged rename succeeds.
fn replace(stage: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target.parent().unwrap())?;
    let backup = target.with_extension("mskill-backup");
    remove_path(&backup)?;
    let exists = fs::symlink_metadata(target).is_ok();
    if exists {
        fs::rename(target, &backup)?;
    }
    if let Err(e) = fs::rename(stage, target) {
        if exists {
            let _ = fs::rename(&backup, target);
        }
        return Err(e.into());
    }
    remove_path(&backup)
}
/// Flush a complete staged JSON document before replacing its visible path.
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::create_dir_all(path.parent().unwrap())?;
    let stage = path.with_extension("mskill-write");
    let mut file = File::create(&stage)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    replace(&stage, path)
}
/// Distinguish an absent project manifest from a corrupt one.
fn read_manifest(path: &Path) -> Result<ProjectManifest> {
    if !path.exists() {
        return Ok(ProjectManifest::default());
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
/// Serialize mutations per project, including clients with different library roots.
fn project_lock(root: &Path) -> Result<File> {
    fs::create_dir_all(root)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.join(".mskill.lock"))?;
    file.lock_exclusive()?;
    Ok(file)
}
#[cfg(unix)]
fn create_link(source: &Path, target: &Path) -> Result<()> {
    Ok(std::os::unix::fs::symlink(source, target)?)
}
#[cfg(windows)]
fn create_link(source: &Path, target: &Path) -> Result<()> {
    Ok(std::os::windows::fs::symlink_dir(source, target)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All public boundaries validate, while internal author/import paths reuse proof.
    #[test]
    fn archive_validation_is_reused_without_skipping_boundaries() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.temp")
            .join(format!("core-validation-{}", std::process::id()));
        remove_path(&root)?;
        let source = root.join("source");
        fs::create_dir_all(&source)?;
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo skill\n---\n# Demo\n",
        )?;
        let store = LocalStore::open(root.join("home"))?;
        VALIDATION_PASSES.with(|count| count.set(0));
        let bytes = pack(&source)?;
        assert_eq!(VALIDATION_PASSES.with(|count| count.get()), 1);
        assert_eq!(bytes, pack_unvalidated(&source)?);
        VALIDATION_PASSES.with(|count| count.set(0));
        assert_eq!(pack_or_read(&source)?, bytes);
        assert_eq!(VALIDATION_PASSES.with(|count| count.get()), 1);
        let archive = root.join("demo.skill");
        fs::write(&archive, &bytes)?;
        VALIDATION_PASSES.with(|count| count.set(0));
        assert_eq!(pack_or_read(&archive)?, bytes);
        assert_eq!(VALIDATION_PASSES.with(|count| count.get()), 1);
        VALIDATION_PASSES.with(|count| count.set(0));
        let info = store.add(&source)?;
        assert_eq!(VALIDATION_PASSES.with(|count| count.get()), 1);
        VALIDATION_PASSES.with(|count| count.set(0));
        assert_eq!(store.add(&archive)?.sha256, info.sha256);
        assert_eq!(VALIDATION_PASSES.with(|count| count.get()), 1);
        VALIDATION_PASSES.with(|count| count.set(0));
        store.install_bytes(&info.id, &bytes)?;
        assert_eq!(VALIDATION_PASSES.with(|count| count.get()), 1);
        assert!(store.install_bytes(&info.id, b"not a zip").is_err());
        let mismatched = SkillId::new("local", "different")?;
        assert!(store.install_bytes(&mismatched, &bytes).is_err());
        assert!(store.get(&mismatched).is_err());
        assert_eq!(store.read_archive(&info.id)?, bytes);
        remove_path(&root)
    }

    /// Removing live and dangling directory links never touches target contents.
    #[test]
    fn directory_links_remove_without_following() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.temp")
            .join(format!("core-links-{}", std::process::id()));
        remove_path(&root)?;
        let target = root.join("target");
        fs::create_dir_all(&target)?;
        fs::write(target.join("keep"), "target content")?;
        let live = root.join("live");
        // Platforms lacking Windows symlink privilege are covered by the
        // CLI's actionable clone fallback instead of weakening removal checks.
        if let Err(error) = create_link(&target, &live) {
            #[cfg(windows)]
            {
                if error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.raw_os_error() == Some(1314))
                {
                    remove_path(&root)?;
                    return Ok(());
                }
            }
            return Err(error);
        }
        remove_path(&live)?;
        assert!(fs::symlink_metadata(&live).is_err());
        assert_eq!(fs::read_to_string(target.join("keep"))?, "target content");
        let dangling = root.join("dangling");
        create_link(&target, &dangling)?;
        remove_path(&target)?;
        remove_path(&dangling)?;
        assert!(fs::symlink_metadata(&dangling).is_err());
        remove_path(&root)
    }
    #[test]
    fn portable_paths_reject_escape() {
        for name in [
            "../escape",
            "/absolute",
            "C:/drive",
            "a\\b",
            "NUL.txt",
            "a/../../b",
        ] {
            assert!(safe_entry(name).is_err(), "{name}");
        }
        assert!(safe_entry("references/help.md").is_ok());
    }
    #[test]
    /// Check source manifests early before spending time compressing the directory.
    fn frontmatter_requires_contract() {
        assert!(frontmatter("---\nname: demo\ndescription: useful\n---\n# Demo").is_ok());
        assert!(frontmatter("# Demo").is_err());
        assert!(frontmatter("---\nname: ../bad\ndescription: useful\n---").is_err());
    }

    /// Exercise authoring, unchanged imports, project copies, updates and safe removal.
    #[test]
    fn local_project_lifecycle() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.temp")
            .join(format!("core-lifecycle-{}", std::process::id()));
        remove_path(&root)?;
        let source = root.join("source");
        fs::create_dir_all(source.join("references"))?;
        fs::write(
            source.join("SKILL.md"),
            "---\nname: demo\ndescription: Demo skill\n---\n# Demo\n",
        )?;
        fs::write(source.join("references/help.md"), "first")?;
        let store = LocalStore::open(root.join("home"))?;
        let first = store.add(&source)?;
        assert_eq!(store.add(&source)?.sha256, first.sha256);
        let project = root.join("project");
        let dest = store.clone_to_project(&project, &first.id, None)?;
        assert_eq!(
            fs::read_to_string(dest.join("references/help.md"))?,
            "first"
        );
        fs::write(source.join("references/help.md"), "second")?;
        let second = store.add(&source)?;
        assert_ne!(first.sha256, second.sha256);
        assert_eq!(store.list()?.len(), 1);
        assert_eq!(
            fs::read_to_string(dest.join("references/help.md"))?,
            "first"
        );
        store.clone_to_project(&project, &first.id, None)?;
        assert_eq!(
            fs::read_to_string(dest.join("references/help.md"))?,
            "second"
        );
        let cloud = SkillId::new("publisher_123", "demo")?;
        store.install_bytes(&cloud, &store.read_archive(&first.id)?)?;
        assert!(store.clone_to_project(&project, &cloud, None).is_err());
        let cloud_project = root.join("cloud-project");
        assert!(store
            .clone_to_project(&cloud_project, &cloud, Some("cloud-demo"))
            .is_err());
        assert!(store
            .link_to_project(&cloud_project, &cloud, Some("cloud-demo"))
            .is_err());
        assert!(
            !cloud_project.exists(),
            "invalid aliases must not mutate the project"
        );
        let cloud_dest = store.clone_to_project(&cloud_project, &cloud, Some("demo"))?;
        assert_eq!(
            cloud_dest.file_name().and_then(|name| name.to_str()),
            Some("demo")
        );
        let unmanaged_project = root.join("unmanaged-project");
        let unmanaged = unmanaged_project.join(".agents/skills/demo");
        fs::create_dir_all(&unmanaged)?;
        assert!(store
            .clone_to_project(&unmanaged_project, &cloud, None)
            .is_err());
        assert!(store.remove_project(&unmanaged_project, "demo").is_err());
        assert!(unmanaged.is_dir());
        store.remove_project(&project, "demo")?;
        assert!(!dest.exists());
        store.remove_local(&first.id)?;
        assert!(store.get(&first.id).is_err());
        assert!(cloud_dest.join("SKILL.md").exists());
        remove_path(&root)
    }

    /// Simulate interruption after replacing the tree but before replacing archive bytes.
    #[test]
    fn interrupted_install_rolls_back() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.temp")
            .join(format!("core-recovery-{}", std::process::id()));
        remove_path(&root)?;
        let store = LocalStore::open(&root)?;
        let id = SkillId::new("local", "demo")?;
        let (_, tree, _) = store.paths(&id)?;
        fs::create_dir_all(&tree)?;
        fs::write(tree.join("old"), "original")?;
        let stage = root.join(".temp/install");
        fs::create_dir_all(&stage)?;
        write_json(
            &stage.join("intent.json"),
            &InstallIntent {
                id,
                existed: [true, false, false],
            },
        )?;
        fs::rename(&tree, stage.join("backup-0"))?;
        fs::create_dir_all(&tree)?;
        fs::write(tree.join("new"), "partial")?;
        drop(store);
        LocalStore::open(&root)?;
        assert!(tree.join("old").exists());
        assert!(!tree.join("new").exists());
        remove_path(&root)
    }
}
