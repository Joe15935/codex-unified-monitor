# Codex Unified Monitor

**English** · [简体中文](README.zh-CN.md)

A local macOS menu bar and Windows system-tray app for Codex quota, token accounting, cache health, and estimated API equivalent value. One Tauri 2 application, one SQLite index, and one persistent official Codex app-server child. No Electron, Python, web server, telemetry, or cloud sync in the shipped app.

**[Download v0.3.1 · macOS + Windows beta](https://github.com/Joe15935/codex-unified-monitor/releases/tag/v0.3.1)** · [Accounting](ACCOUNTING.md) · [Privacy](PRIVACY.md) · [Validation](TEST_REPORT.md)

**Preview boundaries:** Windows support is beta; build/test results and interactive Windows acceptance are reported separately in [TEST_REPORT.md](TEST_REPORT.md). Tier Auditor still needs independent, controlled field observations to validate its classification method. The [previous stable release](https://github.com/Joe15935/codex-unified-monitor/releases/latest) remains available.

## Install

| Platform | Package and requirements |
| --- | --- |
| macOS | Download the `.dmg` or `.app.zip` and place **Codex Unified Monitor.app** in `/Applications`. Requires macOS 12+ on Apple Silicon. Intel binaries are not provided. |
| Windows beta | Download the x64 NSIS setup `.exe` for Windows 10/11. Installs for the current user; the installer supports English and Simplified Chinese. Microsoft WebView2 is required; setup can download its bootstrapper when missing. |

Sign in using the official Codex CLI or ChatGPT/Codex desktop app first; this app never asks for your credentials. On Windows, install the official Windows Codex CLI if its native `codex.exe` cannot be found; a CLI installed only inside WSL is not a native Windows provider. Open the app, then click its menu bar/tray icon for the compact panel. Closing the dashboard keeps monitoring active. Quit from the compact panel or tray menu. Launch at Login is optional.

macOS releases are ad-hoc signed, **not Apple notarized**; Windows beta installers are not commercially code-signed. Gatekeeper or SmartScreen may require approval. Verify the release checksums and publisher source; do not disable system protections globally. Source builds are also supported.

## Language

Choose **中文 / English** at the top of the dashboard, in Settings, or in the compact panel. Chinese is the default. The choice is saved locally and survives restart; the dashboard, compact panel, tray menu and newly generated HTML reports follow it. Switching language does not change prices, token counts, quota readings or audit controls.

Dates and compact numbers follow the selected language; the configured time zone remains the same. Model identifiers, protocol values and user-entered names remain original. JSON and CSV keep stable English field names and machine-readable values, so evidence checksums, baseline imports and existing integrations remain compatible. Unrecognized external error messages are preserved verbatim. See [LOCALIZATION.md](LOCALIZATION.md) for contributors.

## What it shows

- Official account quota with explicit used/remaining percentages, reset dates, reset credits when provided, and separate model buckets.
- Local raw input, cached input, fresh input, output, reasoning, total tokens, and cache ratios.
- Today, rolling 5h, calendar week/month/year, 30d, 90d, all time, and custom dates, in a configurable IANA timezone.
- Model and session breakdowns; rankings by equivalent value, tokens, cache hit, duration and output; session detail timelines.
- Two separate pricing catalogs: Public API and Codex / Work. Unknown models stay UNPRICED. Custom aliases and rates are explicit settings.
- Observed quota burn and efficiency after at least four samples over 30 minutes with a measurable change; optional subscription equivalent value multiple.
- A quota pace reference and configurable in-app low-quota highlighting: default 10% remaining, or 0 to disable. These are planning cues, not official consumption rules or tier evidence; they do not send OS notifications.
- Adaptive refresh, enabled by default: the configured interval while active, less frequent polling while idle, and a fixed configured cadence during controlled audits. Local ingestion continues independently and recovers automatically from watcher errors.
- Local CSV, JSON and standalone HTML exports. Launch at Login is optional.
- Chinese and English interface, tray labels and HTML reports, with a persistent language switch.

**API equivalent value is an estimate, not your subscription bill or OpenAI's cost.** Current base prices are applied to recorded token counts; historical prices, request-level long-context adjustments, cache writes and service tiers are not reconstructed. See [PRICING.md](PRICING.md).

A `primary` quota field is not necessarily a five-hour window. Windows are classified by their actual duration. Missing values remain Unavailable. Account failures mark older readings Cached/Stale while local tokens remain accessible.

With the default 90-second interval, adaptive polling increases to at most 5 minutes during inactivity. A longer user-configured interval is respected. Turn adaptive refresh off for a fixed cadence; provider backoff still applies after failures. Settings changes to language, theme or pricing do not force additional account requests.

v0.3.1 reduces repeated work during use: countdown ticks update their own widgets, overlapping refresh triggers share sequential reads, session details use indexed lookups, and bounded caches reuse number/date formatters by language and time zone. The existing layout, accounting rules and Tauri/SQLite stack are preserved, with no new production dependencies. See the [research record](docs/RESEARCH-2026-09-12.md) and [validation results](TEST_REPORT.md) for evidence and remaining limits; isolated benchmarks do not establish a whole-app speed multiplier.

## Pro Tier Auditor / 套餐额度审计

The new **Tier Auditor** page aligns each observed weekly quota change with local tokens, records controlled observations and frozen prices, compares against a user-supplied independent Pro 5x reference, and exports redacted JSON/HTML/CSV evidence. Missing or incomparable evidence stays **INCONCLUSIVE**. It reports capacity resemblance, never a proven entitlement error. No reference dollar amount is invented, and reports are not sent anywhere automatically. See [AUDITOR.md](AUDITOR.md) for setup, thresholds and limitations.

## Build from source

Install Node.js 22.12+, Rust stable, and official Codex. macOS builds need Xcode command line tools; Windows builds need the MSVC Rust toolchain, Visual Studio Build Tools with Desktop development with C++, a Windows SDK, and WebView2. Python 3 is optional for independent real-log validation only.

```sh
npm ci
npm test
npm run build
```

Build the desktop package on its target OS:

```sh
# macOS Apple Silicon
npm run package -- --bundles app,dmg

# Windows x64
npm run package -- --target x86_64-pc-windows-msvc --bundles nsis
```

Builds appear under `src-tauri/target/` in the selected target's `release/bundle/` directory. For development use `npm run tauri dev`; its development server binds only to `127.0.0.1`. The packaged app starts no HTTP server.

```sh
npm run data -- --help
python3 scripts/validate_local.py
```

The data CLI reads real local metadata. Its output and exports are private; do not commit them or attach them to public issues. Use synthetic fixtures for bug reports.

## Data and controls

Reads `CODEX_HOME/sessions` and `CODEX_HOME/archived_sessions`, defaulting to `~/.codex` (`%USERPROFILE%\.codex` on Windows). The app's own database is `~/Library/Application Support/Codex Unified Monitor/monitor.sqlite3` on macOS and `%LOCALAPPDATA%\Codex Unified Monitor\monitor.sqlite3` on Windows. It does not use Windows roaming storage.

Settings → Disconnect account stops only this component's provider; it does not sign you out of official Codex. Disable Launch at Login before uninstalling. Removing this app and its own data folder does not alter Codex sessions or authentication.

For the Chinese installation guide, feature explanation and audit workflow, see [简体中文说明](README.zh-CN.md) and [套餐额度审计](AUDITOR.zh-CN.md).

## Reuse and license

MIT. Built on the architecture and selected ports from [CodexScope](https://github.com/poer2023/CodexScope), reporting ideas from [YUHAO-corn/codex-usage-dashboard](https://github.com/YUHAO-corn/codex-usage-dashboard), and a Rust adaptation of the persistent provider approach from [zhang-mengjia/codex-usage-dashboard](https://github.com/zhang-mengjia/codex-usage-dashboard). Exact upstream revisions, inherited HduSy/tokenscope notices, and actual reuse are documented in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). The original applications are not bundled or run.

The [September 2026 research record](docs/RESEARCH-2026-09-12.md) compares these projects with CodexBar and ccusage. v0.3.0 keeps the existing stack and adds conservative `thread_settings_applied` compatibility, using official Codex protocol evidence; missing or unknown speed metadata never becomes proof that Fast was off. No new production dependency is needed for these changes.
