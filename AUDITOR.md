# Pro Tier Auditor / 套餐额度审计

Added in v0.2.0. This local observational audit reports `PRO-5X-LIKE`, `INCONCLUSIVE`, or `PRO-20X-LIKE` relative to a supplied independent comparison. It cannot establish an OpenAI backend entitlement error. There is no bundled Pro 5x dollar baseline. No complaint, report or support message is sent automatically.

## Research and reuse — 2026-09-10

The three existing upstreams remain MIT and unarchived: CodexScope last pushed July 16, YUHAO's dashboard May 13, and zhang's dashboard September 3. Their pinned revisions remain in THIRD_PARTY_NOTICES.md. Scope's Tauri structure, YUHAO's accounting approach and zhang's provider lifecycle are already adapted here. Extending those Rust/SQLite modules needs no new production dependency, service, background process or UI framework. A targeted GitHub search did not establish a ready-made controlled tier auditor suitable for direct integration.

The route is a thin extension: capture control metadata, align observed official changes to deduplicated counters, freeze prices, apply evidence gates, and export a redacted report. Existing navigation and styles remain. For future contributors, Sol suits accounting/migration/inference changes, Terra isolated UI changes, and Luna documentation; this guidance does not request a model switch or delegation.

Evidence reviewed:

- [Official usage and pricing](https://learn.chatgpt.com/docs/pricing): Pro tier comparisons are estimates; model, context, reasoning, tools, retrieval and caching affect consumption. Cloud activity and other eligible agentic features can share allowance. No fixed personal weekly API-dollar capacity is supplied.
- [Official Speed documentation](https://learn.chatgpt.com/docs/agent-configuration/speed): Fast uses allowance at a higher rate, depending on the model. Missing Fast metadata cannot mean Fast off.
- [Cross-report tracker #41220](https://github.com/openai/codex/issues/41220): user reports include #38335's approximately 11 percentage points during a frontend task. These are reports for investigation, not independently verified entitlement findings.
- [Pro 5x report #43413](https://github.com/openai/codex/issues/43413): explicitly lacks exact per-reset timestamps and model-separated measurements. Its aggregate is unsuitable as an automatic baseline.

## Workflow

1. Open **Tier Auditor**. Passive history shows every observed weekly change and the local counters between endpoints. Excluded rows remain visible with reasons.
2. Select the expected tier (user configured), exact model, reasoning effort, input-context band and workload class. Declare Fast off, no subagents and exclusive use of this Mac's signed-in subscription workloads. Avoid API-key workloads, other devices, cloud tasks and other shared-quota features. The software records these declarations; it does not change Codex settings or claim they were independently verified.
3. Start before the workload; stop before changing the protocol. Prices are frozen at start. Previous run records remain in SQLite; v0.2.0 displays the latest run for the current account. Export a completed run before starting another to retain its conveniently viewable report.
4. An independently provisioned Pro 5x account can collect a comparable completed run and export JSON. Import that file with a public GitHub/official URL or `private-reference` for a privately shared reference. The source is a citation; it is not fetched.
5. **Export Evidence Report** writes JSON, HTML and CSV together. Review them locally and share with Support if desired. JSON includes controls, reset timestamps, frozen rates and checksum. HTML is readable; CSV is the interval table and should travel with the methodology.

## Alignment

Successful official polls are ordered across accounts. A row spans the previous change boundary to the next observed change, including intervening unchanged polls. It includes deduplicated local events in `(start, end]`, at one-second resolution. Workloads are distinct session/turn pairs, not response or poll counts. A pending unchanged tail stays separate. Polling observes meter changes, not every server-side debit.

The table shows remaining quota; positive delta means used percentage points. Rows include fresh input, cached input, output, reasoning included in output, API equivalent, unpriced tokens, model/effort sets, Fast/subagent evidence, input extrema and exclusions. No prompts or tool payloads are needed.

```
API USD = (fresh × input rate + cached × cached rate + output × output rate) / 1M
USD / 1% = interval API USD / interval used percentage points
Weekly capacity = 100 × sum(eligible API USD) / sum(eligible percentage points)
```

Public base rates are frozen for each observation, independently of the dashboard's selected pricing mode. Service-tier, long-context, tool-fee and historical pricing adjustments are not reconstructed. This is observed correlation, not OpenAI's quota formula or a subscription bill. Local logs cannot establish complete account coverage. Delayed server accounting and late log flushes can shift usage across boundaries; ten-minute settling reduces this uncertainty without eliminating it.

## Exclusions and quality

Exclude rows for missing/non-live weekly data, reset boundaries, decreases, reset-credit consumption, account switches, plan changes, polling gaps, saturation, no local tokens, unknown prices, missing workload identity, protocol violations or settling. New parser errors during a run block its rows. A polling gap exceeds `clamp(3 × configured poll seconds, 300, 900)`, frozen at start.

Mixed-model/effort intervals are excluded completely. Tokens are never filtered to one model while retaining a whole-account quota delta. Context bands use raw input per response: ≤32,000, ≤128,000, ≤256,000 or larger. This is an observable context proxy, not the configured maximum context window.

The current inspected local format includes model and effort but omits per-turn service tier. Fast therefore stays `UNKNOWN`, or `USER_ATTESTED` during a declared observation; it never becomes `LOG_VERIFIED`. Explicit non-standard service or detected delegation overrides declarations. Old indexed records retain unknown control fields. A bounded first-header read supplies session-source metadata once when an upgraded active file next changes.

| Quality | Minimum evidence |
|---|---|
| INSUFFICIENT | Any gate below is unmet |
| MEDIUM | 12 eligible change windows, 20 distinct workloads, 8 eligible hours, 20 percentage points, fully priced eligible tokens, rate CV ≤25%, finite sensitivity range |
| HIGH | MEDIUM plus 30 windows, 30 workloads, 24 hours, 30 percentage points, and log-verified Fast/subagent controls in every eligible row |

Quality is heuristic and conditional on declarations, not a calibrated probability. User-attested control metadata caps quality at MEDIUM. Missing baseline, stale current quota or incomparable evidence keeps classification INCONCLUSIVE, even when a descriptive capacity estimate is available.

Dispersion is quota-weighted standard deviation of interval rates. The sensitivity range contains both mean ±2 weighted SD and endpoint-quantization bounds. Each reported endpoint is conservatively assumed uncertain by ±1 percentage point; adjacent eligible endpoints cancel. Excluded gaps increase uncertainty. This is not an official precision guarantee or statistical 95% confidence interval.

## Independent comparison

An imported reference must be a completed Pro 5x observation with this method, generated within 30 days. Import checks checksum, price fingerprint, nonoverlapping intervals, canonical counters, positive quota deltas, arithmetic and recomputed summary quality. The reference tier is still its contributor's declaration. Neither a checksum nor this software authenticates the purchased plan or measurement. Same-origin references are rejected.

Comparison requires matching model, effort, context band, workload class and price fingerprint. Mean input per response must be within 25%; cache hit within 10 percentage points; output/input within 5 percentage points; tool calls per response within the larger of 0.25 or 25% of the reference. These conservative v1 rules do not prove task equivalence.

The relative sensitivity range divides current by baseline capacity using opposite endpoints. The entire range must fit a band:

| Assessment | Relative range |
|---|---|
| PRO-5X-LIKE | 0.65–1.50× |
| PRO-20X-LIKE | 2.80–5.60× |
| INCONCLUSIVE | Otherwise, or a prerequisite fails |

The nominal 4× comparison is 20/5, an explicit comparison assumption rather than a fixed weekly-dollar promise. Identical model and effort cannot control every workload or server-side variable.

## Privacy and migration

Schema v3 adds nullable event-control columns and audit-run/origin tables, preserving v2 counters, offsets, settings, prices and quota history. No authentication file or new account scope is required. Export origins are random local pseudonyms; workload labels are report-local ordinals. Reports omit email, account identifiers, original thread/response IDs, paths, prompts and tool payloads. Custom rate-source text is omitted; public citations are limited to safe official URLs.

Native commands expose fixed audit operations, not arbitrary SQL or file paths. Exports are created without overwrite with owner-only permissions. Imports are limited to 2 MiB. No real evidence or private reference is committed or shipped.

Validation commands:

```
cargo test --manifest-path crates/core/Cargo.toml --test tier_auditor
cargo test --manifest-path crates/core/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --test watcher
npm run build
npm run package -- --bundles app,dmg
```

See TEST_REPORT.md for actual results and remaining acceptance limits.
