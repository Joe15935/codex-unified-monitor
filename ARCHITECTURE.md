# Architecture

The app has one application process, two webview windows (dashboard and compact tray panel), one worker thread, and at most one owned official `codex app-server --stdio` child. macOS uses WebKit; Windows uses WebView2. Webviews may use OS-managed helper processes; “single app” does not imply a single OS PID. No independent report daemon, Python runtime, Electron, or production HTTP server is included.

`crates/core` is the shared Rust accounting library and optional developer CLI. It owns accounting, parsing, ingestion, storage, account data, pricing, analytics, settings, exports, localization and the Auditor. `src-tauri/src/worker.rs` owns the filesystem watcher, queue, reconciliation and account polling; `refresh_policy.rs` holds deterministic scheduling/recovery rules. `tray.rs` adapts the CodexScope placement approach and uses display work areas for Windows tray placement. React/TypeScript renders serializable reports through Tauri IPC.

## Local ingestion

A first scan indexes `sessions` and `archived_sessions`. Native notifications queue changed paths; a 250 ms debounce coalesces bursts. Only the log trees are recursively watched. The macOS kqueue backend uses a bounded 128-node eligibility check because it opens a descriptor per node; larger trees or failed registrations use 60-second metadata reconciliation. Partial registrations are dropped, ingestion errors retry, and watcher availability/root changes are checked for recovery. A healthy native watcher retains a 15-minute reconciliation safety net. Windows uses notify's native Windows backend. These are metadata checks: unchanged logs are not reparsed, and dashboard reads query SQLite.

Each file checkpoint records device/inode, path, nanosecond mtime, size, committed byte offset, head and offset-tail hashes, parser state, and error counters. Appends read from the checkpoint. Moves preserve the inode checkpoint. Truncation, same-size changes, and fingerprint changes rebuild that source's event links transactionally. File-to-event references preserve deduplicated events shared by copied/archived files. Partial final lines wait for a newline. Lines over 2 MiB are drained with bounded allocation and reported as skipped.

SQLite uses WAL, transactions, indexes, foreign keys, a busy timeout and schema versioning. Unix files receive owner-only permissions; Windows storage inherits the user's local profile access controls. v2 rebuilds the app's derived v1 session index to fix parent-lineage identities; settings, pricing and quota history survive. Original rollout files are never modified. Current aggregation streams rows from SQLite rather than loading raw history into memory.

New `thread_settings_applied` records can provide current service-tier metadata only for their logical owner and outside known fork replay. Explicit unknown/null clears evidence; missing per-turn metadata never inherits an earlier Fast-off assumption. The v0.3.0 compatibility change does not recalculate historical rows or modify accounting identities. See [research and protocol evidence](docs/RESEARCH-2026-09-12.md).

## Account provider

The worker owns and reuses one stdio child. Only initialize, account/read with refreshToken=false, account/rateLimits/read, and account/usage/read are allowlisted. No session creation or account writes. The default interval is 90 seconds with adaptive refresh enabled: real ingestion activity keeps the configured cadence, while idle polling relaxes to at most 5 minutes (or preserves a longer configured interval). An active controlled audit uses the fixed configured cadence. Optional usage summary is fetched every 15 minutes. Manual refresh has a 15-second floor and cannot bypass backoff. Failures exponentially back off to one hour and drop/reap the child before reconnecting. Quit and disconnect dispose of the child. Upstream stderr/error bodies are not logged.

Windows discovers a native `codex.exe`, including supported npm package locations, and launches it without a console window. Shell wrappers such as `codex.cmd` are not executed. An explicit `CODEXMETER_CODEX_BINARY` override must point to the native executable. No WSL bridge, shell interpolation, helper daemon or extra credential store is introduced.

Quota windows use actual duration, not primary/secondary position. Account identity is hashed for local history and masked for display. Reset-credit parsing stays inside the provider. No independent OAuth or undocumented backend is needed for the currently available official interface.

## UI and settings

One persisted Settings object controls language, timezone, adaptive/fixed polling, tray metric, theme, pricing mode, explicit aliases/rates, thresholds and optional subscription cost. Quota pace and low-quota highlighting stay inside existing UI cards; the default remaining threshold is 10%, and 0 disables it. They are not audit evidence or OS notifications. Language/theme/pricing changes do not trigger quota requests. The Tauri autostart plugin registers only this app when requested (LaunchAgent on macOS, user login startup on Windows). Close hides; Quit stops the worker. The single-instance plugin returns subsequent launches to the existing app.

App CSP restricts scripts to bundled assets and connections to IPC. No remote content or arbitrary shell command is exposed to webview IPC. Exports use fixed datasets, validated formats, escaped text and an app-owned directory. GitHub releases are built from the tagged source and lockfiles. See PRIVACY.md and TEST_REPORT.md for boundaries and observed acceptance.

Windows NSIS configuration installs for the current user, offers English/Chinese setup and uses the WebView2 bootstrapper when needed. Its private metadata lives under `%LOCALAPPDATA%`, while the existing macOS Application Support path stays unchanged. Windows build, synthetic native tests and interactive/live-account acceptance are separate release gates; Windows support remains beta until the latter is documented.

## Tier Auditor (v0.2.0)

`auditor.rs` aligns successful official snapshots with indexed local events, stores controlled-run declarations and frozen prices, validates imported references and renders redacted reports. Schema v3 is additive over v2. The UI loads audit calculations only on the Tier Auditor page; no provider or background process is added. See AUDITOR.md.
