# Third-party notices

Codex Unified Monitor is an independent MIT project. It is not affiliated with or endorsed by OpenAI. Codex, ChatGPT and OpenAI are trademarks of their respective owners.

| Project | Commit | License | Reused material |
| --- | --- | --- | --- |
| poer2023/CodexScope | dea9cdcfd97573038feca75899259ce6a67265e8 | MIT | Application icons; Tauri/React scaffold; adapted menu-bar positioning in src-tauri/src/tray.rs; chart glyph and stack structure; parser state/append manifest design ported to the core parser and SQLite ingest |
| HduSy/tokenscope | Transitive material retained at the pinned CodexScope commit; no separate upstream snapshot fetched | MIT, Copyright (c) 2026 HduSy | Original desktop/chart lineage; original license and NOTICE preserved verbatim |
| YUHAO-corn/codex-usage-dashboard | 267b9a2516cdfdf844e0dda062f881f5a4dd9338 | MIT | `summarize`, bucket accumulation, cache ratio, calendar period and local-report patterns ported from Python to core analytics/export; no runtime Python code shipped |
| zhang-mengjia/codex-usage-dashboard | 406ebadbf8b0607f0fdb6cf17caceb37df1a2678 | MIT, Copyright (c) 2026 MENGJIA ZHANG | Persistent JSON-RPC client, executable discovery and rate-limit normalization ported from CJS to core account; no Electron or control/OAuth code shipped |

Exact upstream license texts are in [licenses/](licenses/), including CodexScope's original NOTICE. They are included in the application bundle. Main direct dependencies are Tauri, React, TypeScript, Vite, serde, serde_json, chrono/chrono-tz, rusqlite/SQLite, sha2, notify, dirs, anyhow and the official Tauri autostart/single-instance plugins. Lockfiles pin the resolved versions; each dependency retains its own license metadata.

OpenAI account data, local session contents and authentication credentials are not included in this repository or release. Public prices are factual data, separately sourced and dated in pricing_catalog.json.
