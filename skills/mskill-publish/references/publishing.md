# Account and publication workflow

Use the same registry for login and subsequent operations. The default is `https://skills.moesegfault.dev`; global `--registry URL` or `MSKILL_REGISTRY` selects another endpoint.

## Sign in

```sh
mskill login
mskill whoami
```

Login uses moeSegFault Identity and maps the signed-in account to its registry namespace. The official service already has its client configured: `mskill login` needs no client-ID setup. For a staging or custom registry, explicitly set the registry, its issuer and registered client ID; the official default does not follow a registry or issuer override. Global `--client-id` / `MSKILL_OIDC_CLIENT_ID`, `--issuer` / `MSKILL_OIDC_ISSUER`, and `--redirect-uri` / `MSKILL_OIDC_REDIRECT_URI` remain available.

```sh
mskill --registry https://skills-staging.moesegfault.dev \
  --issuer https://identity-staging.moesegfault.dev \
  --client-id mskill-cli-staging login
```

Keep these options or environment values for later commands against that service. For other registries, use their registered configuration rather than copying the staging values.

Complete browser sign-in as the intended account. `whoami` returns the registry publisher ID for that account; use it with `mskill list --cloud --owner OWNER_ID` to list its publications. Never paste access tokens into shared files, chat, or logs. `mskill logout` clears the CLI's local login; it is not deletion of the moeSegFault account.

## Publish or replace

```sh
mskill add ./skills/my-skill
mskill publish local/my-skill
mskill list --cloud
```

`publish` accepts a library name or full identity and uploads it under the mapped account owner using the skill's name. It does not publish under another account just because its local identity has that owner's prefix. Publishing the same name again replaces the current package; there is no registry version history.

Before upload, inspect the complete package, including resources and hidden files. Remove secrets and private data. Keep license and attribution files. Do not change package content or licensing beyond the user's requested publication work. A hash match means identical bytes, not trusted or legally cleared content.

## Delete a cloud publication

```sh
mskill remove publisher-id/my-skill --scope cloud
```

Replace `publisher-id` with the publisher namespace shown by `whoami`, `publish`, or in the registry listing. Use the actual account-scoped identity, and delete only the publication requested by the user. Cloud deletion does not remove local library packages, project copies, or others' downloaded copies. Project removal uses the canonical skill name; library removal uses `--scope local`.

For machine output use global `--json`. For a local test service, use an isolated `--home` and explicitly select its registry, issuer, and registered client ID.
