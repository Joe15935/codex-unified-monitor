import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Aggregate, Money } from "./types";
export const native = isTauri();
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (native) return invoke<T>(command, args);
  throw new Error("Open the desktop app to read your local Codex data.");
}
export const count = (n: number) =>
  new Intl.NumberFormat("en-US", {
    notation: "compact",
    maximumFractionDigits: 2,
  }).format(n);
export const exact = (n: number) => new Intl.NumberFormat("en-US").format(n);
export const usd = (n: number | null | undefined) =>
  n == null
    ? "—"
    : new Intl.NumberFormat("en-US", {
        style: "currency",
        currency: "USD",
        maximumFractionDigits: 2,
      }).format(n);
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
    ? `${Math.floor(h / 24)}d ${h % 24}h`
    : h > 0
      ? `${h}h ${m}m`
      : `${m}m ${n % 60}s`;
}
export const date = (seconds: number | null | undefined, tz: string) =>
  seconds
    ? new Intl.DateTimeFormat("en-US", {
        timeZone: tz,
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      }).format(new Date(seconds * 1000))
    : "Unavailable";
export const priceNote =
  "Theoretical value at published token rates. This is not a ChatGPT/Codex subscription bill and does not represent OpenAI’s actual costs. Long-context, cache-write, service-tier and request-level adjustments are not reconstructed.";
