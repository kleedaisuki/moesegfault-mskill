# mskill

Reuse skills across projects without keeping a separate copy by hand.

`mskill` maintains a local library in `~/.mskill`, distributes skills as portable ZIP archives with a `.skill` extension, and installs ordinary skill directories into a project's `.agents/skills`. The registry is `skills.moesegfault.dev`, with account-scoped names such as `publisher-id/code-review`.

## Websites

- [mskill.moesegfault.dev](https://mskill.moesegfault.dev): product introduction, CLI downloads, installation examples, and agent-readable guides.
- [skills.moesegfault.dev](https://skills.moesegfault.dev): browse the community, read skill instructions and resources, download packages, and view public discussions without signing in. Sign in with your moeSegFault account to publish or manage your packages and participate in comments.

Both websites provide Chinese, Japanese, and English interfaces and light, dark, or system appearance. Browser publication uses the same account namespace as the CLI. To upload a source directory through the website, first package it with `mskill pack`; the upload accepts portable `.skill` archives.

## Install the CLI

Download the package for your operating system from [GitHub Releases](https://github.com/kleedaisuki/moesegfault-mskill/releases/latest), extract it, and put `mskill` (`mskill.exe` on Windows) on your `PATH`. Release packages include the application license, upstream dependency license notices, and a SHA-256 checksum file.

### Build from source

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

This copies the skill into `.agents/skills/mskill-use`. Choose either `clone` for a snapshot or `link` for live local-library contents in the project. To switch the example above to a link, first remove its project copy:

```sh
mskill remove mskill-use --scope project --project .
mskill link local/mskill-use --project .
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

Publishing signs in with moeSegFault Identity and uses your mapped account namespace. Different accounts can publish the same skill name. The official service is configured out of the box: run `mskill login` without setting a client ID. In these examples, replace `publisher-id` with the publisher namespace shown by `whoami`, `publish`, or in the registry listing.

## Update and remove

```sh
mskill update publisher-id/code-review
mskill update local/mskill-use --from ./skills/mskill-use
mskill update

mskill remove mskill-use --scope project --project .
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

Project removal is the default removal scope and uses the skill name. Installed directories keep the name declared in `SKILL.md`, as required by the [Agent Skills specification](https://agentskills.io/specification). Use full `owner/name` identities to distinguish library packages with the same name. A project can install only one publisher of a given skill name: use another project, or explicitly remove its current installation before installing the other publisher. The CLI does not overwrite unmanaged project skill directories.

## Configuration

| Option / environment variable | Purpose |
| --- | --- |
| `--home DIR` / `MSKILL_HOME` | Local library root; default `~/.mskill` |
| `--registry URL` / `MSKILL_REGISTRY` | Registry endpoint |
| `--client-id ID` / `MSKILL_OIDC_CLIENT_ID` | Optional override; the official service uses the registered `mskill-cli` client |
| `--issuer URL` / `MSKILL_OIDC_ISSUER` | Identity issuer |
| `--redirect-uri URL` / `MSKILL_OIDC_REDIRECT_URI` | Identity callback URL |
| `--json` | Machine-readable output |
| `--verbose` | Diagnostic output |

Run `mskill --help` or `mskill COMMAND --help` for the installed CLI's full options. Interactive output uses color; pipes and unsupported terminals receive plain output.

For a staging or custom registry, explicitly configure its Identity client. The official client default applies only to the official registry and issuer together; changing either requires an explicit client ID.

```sh
mskill --registry https://skills-staging.moesegfault.dev \
  --issuer https://identity-staging.moesegfault.dev \
  --client-id mskill-cli-staging login
```

Keep the same configuration for subsequent account and publication commands, or set the corresponding environment variables for that session.

## Maintained skills and distribution

- [Use mskill](skills/mskill-use/SKILL.md): local library, project installation, downloads, and updates.
- [Publish with mskill](skills/mskill-publish/SKILL.md): account and publication workflows.
- [Distribution responsibilities](docs/distribution-policy.md): licensing, secrets, latest-only storage, and deletion limits.

Treat downloaded skills as untrusted content until reviewed. Installing a package does not authorize running its scripts or following its instructions.

## Development and deployment

[Build and deployment](docs/delivery.md) covers the Worker, CI, Cloudflare environments, and observability. [Identity onboarding](docs/identity-onboarding.md) covers account client setup. [User-journey testing](docs/e2e.md) describes the CLI-to-Worker acceptance workflow.
