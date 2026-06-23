# Contributing to Ortu

Thanks for your interest in improving Ortu! 🎉 This guide covers how to propose
changes, set up your environment, and get a PR merged.

By participating, you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute

- 🐛 **Report bugs** — open an issue with clear repro steps.
- 💡 **Suggest features** — open a feature-request issue or discussion.
- 📝 **Improve docs** — fixes and clarifications are very welcome.
- 🔧 **Send code** — pick up an issue (ideally one labeled `good first issue`).

For anything large, please open an issue to discuss the approach **before**
investing significant time, so we can align on direction.

## Development setup

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for prerequisites and the full
project layout. The short version:

```bash
git clone https://github.com/abhijith-p-subash/ortu.git
cd ortu
npm install
npm run tauri dev
```

## Branch & PR workflow

1. **Fork** the repo and create a branch off `main`:
   ```bash
   git checkout -b feature/short-description
   ```
   Use a descriptive prefix: `feature/`, `fix/`, `docs/`, `chore/`, `refactor/`.
2. Make focused commits.
3. Ensure checks pass (see below).
4. Push and open a **Pull Request** against `main`. Fill out the PR template,
   link related issues (`Fixes #123`), and add screenshots/GIFs for UI changes.
5. A maintainer will review; please be responsive to feedback.

Keep PRs scoped — several small PRs are easier to review than one large one.

## Required checks

Before pushing, make sure both pass cleanly:

```bash
npm run check                 # frontend type-check (0 errors/0 warnings)
cd src-tauri && cargo check   # backend compiles
```

If you touched Rust, also run:

```bash
cd src-tauri && cargo fmt && cargo clippy
```

## Coding guidelines

- **Match existing style** — comment density, naming, and idioms should look like
  the surrounding code.
- **Frontend:** Svelte 5 runes; Tailwind utilities using theme tokens
  (`bg-surface`, `text-fg`, `border-overlay/…`) so light & dark both work. Don't
  hard-code colors.
- **Shortcuts:** change them via `src/lib/shortcuts.ts` (single source of truth).
- **Notifications:** use the shared `showToast()` / `<Toaster />`.
- **Backend:** keep DB migrations idempotent; new settings go through `app_meta`.
- **Cross-platform:** changes must build and behave on macOS, Windows, and Linux.
  Guard platform-specific code with `#[cfg(...)]` and note any path you couldn't
  test on other OSes.
- **Keep it lightweight:** avoid heavy dependencies; prefer the existing stack.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/) are encouraged:

```
feat(stack): add copy-to-stack global shortcut
fix(db): keep grouped items on reboot cleanup
docs: clarify retention modes
```

## Versioning & changelog

- Ortu follows [Semantic Versioning](https://semver.org).
- Maintainers bump versions with `npm run version-up <x.y.z>`.
- Note user-facing changes in [CHANGELOG.md](CHANGELOG.md) (Unreleased section).

## Reporting security issues

Please do **not** file public issues for vulnerabilities. Follow
[SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contributions are licensed under the
project's [MIT License](LICENSE).
