# Research and reuse decision

Verified 2026-09-10 against the current `main` snapshots, local CLI 0.148.0 and the installed ChatGPT bundled Codex 0.153.4. This document precedes implementation.

| Upstream | Pinned commit | Maintenance observed | Runtime / deployment | Reuse decision |
| --- | --- | --- | --- | --- |
| [poer2023/CodexScope](https://github.com/poer2023/CodexScope) | `dea9cdcfd97573038feca75899259ce6a67265e8` | Not archived; pushed 2026-07-16 | Tauri 2, Rust, React/TS; native WebKit; Rust/Node needed only to build | Desktop scaffold, chart glyph and menu-bar positioning; adapt parser state and append-only ingestion into SQLite |
| [YUHAO-corn/codex-usage-dashboard](https://github.com/YUHAO-corn/codex-usage-dashboard) | `267b9a2516cdfdf844e0dda062f881f5a4dd9338` | Not archived; pushed 2026-05-13 | Python 3.10 plus ccusage/Node; static local HTML | Port bucket aggregation, calendar periods, cache ratio and local export semantics into Rust; no Python/ccusage runtime |
| [zhang-mengjia/codex-usage-dashboard](https://github.com/zhang-mengjia/codex-usage-dashboard) | `406ebadbf8b0607f0fdb6cf17caceb37df1a2678` | Not archived; pushed 2026-09-03 | Electron 43; bundled Chromium/Node; extra browser memory | Port app-server lifecycle, discovery and quota normalization into a Rust account adapter; omit account and conversation control actions |

All three are MIT. CodexScope preserves Copyright (c) 2026 HduSy from `HduSy/tokenscope`; its LICENSE and NOTICE must travel with the derivative. Exact license files are retained under `licenses/`. These are snapshot observations, not claims about future maintenance or measured resource usage.

## Audited areas

CodexScope: README, LICENSE/NOTICE, Cargo/package/Tauri config; `store.rs`, `parser.rs`, `account_usage.rs`, `pricing.rs`, tray/window code and chart primitives. Yuhao: README, AGENTS, pyproject, `cli.py` parsing delegation, bucket/cost/date logic and export rendering. Zhang: README/license/package, `codex-app-server.cjs`, `codex-cli.cjs`, `usage-model.cjs`, isolated home setup, timer/auth lifecycle and tests. Zhang's local unit suite passed 20/20 without installing Electron. No upstream application was installed or started.

## Findings addressed by this project

1. Scope's `file_state_from_prefix` rereads the entire consumed prefix after each append; persist parser state instead. `read_to_end` buffers whole file tails; use bounded streaming lines.
2. Scope prunes local events after 210 days. Retain all compact metadata; never substitute official account totals into local totals.
3. Scope uses session plus cumulative total as a dedup key, but needs rotation, equal-size rewrites, archive/move and partial-line regression tests. Use cumulative vectors and stable response IDs with transactional checkpoints.
4. Scope's reset-credit fallback reads `auth.json` directly. Omit this. The installed official app-server already supplies reset count and grant/expiry detail schema.
5. Yuhao defaults unknown prices to GPT-5.5. Omit this: unknown remains UNPRICED, with explicit user aliases/custom rates.
6. Zhang defaults missing window durations and some reset text. Preserve nulls; identify five-hour/weekly by their actual minutes. The actual Codex bucket on this Mac currently has a weekly **primary** and no five-hour window. Spark is a separate bucket.
7. New real logs contain `token_usage_record` with response identity and per-response usage, alongside older `token_count`. Both formats must reconcile, including inherited histories. A nonzero cumulative total with zero last usage is a baseline, not a new token charge.
8. Cached input is part of raw input; reasoning is part of output. Use one integer accounting implementation everywhere.
9. Existing models.dev/LiteLLM fallbacks and old embedded tables are not imported. Public API and Work/Codex catalogs remain separate, dated, source-linked base-rate estimates.

## Official evidence

- [App-server protocol](https://learn.chatgpt.com/docs/app-server): initialize, account/read with refreshToken=false, account/rateLimits/read, account/usage/read. Reset details are part of rateLimits/read. Local read-only probing returned all three methods successfully. No OAuth or token extraction was needed.
- [Public API pricing](https://developers.openai.com/api/docs/pricing): standard short/long context, cache-write, Fast/Batch/Flex and promotional distinctions.
- [Work/Codex USD rate card](https://help.openai.com/en/articles/20001415-chatgpt-rate-card-enterprise-token-based-pricing): separate commercial basis; Astra Codex long-context exception and no Codex cache-write charge. This is an equivalence reference, not a personal subscription bill.

## Route and MVP

One Tauri app, one tray icon, one SQLite store and one managed persistent official app-server child. Native WebKit may use operating-system helper processes; “single application” does not mean a literal single OS PID. No HTTP server in the packaged app. No Python, Electron or remote frontend assets.

First milestone: incremental metadata ingest, accounting/dedup tests and real-session comparison. Next: live quota and stale/backoff handling. Then dated pricing, analytics, menu bar, dashboard/settings, observed metrics, exports, local acceptance and distributable release. Keep upstream-inspired modules separable and record ports in THIRD_PARTY_NOTICES. No changes to official apps, auth, original config or rollout files.

Implementation model guidance for future contributors: Sol for changes spanning parser, storage and account lifecycle because they need cross-module reasoning; Terra for isolated UI or adapter work; Luna for bounded documentation and small fixes. This is guidance, not a model switch or delegation request.

## Real-log correction discovered during acceptance

Independent per-session verification exposed a current rollout distinction absent from the initial implementation: `session_meta.id` is the thread identity while `session_id` can be the shared root lineage. Fork snapshots can embed a second parent header and replay inherited token events at the fork-header timestamp. v2 keeps the first thread identity, excludes inherited fork events from new usage and preserves millisecond boundaries. Added regressions for root/child/fork ownership and metadata-only schema migration. Re-running five random legacy sessions plus two modern response-record sessions matched all canonical counters. This finding is why synthetic tests were followed by actual local acceptance.

## Native watcher acceptance

The pinned notify 6 FSEvents backend registered successfully but delivered zero events in an independent 15-second read-only probe while local session files grew. The maintained notify project's issue 937 describes a related delivery problem (not proof of an identical root cause). Switching the same library to its native macOS kqueue backend produced 16 notifications in a comparable probe. The runtime now watches only the two log trees recursively and watches the home non-recursively for new directories. An actual OS create/append/SQLite integration test and the installed app's zero-lag follow-up validated the change. No periodic full-log scan was substituted for events.
