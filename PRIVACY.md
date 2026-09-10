# Privacy and security

Local first. Read only. No telemetry. No cloud sync.

The monitor reads rollout metadata and counters from Codex sessions. It does not store prompts, assistant messages, reasoning text, file bodies, tool arguments or tool output. Its database contains session IDs, local project paths, models, effort, timestamps, tool names/IDs, token counts, file checkpoints, masked account details, hashed account identity, official quota snapshots, pricing and settings. These are still private metadata. Exports contain metadata and should be shared deliberately.

The app data directory is owner-only; its SQLite database and exports are owner-readable/writable. Nothing is uploaded by this component. The official Codex app-server child communicates with official account services to retrieve quota. It is launched with analytics disabled for that process. Existing Codex settings/authentication are not changed. There are no credential inputs, authentication-file readers, OAuth refresh-token stores, account mutation calls or reset-credit redemptions. Upstream stderr and upstream error text are suppressed to avoid logging secrets.

Production starts no HTTP server. The development server binds to loopback. The packaged webview loads bundled code under a restrictive CSP. Exports are escaped standalone files without remote JS/CSS. HTML is generated locally and CSV cells protect against spreadsheet formula injection.

Settings → Disconnect account stops this component's provider; it does not revoke or sign out the official client's shared authentication, because the component has no independent OAuth grant. Reconnect resumes read-only calls. Disable Launch at Login before uninstalling; remove the application and, if desired, its own Application Support folder. Original Codex files and other apps are untouched.

Public source includes synthetic fixtures only. Do not commit real rollout JSONL, SQLite files, private screenshots, account snapshots, environment files, exported reports or tokens. Report security problems privately to the maintainer through the repository's available private reporting mechanism; never include credentials in public issues.
