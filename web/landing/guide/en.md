# mskill · Reuse Agent Skills across projects

Manage a local Skill library with mskill. Download, publish, copy and link Agent Skills using portable .skill packages and latest-only updates.

## Install the CLI

Extract the download and put mskill (mskill.exe on Windows) on your PATH. Packages include licenses; checksum files are provided alongside the archives.

- [Windows x86_64](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest/download/mskill-windows-x86_64.tar.gz)
- [Linux x86_64](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest/download/mskill-linux-x86_64.tar.gz)
- [macOS Apple Silicon](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest/download/mskill-macos-aarch64.tar.gz)

[All releases and checksum files](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest)

## Add to your library / Install in a project

Replace ./my-skill with your Skill directory. Its directory name must match the name in SKILL.md.

```sh
mskill add ./my-skill
mskill clone local/my-skill --project .
```

This creates .agents/skills/my-skill. Use link instead to follow library changes. Windows links may require Developer Mode.

Before switching to a link, explicitly remove the same-named installation from that project. Never overwrite an unmanaged directory.

```sh
mskill remove my-skill --scope project --project .
mskill link local/my-skill --project .
```

## Download and update

Replace owner/name with the full publication identity shown in the workspace. Public downloads need no login.

```sh
mskill pull owner/name
mskill clone owner/name --project .
mskill update owner/name
```

## Publish from the CLI

```sh
mskill add ./my-skill
mskill login
mskill publish local/my-skill
```

Publishing again replaces the current package. Keep your own backups if you need history.

## Preserve operation scope

Project, local-library and cloud removal are separate operations. Perform only the removal the user explicitly requests. A cloud identity must belong to the signed-in account. Project copies remain; deleting local-library content can break existing links.

```sh
mskill remove my-skill --scope project --project .
mskill remove local/my-skill --scope local
mskill remove my-owner-id/my-skill --scope cloud
```

## Before you begin

### Do I need an account?

Not for local management or public downloads. Publishing, managing cloud content and participating in the community require a moeSegFault account.

### Are old versions kept?

No. Each publication name keeps only its latest package. update replaces local content when SHA-256 changes. Project copies do not update automatically; links follow the local library.

### Can I run downloaded content immediately?

Review the Skill first. Downloading or installing does not authorize running bundled scripts or following its instructions.

- [Open the Skill workspace](https://skills.moesegfault.dev/en/)
- [Documentation](https://github.com/kleedaisuki/moesegfault-mskill#readme)
- [Privacy](https://skills.moesegfault.dev/en/privacy)
- [Terms](https://skills.moesegfault.dev/en/terms)
