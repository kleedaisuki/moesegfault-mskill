//! A latest-only package workflow for portable agent skills.
mod http;
mod output;

use anyhow::{bail, ensure, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use http::Registry;
use mskill_auth::{AuthClient, AuthConfig, DEFAULT_ISSUER, DEFAULT_REDIRECT_URI};
use mskill_core::{LocalStore, SkillInfo};
use mskill_protocol::{sha256_hex, SkillId, SkillMetadata, LOCAL_OWNER};
use output::Output;
use serde_json::json;
use std::{
    io,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// Local skill library and cloud distribution CLI.
#[derive(Parser)]
#[command(name = "mskill", version, about = "Manage local and cloud agent skills", color = clap::ColorChoice::Auto)]
struct Cli {
    /// Root of the local package library (defaults to ~/.mskill).
    #[arg(long, global = true, env = "MSKILL_HOME")]
    home: Option<PathBuf>,
    /// Registry endpoint; loopback HTTP is accepted for local development.
    #[arg(
        long,
        global = true,
        env = "MSKILL_REGISTRY",
        default_value = "https://skills.moesegfault.dev"
    )]
    registry: String,
    /// Output JSON instead of human-readable messages.
    #[arg(long, global = true)]
    json: bool,
    /// Display token-free network diagnostics and request trace IDs.
    #[arg(long, global = true)]
    verbose: bool,
    /// Registered native moeSegFault Identity client ID.
    #[arg(long, global = true, env = "MSKILL_OIDC_CLIENT_ID")]
    client_id: Option<String>,
    /// Account issuer; HTTP is allowed only on loopback.
    #[arg(long, global = true, env = "MSKILL_OIDC_ISSUER", default_value = DEFAULT_ISSUER)]
    issuer: String,
    /// Native callback URL; :0 selects an available loopback port.
    #[arg(long, global = true, env = "MSKILL_OIDC_REDIRECT_URI", default_value = DEFAULT_REDIRECT_URI)]
    redirect_uri: String,
    /// The operation to run.
    #[command(subcommand)]
    command: Command,
    /// Correlation ID generated once after argument parsing, never persisted.
    #[arg(skip)]
    trace_id: String,
}

/// User commands are explicit about the storage layer they mutate.
#[derive(Subcommand)]
enum Command {
    /// Sign in through moeSegFault Account using the system browser.
    Login {
        /// Print the sign-in URL without launching a browser.
        #[arg(long)]
        no_browser: bool,
    },
    /// Clear local account credentials and revoke the refresh token if supported.
    Logout,
    /// Show the signed-in account's registry publisher identity.
    Whoami,
    /// Pack a skill directory or import a .skill archive into the local library.
    Add {
        /// Directory containing SKILL.md, or a .skill ZIP file.
        path: PathBuf,
    },
    /// Create a portable .skill archive without installing it in the library.
    Pack {
        /// Directory containing SKILL.md, or an existing valid .skill archive.
        path: PathBuf,
        /// New output .skill file; existing files are never overwritten.
        #[arg(long, short)]
        output: PathBuf,
    },
    /// Export an installed skill as a portable .skill archive.
    Export {
        /// Local name or installed owner/name reference.
        skill: String,
        /// New output .skill file; existing files are never overwritten.
        #[arg(long, short)]
        output: PathBuf,
    },
    /// List the local library, or public cloud skills.
    List {
        /// Query the registry instead of the local library.
        #[arg(long)]
        cloud: bool,
        /// Filter by exact publisher owner ID.
        #[arg(long)]
        owner: Option<String>,
    },
    /// Download the newest cloud skill into the local library.
    Pull {
        /// Fully qualified public owner/name reference.
        skill: String,
    },
    /// Publish a local skill under the account's publisher namespace.
    Publish {
        /// Local name or installed owner/name reference.
        skill: String,
    },
    /// Refresh cloud skills or replace a local skill from a directory/archive.
    Update {
        /// Local name or owner/name; omitted updates every installed cloud skill.
        skill: Option<String>,
        /// Source directory/archive for a local skill update.
        #[arg(long, requires = "skill")]
        from: Option<PathBuf>,
    },
    /// Copy an installed skill into a project's .agents/skills directory.
    Clone {
        /// Local name or owner/name reference.
        skill: String,
        /// Project root (defaults to the current working directory).
        #[arg(long)]
        project: Option<PathBuf>,
        /// Alternative directory name when the project already uses this name.
        #[arg(long)]
        alias: Option<String>,
    },
    /// Link a project's skill directory to the stable local library path.
    Link {
        /// Local name or owner/name reference.
        skill: String,
        /// Project root (defaults to the current working directory).
        #[arg(long)]
        project: Option<PathBuf>,
        /// Alternative directory name when the project already uses this name.
        #[arg(long)]
        alias: Option<String>,
    },
    /// Remove a managed project install, local package, or published cloud skill.
    Remove {
        /// Project directory alias; or a skill identity for local/cloud scope.
        skill: String,
        /// Storage layer to remove; cloud removal requires account sign-in.
        #[arg(long, value_enum, default_value_t = Scope::Project)]
        scope: Scope,
        /// Project root for project scope (defaults to the current directory).
        #[arg(long)]
        project: Option<PathBuf>,
    },
}

