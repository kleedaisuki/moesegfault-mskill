# Registry downloads

The default registry is `https://skills.moesegfault.dev`. Select another registry with global `--registry URL` or `MSKILL_REGISTRY`; use the same registry consistently for a workflow.

```sh
mskill list --cloud
mskill list --cloud --owner publisher-id
mskill pull publisher-id/my-skill
mskill clone publisher-id/my-skill --project .
mskill update publisher-id/my-skill
```

Replace `publisher-id` in these examples with a namespace from the registry listing. Always use `owner/name` for a cloud package. Two publishers may use the same skill name without sharing ownership. `pull` places the package in the local library; `clone` then copies its extracted directory into the project. Fetching a package is not authorization to execute scripts it contains.

Updates compare archive SHA-256 values and replace changed packages, without keeping historical versions. If the user needs a previous package, preserve an independent backup before replacement; do not promise registry rollback.

For an isolated local-registry exercise, set both `--home` and `--registry` explicitly so it does not mix test state with the user's normal library. Keep test files within the project's `.temp` or `.cache` directory.

