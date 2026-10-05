---
name: mskill-publish
description: Sign in to the mskill registry with moeSegFault Identity and publish, replace, or remove skills using the mskill CLI. Use for registry account and publishing workflows, not unrelated authentication or skill authoring.
---

# Publish with mskill

Use the installed `mskill` CLI. Read [references/publishing.md](references/publishing.md) for the account, publication, and deletion workflow; consult `mskill --help` when the installed command surface differs.

Publish only when the user requested publication. Confirm which local skill and account namespace the operation affects. Registry names are scoped to an account, so different accounts may publish the same skill name.

The registry stores only the latest package. Publishing replaces that account's current package; it does not create a version history or rollback point. Keep a separate backup when the user needs one.

Before publishing, check the package contents for credentials and private material, and preserve its license and attribution files. The uploader must have permission to distribute all included material. Removing a registry package does not remove copies already downloaded by others.

Do not put access tokens in chat, logs, command examples, or project files. Login authorizes the requested registry workflow, not unrelated account changes.
