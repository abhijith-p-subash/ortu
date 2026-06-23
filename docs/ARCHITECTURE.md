# Architecture

Ortu is a [Tauri v2](https://tauri.app) desktop app: a Rust backend (the
"core") paired with a SvelteKit web frontend rendered in the OS WebView. They
communicate over Tauri's IPC bridge.

```
┌──────────────────────────────────────────────────────────┐
│                        OS WebView                          │
│   SvelteKit SPA (Tailwind)                                 │
│   ┌───────────────┐  ┌──────────────┐  ┌───────────────┐  │
│   │  Main window  │  │ Quick popup  │  │ Settings page │  │
│   └───────┬───────┘  └──────┬───────┘  └───────┬───────┘  │
│           │   invoke()/events (IPC)             │          │
└───────────┼─────────────────┼───────────────────┼─────────┘
            │                 │                    │
┌───────────▼─────────────────▼────────────────────▼─────────┐
│                    Rust core (Tauri)                        │
│  commands.rs   clipboard.rs   db.rs   crypto.rs   lib.rs    │
│   (IPC API)   (capture+class)  (data)  (encrypt)  (setup)   │
└───────────────────────────────┬────────────────────────────┘
                                 │
                     ┌───────────▼───────────┐
                     │  SQLite (WAL)  +  blob │
                     │     store (images)     │
                     └────────────────────────┘
```

## Frontend (`src/`)

SvelteKit configured as a static SPA (`adapter-static`, `ssr = false`) so it can
run inside the WebView without a server.

| Path | Role |
|------|------|
| `src/routes/+page.svelte` | Main window — history, groups, search, snippets, backup |
| `src/routes/popup/+page.svelte` | Quick-access popup window |
| `src/routes/settings/+page.svelte` | Settings page (appearance, privacy, retention, shortcuts) |
| `src/routes/+layout.svelte` / `+layout.ts` | Root layout; theme init; SPA config |
| `src/lib/types.ts` | Shared TypeScript types (`ClipboardItem`, `Snippet`) |
| `src/lib/filters.ts` | Search-query building & preview helpers |
| `src/lib/shortcuts.ts` | Single source of truth for shortcut labels/accelerators |
| `src/lib/theme.ts` | Theme resolve/apply/persist |
| `src/lib/toast.ts` + `Toaster.svelte` | Unified toast/notification system |
| `src/lib/updater.ts` | Update-check helpers |

Multiple windows each load a route (`/`, `/popup`) and share the same backend
state through IPC commands and events.

## Backend (`src-tauri/src/`)

| File | Responsibility |
|------|----------------|
| `lib.rs` | App setup: windows, tray, global shortcuts, autostart, updater, panic/startup tracing, boot-change cleanup |
| `commands.rs` | All `#[tauri::command]` handlers exposed to the frontend (the IPC API) |
| `clipboard.rs` | Background capture listener + the rule-based auto-grouping classifier |
| `db.rs` | SQLite schema, migrations, queries, FTS5 setup, retention, blob store |
| `crypto.rs` | Field-level AES‑256‑GCM encryption for sensitive items |
| `main.rs` | Thin binary entry that calls `ortu_lib::run()` |

### Data flow: capturing a clip

1. `clipboard.rs` detects a change (text/image/files).
2. The classifier scores it against ~30 detectors and produces candidate groups.
3. `db.rs` inserts/updates the row (de-duplicated), records group confidences,
   and stores image bytes in the content-addressed blob store.
4. The core emits a `clipboard-updated` event.
5. The frontend reloads the (debounced) history list.

### Data flow: searching

1. The frontend calls `get_history(search)`.
2. For free-text, `db.rs` queries the FTS5 index for candidates, then re-ranks
   them with a fuzzy scorer.
3. `group:` / type filters take dedicated query paths.

## Persistence

- **SQLite** opened in WAL mode with performance pragmas (`temp_store=MEMORY`,
  page cache, `mmap_size`, bounded WAL checkpoints).
- **Tables (high level):** `history`, `groups`, `item_groups`,
  `item_group_confidence`, `snippets`, `blobs`, `app_meta`, and an FTS5
  virtual table `history_fts` kept in sync by triggers.
- **`app_meta`** is a generic key/value store used for settings (see
  [CONFIGURATION.md](CONFIGURATION.md)).
- **Blob store** keeps image bytes (and thumbnails) keyed by content hash so
  identical images are de-duplicated; `history` only stores the reference.

See [CONFIGURATION.md](CONFIGURATION.md#storage-locations) for on-disk paths.

## Processes & threads

- The capture listener runs on a dedicated background thread.
- A periodic maintenance task applies retention/cleanup.
- Async commands (paste flows, etc.) run on Tauri's async runtime.

## Build pipeline

`npm run tauri dev|build` runs the SvelteKit build (`beforeBuildCommand`),
outputs static assets to `build/` (`frontendDist`), then the Tauri CLI compiles
the Rust core and bundles platform installers. See
[BUILD_AND_RELEASE.md](BUILD_AND_RELEASE.md).
