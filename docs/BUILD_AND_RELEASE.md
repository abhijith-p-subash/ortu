# Build & Release

## Local production build

```bash
npm run tauri build
```

This runs the SvelteKit build, compiles the Rust core with the optimized release
profile, and bundles platform installers.

### Output artifacts

Bundles are written under `src-tauri/target/release/bundle/`:

| Platform | Artifacts |
|----------|-----------|
| macOS | `.app`, `.dmg` |
| Windows | `.msi`, `.exe` (NSIS) |
| Linux | `.AppImage`, `.deb` |

> Cross-compiling between OSes is not supported by Tauri — build each platform on
> its own OS (locally or in CI).

## Release profile

The Rust release profile (in `src-tauri/Cargo.toml`) is tuned for a small, fast
binary: `lto = true`, `codegen-units = 1`, `opt-level = 3`, `panic = "abort"`,
`strip = true`. These only affect `--release` / `tauri build`, not `tauri dev`.

## Versioning a release

Bump the version everywhere it's referenced, then commit and tag:

```bash
npm run version-up 2.0.0          # updates package.json, tauri.conf.json, Cargo.toml
git commit -am "chore: release v2.0.0"
git tag v2.0.0
git push --follow-tags
```

Update [../CHANGELOG.md](../CHANGELOG.md) in the same commit.

## Automated releases (CI)

`.github/workflows/release.yml` builds and publishes installers via
[`tauri-action`](https://github.com/tauri-apps/tauri-action) on macOS, Ubuntu,
and Windows runners. It creates a **draft** GitHub release with the artifacts
attached.

### Required repository secrets

For the updater to ship verifiable updates, set these under
**Settings → Secrets and variables → Actions**:

| Secret | Purpose |
|--------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Private key used to sign update artifacts |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Password for the signing key |

`GITHUB_TOKEN` is provided automatically by Actions.

### Generating updater signing keys

```bash
npm run tauri signer generate -- -w ~/.tauri/ortu.key
```

- Keep the **private** key secret (store as `TAURI_SIGNING_PRIVATE_KEY`).
- Put the **public** key in `tauri.conf.json` under `plugins.updater.pubkey`.

## Auto-update flow

1. CI publishes a release with `latest.json` + signed artifacts.
2. The app polls `plugins.updater.endpoints` (GitHub Releases `latest.json`).
3. If a newer signed version exists, it's downloaded, verified against the
   public key, and installed.

## macOS signing & notarization (optional but recommended)

Without an Apple Developer account, distributed macOS builds are unsigned and
trigger Gatekeeper warnings (see the README workaround). For a smooth install
experience, sign and notarize with a Developer ID and configure the matching
environment variables for `tauri build`. See the
[Tauri macOS code-signing guide](https://tauri.app/distribute/sign/macos/).

## Pre-release checklist

- [ ] `npm run check` passes
- [ ] `cargo check` passes (in `src-tauri/`)
- [ ] Version bumped via `npm run version-up`
- [ ] `CHANGELOG.md` updated
- [ ] Smoke-tested a local `tauri build` on at least one OS
- [ ] Tag pushed; CI draft release reviewed before publishing
