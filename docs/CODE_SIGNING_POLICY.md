# Code Signing Policy

This document describes who can produce signed Ortu binaries, how they are
built, and how you can verify that a copy of Ortu came from this project.

It is published to satisfy the requirements of the
[SignPath Foundation](https://signpath.org/) free code signing program for open
source projects, and is mirrored at
<https://ortu.abhijithpsubash.com/code-signing-policy>.

**Last updated:** 2026-07-24

## Project

| | |
|---|---|
| Project | Ortu — local-first clipboard manager |
| Source | <https://github.com/abhijith-p-subash/ortu> |
| License | [MIT](../LICENSE) (OSI-approved, no dual licensing) |
| Homepage | <https://ortu.abhijithpsubash.com/> |
| Privacy policy | [Privacy & Security](PRIVACY_AND_SECURITY.md) · <https://ortu.abhijithpsubash.com/privacy-policy> |

Ortu contains no proprietary or closed-source components beyond the operating
system libraries and the platform WebView runtime that every Tauri application
links against.

## Team and roles

Ortu is currently maintained by a single person. Roles are listed separately
because they carry different responsibilities, even where one person holds more
than one.

| Name | GitHub | Roles |
|------|--------|-------|
| Abhijith P Subash | [@abhijith-p-subash](https://github.com/abhijith-p-subash) | Author, Reviewer, Approver, Release manager |

- **Authors** write code and open pull requests.
- **Reviewers** review every change before it reaches `main`.
- **Approvers** are the only people who may trigger a signed release build.
- Contributors from outside the team are **authors only**. They cannot merge
  their own changes and cannot trigger a signed build.

If additional maintainers join, this table is updated in the same commit that
grants them access.

### Account security

Every person listed above has **two-factor authentication enabled** on their
GitHub account. The repository belongs to the maintainer's personal GitHub
account, and organization-level 2FA enforcement applies to all members with
write access.

## Source and contribution review

- All work lands on `main` through pull requests. Branch protection on `main`
  requires a pull request and passing CI, and **nobody outside the maintainer
  team can push to it under any circumstances**. Repository administrators retain
  a bypass for emergency fixes; every use of it is visible in the public commit
  history.
- **Every pull request from outside the team requires review and approval by a
  maintainer before merge.** No external contribution reaches a release without
  a maintainer having read it, including any dependency it introduces.
- Maintainer pull requests must pass CI and are reviewed against the same
  checklist. While the project has a single maintainer, GitHub cannot enforce a
  second approver on those; once a second maintainer joins, cross-review becomes
  mandatory and this section is updated.
- CI (`.github/workflows/ci.yml`) must pass before a pull request can merge.
- The repository is owned and controlled by the team listed above. We only sign
  binaries built from this repository's source.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the contributor-facing version of
this process.

## Build and release process

Signed binaries are **only ever produced by CI** — never on a developer machine.

1. A maintainer bumps the version and merges to `main`.
2. `.github/workflows/release.yml` runs on GitHub-hosted runners
   (`windows-latest`, `macos-latest`, `ubuntu-22.04`), checks out the tagged
   source, and builds the installers with
   [`tauri-apps/tauri-action`](https://github.com/tauri-apps/tauri-action).
3. Windows installers are submitted to SignPath from that same workflow run.
   SignPath verifies the build originated from this repository and workflow
   before signing — see [Signing](#signing).
4. The signed artifacts are attached to a **draft** GitHub Release.
5. A maintainer verifies the signatures and the auto-update path, then
   publishes the release.

Because the build inputs are the public source tree and the build runs on
ephemeral GitHub runners, anyone can inspect exactly what went into a release.

## Signing

### Windows (Authenticode)

Windows installers are signed with a certificate provided free of charge by the
**[SignPath Foundation](https://signpath.org/)**, using the code signing service
of **[SignPath.io](https://signpath.io/)**.

- The signing certificate's private key is held in SignPath's HSM. **No member
  of this project has access to it**, and it cannot be extracted or used
  outside SignPath.
- Signing requests can only be submitted by the release workflow in this
  repository, under a signing policy that restricts which artifacts may be
  signed.
- Signed artifacts: `Ortu_<version>_x64-setup.exe` (NSIS) and
  `Ortu_<version>_x64_en-US.msi`.

### macOS and Linux

macOS builds are **not currently notarized** with an Apple Developer account,
and Linux packages are not signed. Installation notes for the resulting
Gatekeeper warning are in the [README](../README.md#macos-gatekeeper-note-unsigned-builds).

### Update signatures

Separately from Authenticode, every release artifact carries a `.sig` file — a
[minisign](https://jedisct1.github.io/minisign/) signature checked by Ortu's
built-in updater against the public key embedded in
[`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json). This prevents an
update from being swapped out in transit, on every platform.

The corresponding private key is stored as a GitHub Actions secret, is only
accessible to the release workflow, and is never present on a developer
machine.

## Verifying a download

On Windows, check the signature before installing:

```powershell
signtool verify /pa /v Ortu_2.0.2_x64-setup.exe
```

Or right-click the installer → **Properties** → **Digital Signatures**.

You can also compare the file against the checksums published with each
[GitHub Release](https://github.com/abhijith-p-subash/ortu/releases).

If you find a binary claiming to be Ortu that is signed by anyone else, or an
Ortu installer distributed outside GitHub Releases, please
[report it](../SECURITY.md) — do not run it.

## User protections

- **Privacy** — Ortu is local-first. Clipboard history stays on your device;
  there is no account, no cloud sync, and no telemetry. The only outbound
  network requests are update checks against GitHub. Full detail in
  [Privacy & Security](PRIVACY_AND_SECURITY.md).
- **System configuration** — Ortu registers global keyboard shortcuts and, if
  you enable it, a launch-at-login entry. Both are opt-in from Settings and
  are listed in [Configuration](CONFIGURATION.md).
- **Uninstallation** — Ortu can be removed through the standard uninstaller for
  your platform (Windows: Apps & features; macOS: delete `Ortu.app`; Linux:
  your package manager). Removing the app-data directory documented in
  [Configuration](CONFIGURATION.md#storage-locations) deletes all stored
  history.
- **No hacking or scanning tools** — Ortu is a productivity utility. It does
  not exploit vulnerabilities and is not a security diagnostic tool.

## Credits

Free code signing for Ortu is provided by the
**[SignPath Foundation](https://signpath.org/)**, with a certificate and signing
infrastructure from **[SignPath.io](https://signpath.io/)**. Thank you.

## Reporting

Security issues: see [SECURITY.md](../SECURITY.md). Please report privately —
do not open a public issue.
