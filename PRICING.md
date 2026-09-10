# API equivalent estimates

`pricing_catalog.json` contains independent `public_api` and `codex_work` entries. Each has model, per-million USD input/cached/output rates, optional cache-write rate, source, verified date, optional effective date, processing/context notes and promotion caveats. Verified 2026-09-10 against:

- https://developers.openai.com/api/docs/pricing
- https://help.openai.com/en/articles/20001415-chatgpt-rate-card-enterprise-token-based-pricing
- The official per-model pages linked in each catalog entry.

For each priced event:

```
USD = (uncached_input × input_rate + cached_input × cached_rate + output × output_rate) / 1,000,000
cache value saved = cached_input × (input_rate − cached_rate) / 1,000,000
```

Reasoning is already part of output and is never charged twice. Cache-write rates may be recorded as provenance but are not charged by the base estimate. This is **not a ChatGPT/Codex subscription bill, OpenAI's cost, profit, or a promise of savings**.

Both modes currently share base token rates for the bundled overlapping models but remain separate catalogs. Astra's Codex context treatment and public API long-context pricing can differ. Sol has promotional-rate notes. Fast/Priority/Flex/Batch rates and historical price changes cannot safely be inferred from every rollout. Therefore all ranges, including old logs, use the currently selected base catalog: **Base-rate API equivalent estimate; long-context/request-level adjustments not reconstructed.** Effective dates are not invented when sources do not supply them.

Unknown models have no default rate. Reports expose known subtotal, priced and unpriced tokens, and token-weighted coverage; an entirely unpriced nonempty group displays UNPRICED rather than zero. No implicit dated-name stripping or Spark/code-review mapping is applied. Settings permits explicit aliases and custom rates per mode, with validation and provenance. Import a local catalog JSON in Settings to update prices without rebuilding. Imports stay in SQLite and do not overwrite the bundled file.

Monthly subscription cost defaults to unset. With a positive user-supplied amount, `Equivalent Value Multiple = known monthly equivalent / configured monthly subscription`. It remains a partial estimate when pricing coverage is incomplete. Break-even progress describes this same theoretical equivalent value, not financial return.
