# Local store and project lifecycle

## Contract and layout

`mskill-core` accepts a CLI-selected root, normally `~/.mskill`. Identity is
`owner_id/name` and all path components use `mskill-protocol` validation. Local
authoring uses owner `local`; publisher IDs are immutable opaque identifiers.
The library keeps only current state:

```
~/.mskill/
  archives/<owner>/<name>.skill   # canonical portable ZIP bytes
  library/<owner>/<name>/        # stable extracted tree used by links
  metadata/<owner>/<name>.json   # current identity, digest and description
  .lock                         # cross-process library serialization
  .temp/install/                # staging and recovery intent, not versions
```

`add` packs a directory whose root contains `SKILL.md`, or imports a `.skill` ZIP.
`install_bytes` handles cloud downloads under their actual publisher namespace.
A matching SHA-256 skips replacement; changed bytes replace the latest tree and
archive. Archives are reproducible for unchanged source bytes, file names and
executable-bit semantics. Files are sorted, ZIP timestamps use the ZIP library's
fixed default, and permissions normalize to portable 0644/0755. Directory source
symlinks and special files are rejected rather than dereferenced.

## Safety and transaction boundaries

The shared protocol inspector validates every ZIP entry and streams all bytes to
check decompression/CRC before local state changes. Limits are 16 MiB compressed,
64 MiB expanded, 4,096 entries and 1 MiB for root `SKILL.md`. Only regular files
and directories are accepted. Absolute paths, traversal, Windows separators,
reserved device names, duplicate/case-colliding entries, encrypted entries and
special file modes are rejected. Extraction always targets a newly created staging
tree with create-new writes, never an existing skill directory.

Library writes serialize with an OS file lock. Install stages tree/archive/metadata,
persists an intent, then renames targets with retained backups. An ordinary failure
rolls back, and the next open recovers an interrupted install. A committed marker
permits cleanup after interruption. These temporary backups are transaction recovery,
not user-visible versions. This gives failure recovery and atomic individual renames;
it is not a claim of power-loss durability across an entire multi-file filesystem
transaction. Linked readers outside the CLI may briefly see a missing directory
during replacement, especially on Windows where rename-over-directory is unavailable.

Metadata filesystem paths are derived from the current store root rather than
trusted from persisted JSON, allowing a library to move between machines.

## Project semantics

Project destination is `.agents/skills/<name>` for every publisher. Directory names
must match the root `SKILL.md` name under the
[Agent Skills specification](https://agentskills.io/specification); `--alias` remains
accepted by the API/CLI but must equal that canonical name, before project mutation.
No manifest rewriting or alias view is introduced.
`.agents/skills/.mskill.json` maps installed names to publisher identity, digest and
copy/link mode. A different publisher using the same name is refused: use another
project, or explicitly remove the existing managed install before attaching the other.
Existing unmanaged paths are never overwritten or removed.
A project-specific OS lock serializes mutations even between different local stores.
Copies remain unchanged until explicitly refreshed and survive local deletion.
Links follow the stable library path and see updates; local deletion leaves them
dangling. Removing a project link removes the link itself, never its library target.
Windows directory symlinks may require Developer Mode; failure advises using clone.

Project directory replacement uses rollback-safe renames, and manifests use staged
writes. Unlike library installation, directory + project manifest are not journaled
together: an interrupted first attach may leave a destination that the next command
correctly treats as unmanaged. It must be inspected and moved/removed explicitly,
rather than silently claimed. Project copies can be edited, but a subsequent explicit
clone refresh overwrites their content; authors should edit library source instead.

## Focused verification

Core unit scenarios cover portable path/frontmatter rejection, unchanged imports,
latest replacement, independent copies, refresh, namespace collisions, canonical
name enforcement before mutation, explicit canonical alias compatibility,
unmanaged-path protection, local/project removal and simulated interrupted-install
rollback. Test files live under repository `.temp`. CLI integration owns actual
command execution and cross-platform link testing; this module does not equate unit
coverage with a complete user journey.

## Design references

- [zip-rs upstream](https://github.com/zip-rs/zip2): interoperable ZIP packaging.
- [ZipFile enclosed_name](https://docs.rs/zip/2.4.2/zip/read/struct.ZipFile.html#method.enclosed_name): preserve path meaning and reject escape instead of sanitizing it.
- [Upstream extraction advisory](https://github.com/zip-rs/zip2/security/advisories/GHSA-94vh-gphv-8pm8): path checks alone do not prevent symlink-assisted archive escape; this format rejects all ZIP symlinks.

A content digest is change detection, not publisher authentication or proof that a
skill's instructions are trustworthy. Package management never executes package code.

### Windows unlink correction (2026-10-05)

The first real local CLI journey completed pack/export/add/clone/link/update but
failed directory-link removal with Windows error 5 (`Access denied`), captured in
`.temp/e2e/results.json`. `symlink_metadata(...).is_dir()` does not classify a
Windows directory symlink as a directory, so the old helper selected `remove_file`.
The corrected helper uses Windows `MetadataExt::file_attributes()` to detect directory reparse points and calls
nonrecursive `remove_dir`, including for dangling directory links. It does not
query/follow the target. A focused regression scenario checks that unlink preserves
target contents and that removing a broken link succeeds. The existing CLI journey
must be rerun by integration after rebuilding; no standalone Cargo run was added.
Reference: [Rust Windows MetadataExt](https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html).


### Reuse validated archive inspection (2026-10-05)

The actual CLI baseline showed unchanged adds spending about 163.1 ms on a
768-file fixture and 162.3 ms on a 6 MiB compressible archive, against a 22 ms
process floor (five runs); directory add on the latter fixture was 365.4 ms.
Performance measurement is owned by the performance agent; these values are the
pre-change baseline, not claimed post-change improvements.

The importer now carries a private `ValidatedArchive<B>` coupling immutable owned
or borrowed bytes with the shared `ArchiveInspection`. Its only constructor invokes
`mskill_protocol::validate_archive`. The private source and installation paths reuse
that inspection instead of calling public validating helpers repeatedly. Public
`install_bytes` still fully validates untrusted input, including unchanged digests,
and public `pack` / `pack_or_read` still return fully validated bytes. There is no
public trusted-input bypass, hash-based validation skip, or separate weaker validator.

| Entry path | Previous complete ZIP inspections | Current complete ZIP inspections |
|---|---:|---:|
| `add` from an archive | 3 | 1 |
| `add` from a directory | 4 | 1 |
| `pack_or_read` from a directory (CLI pack path) | 2 | 1 |
| public `pack` | 1 | 1 |
| public `pack_or_read` from an archive | 1 | 1 |
| public `install_bytes` | 1 | 1 |

Extracting changed archives still requires a subsequent decompression pass to write
files; the table counts complete pre-commit inspections, not materialization. The ZIP
writer, order, timestamp, permissions, compression options, archive bytes and digest
algorithm are unchanged. A test-only thread-local counter checks these pass counts;
regression assertions preserve malformed-input rejection, identity matching, exact
packaging bytes and exact stored bytes. Rebuild, the real CLI journey and before/after
hash/performance comparisons are delegated to integration, avoiding duplicate builds.
