# Security Policy

## Supported versions

Ortu is actively developed; security fixes target the **latest release** and the
`main` branch.

| Version | Supported |
|---------|-----------|
| 2.x (latest) | ✅ |
| < 2.0 | ❌ |

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, report privately via one of:

- GitHub's [private vulnerability reporting](https://github.com/abhijith-p-subash/ortu/security/advisories/new)
  (Security → Report a vulnerability), or
- Email: **abhijith.p.subash@gmail.com**

Please include:

- A description of the issue and its impact
- Steps to reproduce (proof-of-concept if possible)
- Affected version(s) and OS
- Any suggested remediation

### What to expect

- **Acknowledgement** within a few days.
- An initial assessment and, where applicable, a coordinated fix and release.
- Credit in the release notes if you'd like (let us know).

Please act in good faith: avoid privacy violations, data destruction, and
service disruption while researching, and give us reasonable time to address the
issue before any public disclosure.

## Security model (summary)

- Ortu is **local-first**: clipboard data is stored only on the device.
- Sensitive (masked) items are encrypted at rest with AES‑256‑GCM; the key lives
  in a `0600` file in the app-data directory.
- Updates are verified against a public signing key embedded in the app.

For the full model and its limits, see
[docs/PRIVACY_AND_SECURITY.md](docs/PRIVACY_AND_SECURITY.md).
