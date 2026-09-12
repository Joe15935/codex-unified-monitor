import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Aggregate, Money } from "./types";
import { getLanguage, t } from "./i18n";
import { formatCount, formatDate, formatExact, formatUsd } from "./formatters";
export const native = isTauri();
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (native) return invoke<T>(command, args);
  throw new Error("Open the desktop app to read your local Codex data.");
}
export const count = (n: number) => formatCount(n, getLanguage());
export const exact = (n: number) => formatExact(n, getLanguage());
export const usd = (n: number | null | undefined) =>
  n == null ? "—" : formatUsd(n, getLanguage());
export const money = (a: Aggregate, mode: string): Money =>
  mode === "codex_work" ? a.codex_work : a.public_api;
export const value = (m: Money) =>
  m.priced_tokens === 0 && m.unpriced_tokens > 0 ? null : m.known_value_usd;
export const coverage = (m: Money) =>
  m.priced_tokens + m.unpriced_tokens
    ? m.priced_tokens / (m.priced_tokens + m.unpriced_tokens)
    : null;
export const ratio = (a: Aggregate) =>
  a.tokens.raw_input_tokens
    ? a.tokens.cached_input_tokens / a.tokens.raw_input_tokens
    : null;
export const percent = (n: number | null) =>
  n == null ? "—" : `${(n * 100).toFixed(1)}%`;
export function duration(seconds: number) {
  const n = Math.max(0, Math.round(seconds));
  const h = Math.floor(n / 3600);
  const m = Math.floor((n % 3600) / 60);
  return h >= 24
    ? t("{days}d {hours}h", { days: Math.floor(h / 24), hours: h % 24 })
    : h > 0
      ? t("{hours}h {minutes}m", { hours: h, minutes: m })
      : t("{minutes}m {seconds}s", { minutes: m, seconds: n % 60 });
}
export const date = (seconds: number | null | undefined, tz: string) =>
  seconds ? formatDate(seconds, tz, getLanguage()) : t("Unavailable");
export const priceNote =
  "Theoretical value at published token rates. This is not a ChatGPT/Codex subscription bill and does not represent OpenAI’s actual costs. Long-context, cache-write, service-tier and request-level adjustments are not reconstructed.";
