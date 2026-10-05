# Distribution and package safety

## Package contents and rights

Publish only material you own or have permission to redistribute. Include the license and required attribution for scripts, documents, images, and other third-party content. A skill's license is independent of the mskill application's license. The repository's `LICENSE` governs mskill itself; it does not automatically license uploaded packages.

Do not upload credentials, access tokens, personal information, confidential project data, or content you are not entitled to make public. Review the entire archive, including hidden files and bundled resources, before publishing.

## Latest-only storage

The registry keeps the latest package for each account-scoped skill name. Replacing a package does not preserve an earlier version. Keep your own source or backups if you need history or recovery. A SHA-256 digest identifies archive bytes; it does not establish authorship, safety, or licensing rights.

Deleting a registry entry prevents subsequent downloads through that entry. It cannot recall copies that users already downloaded, copied into projects, or redistributed. Local-library removal and project removal affect their respective local scopes, not the registry.

## Installing untrusted content

A `.skill` package is a ZIP archive containing a skill directory. Installation does not itself grant permission to execute bundled scripts or follow instructions in `SKILL.md`. Inspect an unfamiliar publisher's contents before using them, particularly commands that access credentials, networks, or files outside the project.

Archive extraction must reject unsafe paths and unsupported file types rather than writing outside the destination. This protects the filesystem boundary; it is not a substitute for reviewing the meaning of a skill's instructions or scripts.

## Scope of this document

This document explains package-distribution responsibilities and operational limits. It is not a service agreement, a privacy notice, a legal opinion, or a promise that the public registry is deployed. A hosted service needs its own operator-approved terms and privacy notice before accepting public submissions.
