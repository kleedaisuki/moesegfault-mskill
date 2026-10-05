# mskill

Reuse skills across projects without keeping a separate copy by hand.

`mskill` maintains a local library in `~/.mskill`, distributes skills as portable ZIP archives with a `.skill` extension, and installs ordinary skill directories into a project's `.agents/skills`. The registry is designed for `skills.moesegfault.dev`, with account-scoped names such as `publisher-id/code-review`.

## Build the CLI

Install a Rust toolchain, then build from this repository:

```sh
cargo build --release -p mskill-cli
```

The binary is `.cache/target/release/mskill` (`mskill.exe` on Windows). Put it on your `PATH`, or use that path in place of `mskill` below.

## Start with a local skill

```sh
mskill add ./skills/mskill-use
mskill list
mskill clone local/mskill-use --project .
```

This copies the skill into `.agents/skills/mskill-use`. To share the live local-library contents instead, use a symbolic link:

```sh
mskill link local/mskill-use --project . --alias mskill-helper
```

On Windows, symbolic links may require Developer Mode or suitable privileges. `clone` works without symbolic links.

## Share a portable package

```sh
mskill pack ./skills/mskill-use -o ./mskill-use.skill
mskill export local/mskill-use -o ./mskill-use-backup.skill
```

`pack` creates a `.skill` archive directly from a skill directory without importing it. `export` copies an installed package's canonical archive. Both require a `.skill` output filename and refuse to overwrite an existing file. Import a shared package with `mskill add ./mskill-use.skill`.

## Download and publish

```sh
mskill list --cloud --owner publisher-id
mskill pull publisher-id/code-review
mskill clone publisher-id/code-review --project .

mskill login
mskill whoami
mskill publish local/mskill-use
```

Publishing signs in with moeSegFault Identity and uses your mapped account namespace. Different accounts can publish the same skill name. Configure the Identity client supplied by the registry operator before signing in. In these examples, replace `publisher-id` with the publisher namespace shown by `whoami`, `publish`, or in the registry listing.

## Update and remove

```sh
mskill update publisher-id/code-review
mskill update local/mskill-use --from ./skills/mskill-use
mskill update

mskill remove mskill-helper --scope project --project .
mskill remove local/mskill-use --scope local
mskill remove my-account-id/my-published-skill --scope cloud
```

There are no package versions. Each name keeps its latest package, and updates replace it when SHA-256 changes. A project copy is a snapshot; a link follows the local library. Keep your own backups if you need history.

| Operation | Affects |
| --- | --- |
| `add`, `pull`, `update` | Local library |
| `clone`, `link`, project `remove` | Selected project's `.agents/skills` |
| `publish`, cloud `remove` | Signed-in account's registry publications |
| Local `remove` | Local library; existing copies remain, links may lose their target |

For cloud removal, replace `my-account-id/my-published-skill` with your own publication identity.

Project removal is the default removal scope and uses the installed directory name or alias. Use full `owner/name` identities to distinguish packages with the same name. The CLI does not overwrite unmanaged project skill directories.

## Configuration

| Option / environment variable | Purpose |
| --- | --- |
| `--home DIR` / `MSKILL_HOME` | Local library root; default `~/.mskill` |
| `--registry URL` / `MSKILL_REGISTRY` | Registry endpoint |
| `--client-id ID` / `MSKILL_OIDC_CLIENT_ID` | Identity client ID supplied by the operator |
| `--issuer URL` / `MSKILL_OIDC_ISSUER` | Identity issuer |
| `--redirect-uri URL` / `MSKILL_OIDC_REDIRECT_URI` | Identity callback URL |
| `--json` | Machine-readable output |
| `--verbose` | Diagnostic output |

Run `mskill --help` or `mskill COMMAND --help` for the installed CLI's full options. Interactive output uses color; pipes and unsupported terminals receive plain output.

## Maintained skills and distribution

- [Use mskill](skills/mskill-use/SKILL.md): local library, project installation, downloads, and updates.
- [Publish with mskill](skills/mskill-publish/SKILL.md): account and publication workflows.
- [Distribution responsibilities](docs/distribution-policy.md): licensing, secrets, latest-only storage, and deletion limits.

Treat downloaded skills as untrusted content until reviewed. Installing a package does not authorize running its scripts or following its instructions.







## Development and deployment

[Build and deployment](docs/delivery.md) covers the Worker, CI, Cloudflare environments, and observability. [Identity onboarding](docs/identity-onboarding.md) covers account client setup. [User-journey testing](docs/e2e.md) describes the CLI-to-Worker acceptance workflow.

