# Architecture

The shipped app has one application process, two WebKit windows (dashboard and compact tray panel), one worker thread, and at most one owned official `codex app-server --stdio` child. WebKit may use macOS-managed helper processes; “single app” does not imply a single OS PID. No independent report daemon, Python runtime, Electron, or production HTTP server is included.

`crates/core` is the shared Rust accounting library and optional developer CLI. Modules are `accounting`, `parser`, `ingest`, `storage`, `account`, `pricing`, `analytics`, `settings`, and `export`. `src-tauri/src/worker.rs` owns the filesystem watcher, queue, reconciliation and account polling. `tray.rs` adapts the CodexScope placement approach. React/TypeScript renders serializable reports through Tauri IPC.

## Local ingestion

A first scan indexes `sessions` and `archived_sessions`. Native kqueue notifications then queue changed paths; 250 ms debounce coalesces bursts. A 15-minute metadata reconciliation catches missed notifications. macOS kqueue was selected after an independent FSEvents probe failed to deliver local changes; kqueue delivered repeated live appends. Only the log trees are recursively watched, limiting file descriptors. Dashboard reads query SQLite and do not reopen historical logs.

Each file checkpoint records device/inode, path, nanosecond mtime, size, committed byte offset, head and offset-tail hashes, parser state, and error counters. Appends read from the checkpoint. Moves preserve the inode checkpoint. Truncation, same-size changes, and fingerprint changes rebuild that source's event links transactionally. File-to-event references preserve deduplicated events shared by copied/archived files. Partial final lines wait for a newline. Lines over 2 MiB are drained with bounded allocation and reported as skipped.

SQLite uses WAL, transactions, indexes, foreign keys, a busy timeout, schema versioning and owner-only permissions. v2 rebuilds the app's derived v1 session index to fix parent-lineage identities; settings, pricing and quota history survive. Original rollout files are never modified. Current aggregation streams rows from SQLite rather than loading raw history into memory.

## Account provider

The worker owns and reuses one stdio child. Only initialize, account/read with refreshToken=false, account/rateLimits/read, and account/usage/read are allowlisted. No session creation or account writes. Default poll is 90 seconds; optional usage summary is fetched every 15 minutes. Manual refresh has a 15-second floor and cannot bypass backoff. Failures exponentially back off to one hour and drop/reap the child before reconnecting. Quit and disconnect dispose of the child. Upstream stderr/error bodies are not logged.

Quota windows use actual duration, not primary/secondary position. Account identity is hashed for local history and masked for display. Reset-credit parsing stays inside the provider. No independent OAuth or undocumented backend is needed for the currently available official interface.

## UI and settings

One persisted Settings object controls timezone, polling, tray metric, theme, pricing mode, explicit aliases/rates, thresholds and optional subscription cost. A Tauri LaunchAgent adds only this app's login item when requested. Close hides; Quit stops the worker. The single-instance plugin returns subsequent launches to the existing app.

App CSP restricts scripts to bundled assets and connections to IPC. No remote content or arbitrary shell command is exposed to webview IPC. Exports use fixed datasets, validated formats, escaped text and an app-owned directory. GitHub releases are built from the tagged source and lockfiles. See PRIVACY.md and TEST_REPORT.md for boundaries and observed acceptance.

## Tier Auditor (v0.2.0)

`auditor.rs` aligns successful official snapshots with indexed local events, stores controlled-run declarations and frozen prices, validates imported references and renders redacted reports. Schema v3 is additive over v2. The UI loads audit calculations only on the Tier Auditor page; no provider or background process is added. See AUDITOR.md.
