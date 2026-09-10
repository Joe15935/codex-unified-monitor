# Codex Unified Monitor

A local macOS menu bar app for Codex quota, token accounting, cache health, and estimated API equivalent value. One Tauri 2 application, one SQLite index, and one persistent official Codex app-server child. No Electron, Python, web server, telemetry, or cloud sync in the shipped app.

**[Download the Apple Silicon release](https://github.com/Joe15935/codex-unified-monitor/releases/latest)** · [Accounting](ACCOUNTING.md) · [Privacy](PRIVACY.md) · [Validation](TEST_REPORT.md)

**Auditor preview:** [v0.2.0](https://github.com/Joe15935/codex-unified-monitor/releases/tag/v0.2.0) adds the Tier Auditor. Native startup acceptance is pending; see [TEST_REPORT.md](TEST_REPORT.md). The existing stable release remains available.

## Install

Download the `.dmg` or `.app.zip`, then place **Codex Unified Monitor.app** in `/Applications`. Requires macOS 12+ on Apple Silicon. Sign in using the official Codex CLI or ChatGPT/Codex desktop app first; this app never asks for your credentials. Open the app, then click its menu bar icon for the compact panel. Closing the dashboard keeps monitoring active. Quit from the compact panel or tray menu.

Releases are ad-hoc signed, **not Apple notarized**. macOS may require you to approve this downloaded app in Privacy & Security. Do not disable Gatekeeper globally. Source builds are also supported. Intel binaries are not provided in v0.1.0.

## What it shows

- Official account quota with explicit used/remaining percentages, reset dates, reset credits when provided, and separate model buckets.
- Local raw input, cached input, fresh input, output, reasoning, total tokens, and cache ratios.
- Today, rolling 5h, calendar week/month/year, 30d, 90d, all time, and custom dates, in a configurable IANA timezone.
- Model and session breakdowns; rankings by equivalent value, tokens, cache hit, duration and output; session detail timelines.
- Two separate pricing catalogs: Public API and Codex / Work. Unknown models stay UNPRICED. Custom aliases and rates are explicit settings.
- Observed quota burn and efficiency after at least four samples over 30 minutes with a measurable change; optional subscription equivalent value multiple.
- Local CSV, JSON and standalone HTML exports. Launch at Login is optional.

**API equivalent value is an estimate, not your subscription bill or OpenAI's cost.** Current base prices are applied to recorded token counts; historical prices, request-level long-context adjustments, cache writes and service tiers are not reconstructed. See [PRICING.md](PRICING.md).

A `primary` quota field is not necessarily a five-hour window. Windows are classified by their actual duration. Missing values remain Unavailable. Account failures mark older readings Cached/Stale while local tokens remain accessible.

## Pro Tier Auditor / 套餐额度审计

The new **Tier Auditor** page aligns each observed weekly quota change with local tokens, records controlled observations and frozen prices, compares against a user-supplied independent Pro 5x reference, and exports redacted JSON/HTML/CSV evidence. Missing or incomparable evidence stays **INCONCLUSIVE**. It reports capacity resemblance, never a proven entitlement error. No reference dollar amount is invented, and reports are not sent anywhere automatically. See [AUDITOR.md](AUDITOR.md) for setup, thresholds and limitations.

## Build from source

Install Node.js 22.12+, Rust stable, Xcode command line tools, and official Codex. Python 3 is optional for independent real-log validation only.

```sh
npm ci
npm test
npm run build
npm run package -- --bundles app,dmg
```

Builds appear in `src-tauri/target/release/bundle/`. For development use `npm run tauri dev`; its development server binds only to `127.0.0.1`. The packaged app starts no HTTP server.

```sh
npm run data -- --help
python3 scripts/validate_local.py
```

The data CLI reads real local metadata. Its output and exports are private; do not commit them or attach them to public issues. Use synthetic fixtures for bug reports.

## Data and controls

Reads `CODEX_HOME/sessions` and `CODEX_HOME/archived_sessions`, defaulting to `~/.codex`. Own metadata database: `~/Library/Application Support/Codex Unified Monitor/monitor.sqlite3`. Settings → Disconnect account stops only this component's provider; it does not sign you out of official Codex. Disable Launch at Login before uninstalling. Removing this app and its own data folder does not alter Codex sessions or authentication.

English UI in v0.2.0. 中文说明：这是一个本地优先、只读、无遥测的 Codex 菜单栏组件。额度来自官方接口，Token 来自本机日志，美元数字是 API 等价估值。没有提供的五小时额度或模型价格会明确标记，不会编造为零。下载页面提供可直接安装的 Apple Silicon 应用。

## Reuse and license

MIT. Built on the architecture and selected ports from [CodexScope](https://github.com/poer2023/CodexScope), reporting ideas from [YUHAO-corn/codex-usage-dashboard](https://github.com/YUHAO-corn/codex-usage-dashboard), and a Rust adaptation of the persistent provider approach from [zhang-mengjia/codex-usage-dashboard](https://github.com/zhang-mengjia/codex-usage-dashboard). Exact upstream revisions, inherited HduSy/tokenscope notices, and actual reuse are documented in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). The original applications are not bundled or run.
