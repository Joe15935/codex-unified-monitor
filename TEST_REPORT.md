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

## Final installed build and public CI

- [GitHub Actions run 34454413948](https://github.com/Joe15935/codex-unified-monitor/actions/runs/34454413948): Linux checks and macOS Apple Silicon app build both SUCCESS. This run validates code commit `f6de13803ccaa31e459cf2fa7ee6e7ecb9507112`; later acceptance-report edits do not change application code.
- Dashboard closed, 20.006-second CPU-time-delta measurement: application 0.250% CPU / 108.72 MiB RSS; owned provider 0.150% CPU / 54.42 MiB RSS. This short observation includes ordinary watcher/poll activity and excludes OS-managed WebKit helpers; it is not a long-duration benchmark.
- Shipped archive: 2.49 MiB. Installed binary hash exactly matches the release binary. Ad-hoc signature verification passed. Bundle scan found no real JSONL, authentication files, database or validation snapshots; build-user home paths are remapped out of the binary.
- Native HTML and model JSON exports were generated and their contents checked. Quota CSV was verified from real data through the same Rust exporter, including blank missing-five-hour fields. Native CSV dropdown automation was not completed; the CSV format itself and its structured columns passed real-data and automated checks.
- HTML structural/data checks passed. **HTML rendered visual review is NOT_TESTED:** Browser's URL security policy refused the local file URL; no alternate browser or workaround was used. The native application itself was visually reviewed with real data.


## v0.2.0 Pro Tier Auditor acceptance

Implementation and release candidate date: 2026-09-10. This section supersedes v0.1.0 results only for the new audit feature.

- Core suite: 51 tests, including 18 new audit tests. The new tests cover independent arithmetic, unchanged-poll aggregation, unique workload counting, all three classifications, sparse/unstable evidence, frozen prices, Fast/subagent/context/model/effort exclusions, reset boundaries, credit changes between unchanged polls, saturation-related boundaries, gaps, cross-account isolation, stale current data, independent reference provenance, checksum/arithmetic/overlap validation and redaction. Tests use synthetic rates/accounts and do not supply a production baseline.
- Native watcher integration: 1 passed. TypeScript and production frontend build passed. App/DMG packaging and ad-hoc signing passed; final release hashes identify the distributed build.
- Real database COPY migration: all 37,871 event identities/counters and 44 quota snapshots preserved through v2 → v3. Original database was not migrated during this isolated check. Existing app and its database were separately backed up before installing the new bundle.
- Five real observed intervals independently reconciled against SQL token counters and separately calculated model-rate costs, with zero dollar difference. No controlled-run declarations or baseline were fabricated; eligible windows remained zero and assessment INCONCLUSIVE.
- Isolated browser rendering: actual React app/component with a read-only fixture from the validated data copy and a simulated IPC adapter. The audit layout, explicit missing-baseline/unknown-Fast state, interval details, eligible filter, model/reasoning controls and declaration checkboxes were rendered and exercised. This is frontend verification, not installed native IPC acceptance. Preview files stay outside the repository.
- Installed native v0.2.0 startup, audit page and export-button flow: PASS. After the old process exited, the installed final bundle launched with one owned official provider. Native screenshots show the real audit page, LIVE official data after Refresh, INCONCLUSIVE assessment, disabled observation start without declarations, and the working eligible-only filter. Export Evidence Report generated JSON, HTML and CSV together and displayed “Saved 3 report files.” No controlled-run declarations or baseline were fabricated.
- Installed database migration: all 37,871 event identities/counters and all 46 quota snapshots in the later installation backup were retained in schema v3. New local events and official snapshots continued to arrive. This is separate from the earlier 44-snapshot copy test above.
- Native export at 2026-09-10 09:42:16 UTC: seven intervals (including the pending tail) independently matched SQL token counters and model-rate arithmetic; maximum dollar difference was 2.49e-14 from floating-point summation. The JSON checksum, seven matching CSV rows, owner-only file permissions (0600), and removal of account/original session/response identifiers passed.
- Audit HTML/JSON/CSV renderers passed data, escaping, integrity and privacy checks. Browser rendering of exported local HTML remains NOT_TESTED because the browser refused local file navigation; no alternate rendering path was used. Controlled-run and baseline validation logic passed core tests; native start/stop and baseline-import UI were not exercised with fabricated declarations on the real account.

Release status: preview for the observational classification method; native startup and export acceptance are complete. Missing Pro 5x reference and insufficient controlled observations are expected INCONCLUSIVE conditions, not inferred entitlement findings. Longitudinal field validation, Apple notarization and an Intel build remain outstanding.


### v0.2.0 public build verification

[GitHub Actions 34460097368](https://github.com/Joe15935/codex-unified-monitor/actions/runs/34460097368): Linux checks and macOS build both SUCCESS for code commit `823a5a2e56776d392e74000927030de30e8b423c`. Later edits to this acceptance record do not change application code. The running installed v0.2.0 binary matches the packaged binary. An unauthenticated public release ZIP download matched the SHA256 below.

Release ZIP SHA256: `13bfa20be4a10c115059708c291942fc27da38ca808350596d9f2350334018c4`.
Release DMG SHA256: `cb7e220c26112bd7217d82f7e9be112d4f4b3a2c193eeb2d6c4c93d4ffb1d72c`.
Ad-hoc signature and DMG filesystem integrity checks PASS. Packaged scan found no real logs, database, authentication files or build-user home path.

## v0.2.1 bilingual acceptance

Executed on 2026-09-10. Application code: `008c6f162be9de53de631b15dfab8f1a8533c899`. This release adds presentation translations and a language preference; it does not change accounting, audit classification thresholds or the database schema. No new production dependency was added.

- `npm test`: 4 JavaScript localization tests and 54 Rust core tests PASS. Coverage includes persisted locale, both switch directions, placeholder parity, dictionary coverage, old preferences, invalid locale rejection, HTML escaping, unchanged JSON/CSV and audit checksums.
- `cargo test --manifest-path src-tauri/Cargo.toml --test watcher`: 1 native integration test PASS. Both Cargo format checks PASS. `npm run build` and `npm run package -- --bundles app,dmg` PASS.
- [GitHub Actions 34467470058](https://github.com/Joe15935/codex-unified-monitor/actions/runs/34467470058): Linux checks and macOS Apple Silicon build both SUCCESS for the code commit above. Subsequent acceptance-record edits do not change application code.
- Actual React app in an isolated browser preview: all seven pages rendered in both languages; English restored after navigation/reload. Overview, audit and settings layouts visually checked. Preview data and adapter remain outside the repository; this check is separate from native IPC acceptance.
- Installed native application: v0.2.1 startup with live official quota and continuing local ingestion PASS. Dashboard switching to English persisted to SQLite; graceful quit/relaunch restored English. Switching back to Chinese from the compact panel updated the main window. Both compact-panel languages and the Chinese tray menu visually checked. Other saved preference fields exactly matched the pre-upgrade backup.
- Final native Chinese overview and audit-page screenshots PASS. The visual pass found Chinese chart-unit wrapping; the final build uses a content-sized, non-wrapping axis gutter. The final installed executable exactly matches the packaged executable.
- Native audit export button generated English and Chinese HTML with JSON/CSV companions. Language attributes, translated headings, original assessment identifiers, 0600 permissions and canonical JSON checksums PASS. Real evidence remained INCONCLUSIVE; no baseline or controlled-use declarations were fabricated.
- Standalone HTML visual rendering remains NOT_TESTED because the browser policy previously refused local exported-file navigation. HTML content, escaping and integrity are verified; no alternate rendering workaround was used. This does not affect the native application visual checks above.
- Existing v0.2.0 application and schema-v3 database were backed up before replacement. Source/build scan found no private home paths, account keys, real session logs, authentication files or database in the shipped bundle.

Final ZIP SHA256: `1f92b1d2dfe57a27cf6cfc57e0a6d93fa51497cc0a324396821c6d1b8418ebf8`.
Final DMG SHA256: `16867ac42acf7db88a074b3270fe19d2fc9ffdb7812f87c4bdab8ed81094e6be`.
Ad-hoc signature and DMG filesystem integrity PASS. Apple notarization, Intel binaries and longitudinal field validation of tier classification remain outside this release. Unknown external errors keep their original text; canonical JSON/CSV fields remain English for compatibility.

Public [v0.2.1 release](https://github.com/Joe15935/codex-unified-monitor/releases/tag/v0.2.1) verification: unauthenticated ZIP and DMG downloads both returned HTTP 200 and matched the final SHA256 values above. The repository and release are public; the release is marked preview because tier classification still requires longitudinal field validation.
