# Localization / 本地化

The shipped languages are Simplified Chinese (`zh-CN`, default) and English (`en`). No translation service or new runtime dependency is used. Existing React subscriptions and browser `Intl` format presentation; Rust reads the same dictionary for native menu labels and HTML reports.

## Files and rules

- `locales/zh-CN.json`: English source strings mapped to Chinese. Preserve every named `{placeholder}` exactly; parameters are inserted once without evaluation.
- `src/i18n.ts`: explicit `t()` calls, persisted browser preference and React subscription. `Settings.language` in the app's SQLite settings is authoritative in the installed app.
- `src/components/LanguageSwitch.tsx`: the native `set_language` command changes only language. A language event synchronizes both windows without overwriting unsaved settings fields.
- `crates/core/src/locale.rs`: the same dictionary for native presentation. Canonical storage and evidence JSON are never translated.
- `crates/core/src/export.rs` and `auditor.rs`: HTML uses the requested language; CSV and JSON stay language-independent. The legacy `render()` functions preserve their English behavior for existing callers.

Keep model IDs, reasoning/protocol option values, pricing keys, user-provided names and URLs unchanged. Translate their explanatory labels, not their values. Unknown external text falls back to its original wording. The HTML evidence appendix is a translated presentation copy; its JSON source remains canonical for checksum validation and baseline import. HTML escapes both labels and data.

Use a complete sentence with placeholders where word order differs. Dates and compact numbers use the active language but retain the user's selected IANA time zone. Language does not change accounting, pricing, audit eligibility or classification thresholds. Older preferences without a language field load as Chinese with no database schema change.

## Checks

```sh
node --test tests/localization.test.mjs
cargo test --manifest-path crates/core/Cargo.toml --test localization
npm test
npm run build
```

The targeted tests verify locale restoration, switching in both directions, placeholder parity, literal translation coverage, old-settings compatibility, validation, HTML escaping and unchanged canonical CSV/JSON. Visually check both languages, long audit explanations and the compact panel after changing presentation. Record native and browser acceptance separately in `TEST_REPORT.md`.

## 中文贡献说明

修改 `locales/zh-CN.json` 即可更新中文文案；英文使用源字符串。保留所有占位符和技术参数，中文与英文的字段顺序可以不同。新增界面文字需要显式使用翻译函数，并补齐词典。主界面、小窗、托盘和 HTML 导出应一起检查；不要翻译用于基准校验的 JSON、CSV 字段或数据。
