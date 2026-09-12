# Privacy and security

Local first. Read only. No telemetry. No cloud sync.

The monitor reads rollout metadata and counters from Codex sessions. It does not store prompts, assistant messages, reasoning text, file bodies, tool arguments or tool output. Its database contains session IDs, local project paths, models, effort, timestamps, tool names/IDs, token counts, file checkpoints, masked account details, hashed account identity, official quota snapshots, pricing and settings. These are still private metadata. Exports contain metadata and should be shared deliberately.

On macOS, the app data directory is owner-only and its SQLite database and exports are owner-readable/writable. On Windows, data is stored in `%LOCALAPPDATA%\Codex Unified Monitor` and inherits the current user's profile access controls; no custom ACL hardening or database encryption is claimed. It is not placed in the roaming profile. Nothing is uploaded by this component. The official Codex app-server child communicates with official account services to retrieve quota. It is launched with analytics disabled for that process. Existing Codex settings/authentication are not changed. There are no credential inputs, authentication-file readers, OAuth refresh-token stores, account mutation calls or reset-credit redemptions. Upstream stderr and upstream error text are suppressed to avoid logging secrets.

Adaptive refresh uses the app's existing ingestion activity and audit state. It does not inspect unrelated processes, scan other applications' data, or transmit activity signals. Native watcher fallback/recovery remains limited to the configured Codex session roots. Quota pace and low-quota highlighting are local UI cues, with no OS notifications or external messaging.

Production starts no HTTP server. The development server binds to loopback. The packaged webview loads bundled code under a restrictive CSP. Exports are escaped standalone files without remote JS/CSS. HTML is generated locally and CSV cells protect against spreadsheet formula injection. The Windows installer may download Microsoft's WebView2 bootstrapper if the runtime is absent; this installation dependency is separate from the monitor's quota collection.

Settings → Disconnect account stops this component's provider; it does not revoke or sign out the official client's shared authentication, because the component has no independent OAuth grant. Reconnect resumes read-only calls. Disable Launch at Login before uninstalling; remove the application and, if desired, its own Application Support folder on macOS or its Local AppData folder on Windows. Original Codex files and other apps are untouched. Windows launches only the resolved native Codex executable, not an npm or shell wrapper; the app does not bridge into WSL.

Public source includes synthetic fixtures only. Do not commit real rollout JSONL, SQLite files, private screenshots, account snapshots, environment files, exported reports or tokens. Report security problems privately to the maintainer through the repository's available private reporting mechanism; never include credentials in public issues.

## Tier Auditor evidence

Auditor reports omit account/email, original session/response IDs, project paths, prompts and tool payloads. Export origin IDs are random per-account local pseudonyms; workload labels are report-local ordinals. Reports retain model, reasoning, token counters, UTC timestamps and public price citations for auditability. JSON checksums prove integrity only, not account entitlement or source authenticity. Import citations are not fetched. Reports stay local until the user shares them.
