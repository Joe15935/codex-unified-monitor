# Data sources and provenance

| Data | Source | Normal state | Failure / absence |
|---|---|---|---|
| Token counters, model, effort, tools, session metadata | local Codex rollout JSONL | LOCAL | parser diagnostics; no fabricated usage |
| Account and plan | official account/read | LIVE | UNAVAILABLE / last known masked account |
| Quota windows, credits, reset details | official account/rateLimits/read | LIVE | CACHED after restart, STALE after failed refresh, UNAVAILABLE if absent |
| Optional official usage summaries | official account/usage/read | LIVE | CACHED, UNSUPPORTED or UNAVAILABLE independently |
| Price catalog and equivalent values | documented current rates × local counters | ESTIMATED | UNKNOWN / UNPRICED models and explicit coverage |
| Burn / efficiency | saved official snapshots + local interval | observed estimate | Collecting data / Unavailable |

Each primary object carries source, timestamp, status and confidence. Official usage summaries are displayed separately; they are not added to local token totals. Model-specific buckets, including Spark when returned, are separate from the main Codex quota. Available reset-credit count is authoritative; missing details differ from an empty details list.

Executable discovery prefers installed ChatGPT/Codex desktop bundle CLI, then common user/Homebrew/PATH locations. Developers can explicitly set CODEXMETER_CODEX_BINARY. No authentication files are opened by the component. The official CLI uses its existing supported authentication mechanism. The experimental methods can change; unsupported methods are handled as unavailable.

Official protocol reference: https://learn.chatgpt.com/docs/app-server

Initial compatibility checked on 2026-09-10 against bundled Codex 0.153.4. On the acceptance account the main primary window was 10,080 minutes, secondary was null, and reset-credit count was available. This is evidence for duration-based classification, not a guarantee about other plans.