/// The three independently managed skill storage layers.
#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum Scope {
    Project,
    Local,
    Cloud,
}

/// Parse help/version before initializing async networking or the local library.
fn main() -> ExitCode {
    let mut cli = Cli::parse();
    cli.trace_id = uuid::Uuid::new_v4().simple().to_string();
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("initialize runtime")
        .and_then(|runtime| runtime.block_on(run(&cli)));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error)
            if error.chain().any(|cause| {
                cause
                    .downcast_ref::<io::Error>()
                    .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe)
            }) =>
        {
            ExitCode::SUCCESS
        }
        Err(error) => {
            if cli.json {
                anstream::eprintln!("{}", json!({"error": format!("{error:#}")}));
            } else {
                anstream::eprintln!("error: {error:#}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Route one command; local operations do not construct an HTTP client or log in.
async fn run(cli: &Cli) -> Result<()> {
    let home = cli
        .home
        .clone()
        .or_else(|| dirs::home_dir().map(|p| p.join(".mskill")))
        .context("cannot locate home directory; use --home")?;
    let output = Output { json: cli.json };
    match &cli.command {
        Command::Login { no_browser } => {
            let session = auth(cli, &home)?.login_with_options(!no_browser).await?;
            output.success(&session, "Signed in.")
        }
        Command::Logout => {
            auth(cli, &home)?.logout().await?;
            output.success(&json!({"signed_out": true}), "Signed out.")
        }
        Command::Whoami => {
            let token = auth(cli, &home)?
                .access_token()
                .await
                .context("sign in with `mskill login` to view your publisher identity")?;
            let profile = Registry::new(&cli.registry, cli.verbose, &cli.trace_id)?
                .me(&token)
                .await?;
            mskill_protocol::validate_owner_id(&profile.owner_id)?;
            ensure!(
                profile.owner_id != LOCAL_OWNER,
                "registry returned reserved local owner ID"
            );
            output.success(&profile, &format!("Publisher: {}", profile.owner_id))
        }
        Command::Add { path } => {
            let info = LocalStore::open(&home)?.add(path)?;
            output.success(&info, &format!("Added {}.", info.id))
        }
        Command::Pack {
            path,
            output: destination,
        } => {
            let bytes = mskill_core::pack_or_read(path)?;
            write_export(destination, &bytes)?;
            output.success(&json!({"path": destination, "sha256": sha256_hex(&bytes), "size_bytes": bytes.len()}), &format!("Packed {}.", destination.display()))
        }
        Command::Export {
            skill,
            output: destination,
        } => {
            let id = local_id(skill)?;
            let bytes = LocalStore::open(&home)?.read_archive(&id)?;
            write_export(destination, &bytes)?;
            output.success(&json!({"id": id, "path": destination, "sha256": sha256_hex(&bytes), "size_bytes": bytes.len()}), &format!("Exported {} to {}.", id, destination.display()))
        }
        Command::List { cloud, owner } => {
            if *cloud {
                let list = Registry::new(&cli.registry, cli.verbose, &cli.trace_id)?
                    .list(owner.as_deref())
                    .await?;
                for skill in &list.skills {
                    skill.validate()?;
                }
                if output.json {
                    return output.value(&list.skills);
                }
                return output.table(
                    &list
                        .skills
                        .iter()
                        .map(|s| {
                            (
                                format!("{}/{}", s.owner_id, s.name),
                                s.sha256.clone(),
                                s.description.clone(),
                            )
                        })
                        .collect::<Vec<_>>(),
                );
            }
            let mut list = LocalStore::open(&home)?.list()?;
            if let Some(owner) = owner {
                list.retain(|s| s.id.owner_id == *owner);
            }
            if output.json {
                return output.value(&list);
            }
            output.table(
                &list
                    .iter()
                    .map(|s| (s.id.to_string(), s.sha256.clone(), s.description.clone()))
                    .collect::<Vec<_>>(),
            )
        }
        Command::Pull { skill } => {
            let id = cloud_id(skill)?;
            let store = LocalStore::open(&home)?;
            let registry = Registry::new(&cli.registry, cli.verbose, &cli.trace_id)?;
            let (info, changed) = pull(&store, &registry, &id).await?;
            output.success(
                &info,
                &format!(
                    "{} {}.",
                    if changed {
                        "Pulled"
                    } else {
                        "Already current:"
                    },
                    id
                ),
            )
        }
        Command::Publish { skill } => {
            let id = local_id(skill)?;
            let store = LocalStore::open(&home)?;
            let bytes = store.read_archive(&id)?;
            let token = auth(cli, &home)?
                .access_token()
                .await
                .context("sign in with `mskill login` before publishing")?;
            let registry = Registry::new(&cli.registry, cli.verbose, &cli.trace_id)?;
            let profile = registry.me(&token).await?;
            let published_id = SkillId::new(profile.owner_id, &id.name)?;
            ensure!(
                published_id.owner_id != LOCAL_OWNER,
                "registry returned reserved local owner ID"
            );
            let uploaded_hash = sha256_hex(&bytes);
            let metadata = registry.publish(&published_id, &token, bytes).await?;
            metadata.validate()?;
            ensure!(
                metadata.id()? == published_id && metadata.sha256 == uploaded_hash,
                "registry publish response does not match uploaded skill"
            );
            output.success(&metadata, &format!("Published {}.", published_id))
        }
        Command::Update { skill, from } => {
            update(cli, &home, &output, skill.as_deref(), from.as_deref()).await
        }
        Command::Clone {
            skill,
            project,
            alias,
        }
        | Command::Link {
            skill,
            project,
            alias,
        } => {
            let store = LocalStore::open(&home)?;
            let id = local_id(skill)?;
            let project = project_path(project.as_deref())?;
            let linked = matches!(&cli.command, Command::Link { .. });
            let path = if linked {
                store.link_to_project(&project, &id, alias.as_deref())?
            } else {
                store.clone_to_project(&project, &id, alias.as_deref())?
            };
            output.success(
                &json!({"id": id, "path": path, "linked": linked}),
                &format!(
                    "{} {} to {}.",
                    if linked { "Linked" } else { "Copied" },
                    id,
                    path.display()
                ),
            )
        }
        Command::Remove {
            skill,
            scope,
            project,
        } => {
            if *scope != Scope::Project && project.is_some() {
                bail!("--project is only valid with --scope project");
            }
            match scope {
                Scope::Project => LocalStore::open(&home)?
                    .remove_project(&project_path(project.as_deref())?, skill)?,
                Scope::Local => LocalStore::open(&home)?.remove_local(&local_id(skill)?)?,
                Scope::Cloud => {
                    let id = cloud_id(skill)?;
                    let token = auth(cli, &home)?
                        .access_token()
                        .await
                        .context("sign in with `mskill login` before removing cloud skills")?;
                    Registry::new(&cli.registry, cli.verbose, &cli.trace_id)?
                        .remove(&id, &token)
                        .await?;
                }
            }
            let warning = if *scope == Scope::Local {
                " Existing project links may now be broken."
            } else {
                ""
            };
            output.success(
                &json!({"removed": skill, "scope": format!("{scope:?}").to_lowercase()}),
                &format!(
                    "Removed {} from {}.{}",
                    skill,
                    format!("{scope:?}").to_lowercase(),
                    warning
                ),
            )
        }
    }
}

/// Refresh only the selected local identity; frontmatter rename is never implicit.
async fn update(
    cli: &Cli,
    home: &Path,
    output: &Output,
    skill: Option<&str>,
    source: Option<&Path>,
) -> Result<()> {
    let store = LocalStore::open(home)?;
    if let Some(source) = source {
        let id = local_id(skill.context("--from requires a skill name")?)?;
        ensure!(
            id.owner_id == LOCAL_OWNER,
            "--from updates local skills only; use `pull owner/name` for cloud skills"
        );
        let old = store.get(&id)?;
        // Validate against the requested identity before modifying library state.
        let bytes = mskill_core::pack_or_read(source)?;
        let info = store.install_bytes(&id, &bytes)?;
        return output.success(
            &info,
            &format!(
                "{} {}.",
                if old.sha256 == info.sha256 {
                    "Already current:"
                } else {
                    "Updated"
                },
                id
            ),
        );
    }
    let ids = match skill {
        Some(skill) => vec![cloud_id(skill)?],
        None => store
            .list()?
            .into_iter()
            .filter(|s| s.id.owner_id != LOCAL_OWNER)
            .map(|s| s.id)
            .collect(),
    };
    if ids.is_empty() {
        return output.success(&Vec::<SkillInfo>::new(), "No cloud skills to update.");
    }
    let registry = Registry::new(&cli.registry, cli.verbose, &cli.trace_id)?;
    let mut updated = Vec::with_capacity(ids.len());
    let mut changed_count = 0;
    for id in ids {
        let (info, changed) = pull(&store, &registry, &id).await?;
        changed_count += usize::from(changed);
        updated.push(info);
    }
    output.success(
        &updated,
        &format!(
            "Updated {} skill(s); {} already current.",
            changed_count,
            updated.len() - changed_count
        ),
    )
}

/// Compare archive hashes and tolerate one concurrent publish between metadata/download.
async fn pull(store: &LocalStore, registry: &Registry, id: &SkillId) -> Result<(SkillInfo, bool)> {
    for _ in 0..2 {
        let metadata: SkillMetadata = registry.metadata(id).await?;
        metadata.validate()?;
        ensure!(
            metadata.id()? == *id,
            "registry metadata returned another skill identity"
        );
        if let Ok(existing) = store.get(id) {
            if existing.sha256 == metadata.sha256
                && existing.archive.is_file()
                && existing.directory.is_dir()
            {
                return Ok((existing, false));
            }
        }
        let bytes = match registry.archive(id, &metadata.sha256).await {
            Ok(bytes) => bytes,
            Err(error) if http::archive_changed(&error) => continue,
            Err(error) => return Err(error),
        };
        if sha256_hex(&bytes) != metadata.sha256 {
            continue;
        }
        ensure!(
            bytes.len() as u64 == metadata.size_bytes,
            "download size disagrees with registry metadata"
        );
        return Ok((store.install_bytes(id, &bytes)?, true));
    }
    bail!("registry archive changed during download or failed SHA-256 verification; retry the command")
}

/// Resolve a local abbreviation without guessing another user's namespace.
fn local_id(reference: &str) -> Result<SkillId> {
    if reference.contains('/') {
        return reference.parse().map_err(Into::into);
    }
    SkillId::new(LOCAL_OWNER, reference).map_err(Into::into)
}

/// Cloud operations require an explicit publisher, avoiding account ambiguity.
fn cloud_id(reference: &str) -> Result<SkillId> {
    let id: SkillId = reference
        .parse()
        .context("cloud skills use owner/name; local skills use `update NAME --from PATH`")?;
    ensure!(
        id.owner_id != LOCAL_OWNER,
        "local/name is not a cloud skill"
    );
    Ok(id)
}

/// Authentication is constructed only for commands requiring account access.
fn auth(cli: &Cli, home: &Path) -> Result<AuthClient> {
    let client_id = cli.client_id.clone().filter(|s| !s.is_empty()).context(
        "account client ID is not configured; set MSKILL_OIDC_CLIENT_ID or use --client-id",
    )?;
    AuthClient::new(
        AuthConfig {
            issuer: cli.issuer.clone(),
            client_id,
            redirect_uri: cli.redirect_uri.clone(),
        },
        home,
    )?
    .with_trace_context(&cli.trace_id, cli.verbose)
}

/// Preserve explicit roots and obtain the current directory only when needed.
fn project_path(path: Option<&Path>) -> Result<PathBuf> {
    match path {
        Some(path) => Ok(path.to_path_buf()),
        None => std::env::current_dir().context("locate current project directory"),
    }
}

/// Publish a user-selected portable file without clobbering an existing output.
fn write_export(destination: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    ensure!(
        destination
            .extension()
            .is_some_and(|extension| extension == "skill"),
        "output must have the .skill extension"
    );
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).context("create archive output temporary file")?;
    temporary.write_all(bytes).context("write archive output")?;
    temporary
        .as_file()
        .sync_all()
        .context("sync archive output")?;
    temporary
        .persist_noclobber(destination)
        .map_err(|error| error.error)
        .context("output already exists or cannot be created")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Abbreviations always mean local; cloud operations never guess an owner.
    #[test]
    fn identities_are_unambiguous() {
        assert_eq!(
            local_id("rust-review").unwrap().to_string(),
            "local/rust-review"
        );
        assert_eq!(
            cloud_id("publisher/rust-review").unwrap().owner_id,
            "publisher"
        );
        assert!(cloud_id("rust-review").is_err());
        assert!(cloud_id("local/rust-review").is_err());
        assert!(local_id("../rust-review").is_err());
    }

    /// Required source/name and output contracts are checked before filesystem access.
    #[test]
    fn command_contracts_parse() {
        assert!(Cli::try_parse_from(["mskill", "update", "--from", "skill-dir"]).is_err());
        assert!(Cli::try_parse_from(["mskill", "pack", "skill-dir"]).is_err());
        assert!(Cli::try_parse_from(["mskill", "pack", "skill-dir", "-o", "x.skill"]).is_ok());
        assert!(
            Cli::try_parse_from(["mskill", "clone", "owner/rust-review", "--alias", "review"])
                .is_ok()
        );
    }
}
