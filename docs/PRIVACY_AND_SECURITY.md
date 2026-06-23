# Privacy & Security

Ortu is **local-first**. Your clipboard history never leaves your device unless
you explicitly export it.

## Data handling

- **Storage:** everything is kept in a local SQLite database in the OS app-data
  directory (see [CONFIGURATION.md](CONFIGURATION.md#storage-locations)).
- **No cloud, no account, no telemetry.** Ortu does not phone home. The only
  network calls are checking GitHub Releases for updates.
- **Network allow-list:** the app's Content-Security-Policy restricts outbound
  connections to what's needed (local IPC and the GitHub update endpoints).

## Sensitive items & encryption

- The auto-grouping classifier flags likely **secrets** (API keys, tokens, SSH
  keys, JWTs, environment variables, etc.).
- With **Auto-mask detected secrets** enabled (Settings → Privacy), such items
  are:
  - **masked** in the UI (revealable on demand), and
  - **encrypted at rest** using **AES‑256‑GCM**.
- Encryption is **field-level**: only sensitive values are encrypted, not the
  whole database. Encrypted values carry an `enc:v1:` prefix so they're
  self-describing.
- The 256-bit key is generated on first use and stored in `.sensitive_key`
  (file permissions `0600`) in the app-data directory.

> If `.sensitive_key` is lost or deleted, only the encrypted (masked) items
> become unreadable — all other history is unaffected.

### Threat model (what this does and doesn't protect)

- ✅ Protects masked secrets from casual at-rest inspection of `ortu.db`
  (the ciphertext isn't readable without the key).
- ⚠️ The key lives on the same machine as the data. A process running as your
  user can read both. This is not protection against a compromised account or
  malware running as you.
- ⚠️ Non-sensitive history is stored in plaintext in SQLite by design (for
  search/preview). Use retention settings if you don't want it kept.
- ⚠️ **Backups and text exports are not encrypted.** Treat exported files as
  sensitive.

## Reducing what's stored

- **History retention** (Settings → History) — the default *On reboot* mode
  clears ungrouped & unpinned items on every reboot; or pick an age/count limit.
  See [CONFIGURATION.md](CONFIGURATION.md#history-retention).
- **Pin/group intentionally** — only pinned and grouped items are kept long-term.
- **Pause capture** — click the status pill in the header to stop recording
  entirely (e.g. while handling sensitive data). Nothing is captured until you
  resume, and the paused state survives restarts.
- **Delete** individual items at any time.

## Permissions

- **macOS Accessibility** — required for Ortu to *paste into* other apps
  (it synthesizes the paste keystroke). Grant it under
  System Settings → Privacy & Security → Accessibility. Ortu prompts and links
  you there when needed.

## Distribution & signing

- Release artifacts are built in CI and the **updater verifies downloads against
  a public key** embedded in `tauri.conf.json`, so updates can't be silently
  swapped.
- macOS builds without Apple notarization will trigger Gatekeeper warnings; see
  the README for the quarantine workaround and
  [BUILD_AND_RELEASE.md](BUILD_AND_RELEASE.md#macos-signing--notarization-optional-but-recommended).

## Reporting a vulnerability

Please report security issues privately — see [../SECURITY.md](../SECURITY.md).
Do **not** open a public issue for vulnerabilities.
