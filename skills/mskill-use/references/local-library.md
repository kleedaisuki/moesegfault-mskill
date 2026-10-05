# Local library and projects

`mskill` stores its library under `~/.mskill`. Use global `--home DIR` or `MSKILL_HOME` to choose another library. Local imports have identity `local/name`; downloaded packages have identity `owner/name`. Use the full identity if the short name is ambiguous.

## Import and install

```sh
mskill add ./skills/my-skill
mskill add ./my-skill.skill
mskill list
mskill clone local/my-skill --project .
```

`add` imports a skill directory or `.skill` archive. Importing the same name replaces its current local package when its hash changes; an identical package is a no-op. `clone` installs a snapshot beneath the selected project's `.agents/skills`; it is independent of later library updates. The installed directory name must match the `name` in `SKILL.md`; do not rename it or edit its manifest to resolve a collision.

```sh
mskill link local/my-skill --project ./another-project
```

To compare a copy and a live link, use separate projects as above. In a single project choose one mode; explicitly remove its installation before switching modes.

`link` makes the project's directory a symbolic link to the library's extracted skill directory. Library changes are visible through the link. On Windows, creating symbolic links may require Developer Mode or suitable privileges; use `clone` when links are unavailable. Do not elevate privileges just to make a link when a copy meets the user's goal.

The CLI protects unmanaged project directories from replacement and does not overwrite an installation belonging to another publisher. Both local and cloud packages use the canonical skill name for the project folder. To use a different publisher of the same name, choose another project or explicitly remove the current managed installation before installing the requested publisher. Do not bypass protection by deleting a user-owned directory. The retained `--alias` option accepts only the canonical skill name; it cannot rename installations.

## Share or back up an archive

```sh
mskill pack ./skills/my-skill -o ./my-skill.skill
mskill export local/my-skill -o ./my-skill-backup.skill
```

`pack` creates a package directly from a directory without installing it into the library. `export` writes an installed skill's canonical package. Both require a `.skill` output extension and refuse to overwrite an existing file; choose a new path rather than deleting another file without authorization. Use `add` to import the resulting archive on another device.

## Update

```sh
mskill update local/my-skill --from ./skills/my-skill
mskill update publisher-id/my-skill
mskill update
```

Local skills require `--from PATH`; registry-installed skills fetch their cloud source. With no skill argument, `update` refreshes registry-installed skills. SHA-256 determines whether package bytes changed; only the latest package is retained. A link uses the library's current contents; a clone remains a snapshot until explicitly refreshed by cloning again.

## Remove only the requested scope

```sh
mskill remove my-skill --scope project --project .
mskill remove local/my-skill --scope local
```

Project removal takes the canonical skill name and is the default scope. Local removal takes a library identity; existing project copies remain, while a project link may become unusable if its library target is removed. Registry deletion is a separate publishing operation.

Use `--json` for machine-readable output and `--verbose` for diagnostics. Color is for interactive terminals and is disabled for redirected output. Avoid treating diagnostic text as the command's data contract.
