# Token accounting

All views and exports use `crates/core/src/accounting.rs`.

| Field | Definition |
|---|---|
| raw_input_tokens | All input, including cached input |
| cached_input_tokens | Cached subset of raw input |
| uncached_input_tokens | raw input minus cached input |
| output_tokens | All output, including reasoning where reported |
| reasoning_output_tokens | Subset of output; displayed separately, never added again |
| total_tokens | raw input + output |
| cache hit | cached / raw; unavailable when raw is zero |

Malformed negative/overflow counters or cached > input / reasoning > output are rejected and counted as parser errors. Valid data therefore needs no clamping to disguise inconsistencies. Summed ratios use summed input, not averages of percentages.

## Record identity and ownership

Modern `token_usage_record` uses per-response `usage`, authoritative `thread_id`, and unique `response_id`. Legacy `event_msg/token_count` uses `last_token_usage`; cumulative totals identify duplicate notifications and are **never added as per-request usage**. A fingerprint of thread, cumulative counter vector and reset epoch reconciles modern/legacy paired records. Without cumulative or response identity, timestamp and counters provide a weaker fallback; such old logs cannot prove distinct identical requests at the same timestamp.

The first `session_meta.id` identifies the thread. In current logs `session_id` can name the root lineage, so it cannot override `id`. Fork snapshots may contain the parent's header and inherited usage with rewritten timestamps. The first header remains authoritative; records at/before the fork-header timestamp seed context/cumulative state but are excluded from new usage. Missing parent files do not turn copied history into new fork spend. Tool arguments, messages and reasoning bodies are discarded.

Moves, archives, copies, process restarts and unchanged rescans preserve identities. Source rewrites unlink old derived events before re-indexing. A cumulative decrease starts a new epoch. Local logs can be incomplete; this is observed local usage, not a complete account-wide ledger.

## Time and advanced metrics

Today, week (Monday), month, year and custom dates use an IANA timezone and actual calendar midnights, including 23/25-hour DST days. Five hours, 30 days and 90 days are rolling intervals. Custom end dates are inclusive. A session's counters match the selected range; its start is the observed session metadata start.

Burn/efficiency needs >=4 successful samples, >=30 minutes, >=1 percentage point increase, a fresh last sample, and a single account/reset window with monotonically increasing usage. Reset crossings and stale samples stop an estimate. Quota is account-wide while token logs are local; observed efficiency is correlation, not an official billing rule. Exhaustion forecasts say when the quota window would reset first.

## Tier Auditor

Audit intervals use `(start, end]` and the same canonical counters. Unchanged polls are carried forward to the next change. Whole mixed-model intervals are excluded rather than pairing filtered tokens with account-wide quota. Controls, quality gates, quantization assumptions and limits are defined in AUDITOR.md.
