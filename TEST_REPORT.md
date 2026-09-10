# Validation report — v0.1.0

Executed on an Apple Silicon Mac on 2026-09-10. Real session files and private UI/account evidence remain outside the repository. This report publishes acceptance summaries, not account identity, project paths, prompts or detailed usage exports.

## Automated checks

- `cargo test --manifest-path crates/core/Cargo.toml`: 33 tests passed.
- `cargo test --manifest-path src-tauri/Cargo.toml --test watcher`: 1 actual OS notification / incremental-index integration test passed.
- `npm run build`: TypeScript and production frontend build passed.
- `cargo fmt --manifest-path crates/core/Cargo.toml -- --check` and corresponding desktop check: passed.
- `npm audit --audit-level=high`: zero reported vulnerabilities at validation time.
- `npm run package -- --bundles app,dmg`: Apple Silicon app and DMG built.
- `codesign --verify --deep --strict` on the application bundle: passed (ad-hoc signature, not notarization).
- Upstream zhang-mengjia's original `node --test tests/*.test.cjs`: 20 passed during research.

| Requested risk | Coverage / outcome |
|---|---|
| Cached input included in raw | canonical counters and invalid-counter tests PASS |
| Reasoning double count | canonical totals plus three manual-cost cases PASS |
| Cumulative token_count | duplicate cumulative and modern/legacy reconciliation PASS |
| Archived copies | append/restart/rename/archive/rescan test PASS |
| Moves and renames | persistent inode checkpoint test PASS |
| Day timezone | IANA local calendar bounds PASS |
| Monday week boundary | calendar-week test PASS |
| DST | spring 23-hour and fall 25-hour tests PASS |
| Model alias | explicit alias test; no implicit fallback PASS |
| Unknown model | no price, no silent zero and explicit coverage PASS |
| Reset seconds/ms | timestamp normalization PASS |
| Cached data shown as Live | restart is CACHED; disconnect native UI is unavailable PASS |
| 429 retry storm | bounded exponential-backoff test; manual refresh respects worker deadline PASS |
| Duplicate app-server children | one fake child serves repeated reads; actual app owns one; quit reaps it PASS |
| Token/error disclosure | read allowlist, refresh denied, sanitized secret-bearing fake error and metadata-only storage PASS |
| Large JSONL memory | bounded-line reader test; real 1.54 GB import PASS |
| Truncate/rotation offsets | truncate/same-size replacement and partial-line tests PASS |
| Fork/subagent ownership | first thread ID, parent header and inherited snapshot regression PASS |
| OS watcher repeat delivery | native create + later append to incremental SQLite test PASS |
| Export safety | HTML/CSV escaping, quota-specific columns, no remote script resources PASS |

## Real data acceptance

**Token validation:** five stable sessions selected with deterministic random sampling, independently read with Python and compared against SQLite. Raw input, cached input, fresh input, output, reasoning and total matched 5/5. Two additional modern response-record sessions matched 2/2. The validator is `scripts/validate_local.py`; it reads original logs without modifying them.

The first real-session attempt exposed root-lineage/fork ownership errors. Those errors were fixed in parser schema v2 and the full index was rebuilt before reporting PASS. Copied fork history is excluded from new fork spend.

**Quota:** actual dashboard and independent official account source matched main-window duration, used/remaining percentage and exact reset timestamp. Main primary was a weekly window, secondary null; five-hour quota correctly remained Unavailable. Spark's windows were separate. Reset-credit count was returned as zero. Values naturally changed during active use; the check compared near-contemporaneous snapshots.

**Cost/cache:** three distinct real model aggregates (Astra, Sol, Terra) were independently recalculated from raw/cached/output counters and current official rates. All matched, maximum floating-point difference below 0.000000000004 USD. Synthetic manual scenarios also cover cache savings and no reasoning double charge. SQLite scan found no cached > raw or reasoning > output violations.

**Dedup:** unchanged second rescan read zero bytes and inserted zero events. App quit/relaunch preserved the exact ID/counter digest of 36,893 events in 119 closed sessions. Active logs can flush events later, so a moving timestamp cutoff alone is not a valid immutable baseline; the check freezes closed session identities. Original logs were never changed.

**Native UI:** installed in Applications; actual data rendered in six pages, all nine time-range controls and session details. Compact menu-bar panel rendered quota, estimates, cache and buttons. Settings could create/remove the own LaunchAgent. Disconnect reaped the owned provider child and clearly marked quota unavailable; reconnect restored it. Graceful quit stopped both the app and its child. Launch at Login remains off by default.

## Performance and corrections

- Corrected initial release-mode import: 136 files, 1,541,623,525 bytes, 3,206 ms, 37,432 inserted metadata events; 0 parse errors, 139 bounded oversized lines skipped.
- Independent oversized-record audit found no skipped token_count or token_usage_record among those 139 lines.
- Observed append import: 2,414,010 newly available bytes, 18 ms; immediately repeated unchanged rescan: 0 bytes, 2 ms.
- The initial FSEvents backend did not reliably deliver real appends on this Mac. A read-only probe got zero events while files grew; kqueue got 16 events in a 15-second observation. The app now uses kqueue over the log trees, with event debounce and a 15-minute safety reconciliation. The installed fix caught up automatically and consecutive observations had zero unread bytes, without manual ingest.
- During a 20-second background observation with active logs, app RSS peaked at 114.85 MiB and its official provider at 116.27 MiB. These figures exclude OS-managed WebKit helpers. The app's sampled CPU was 1.77%, provider 0%; this active-ingestion observation is not a claim of idle CPU.

## Limits

No Apple Developer ID notarization or Intel release was attempted. Full reboot/login execution was not tested; LaunchAgent creation/removal and background argv were verified. Official five-hour/reset-detail availability depends on the account service. Rates are current base estimates applied to history; long-context, cache-write and service-tier adjustments are not reconstructed. No independent OAuth grant is created, so disconnecting this component does not revoke the official client's shared sign-in. Forecasts remain Collecting data until their sample conditions are met. Very large numbers of files can hit the OS descriptor limit of the kqueue backend and are reported as a watcher limitation. GitHub CI is separate from local acceptance results; its current status appears on the repository Actions page.
