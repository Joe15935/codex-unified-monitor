import { useSyncExternalStore } from "react";
import messages from "../locales/zh-CN.json";

export type Language = "en" | "zh-CN";
const storageKey = "codex-monitor-language";
const translations: Record<string, string> = messages;
const listeners = new Set<() => void>();
let language: Language = "zh-CN";
try {
  const saved = localStorage.getItem(storageKey);
  if (saved === "en" || saved === "zh-CN") language = saved;
} catch {
  // The persisted native preference remains authoritative if web storage is unavailable.
}

export function getLanguage(): Language {
  return language;
}
export function applyLanguage(next: Language) {
  if (next !== "en" && next !== "zh-CN") return;
  const changed = language !== next;
  language = next;
  document.documentElement.lang = next;
  try {
    localStorage.setItem(storageKey, next);
  } catch {}
  if (changed) listeners.forEach((listener) => listener());
}
function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
export function useLanguage() {
  return useSyncExternalStore(subscribe, getLanguage);
}

/** Translate presentation text only. Model names, JSON fields and protocol values stay stable. */
export function t(
  source: string | null | undefined,
  values: Record<string, string | number> = {},
): string {
  if (!source) return source ?? "";
  let key = source.replace(/\s+/g, " ").trim();
  const accountError = key.match(
    /^Account request failed \(code (-?\d+)\); check official Codex sign-in$/,
  );
  if (accountError) {
    key = "Account request failed (code {code}); check official Codex sign-in";
    values = { ...values, code: accountError[1] };
  }
  let text = language === "zh-CN" ? (translations[key] ?? source) : source;
  return text.replace(/\{(\w+)\}/g, (match, name: string) =>
    Object.prototype.hasOwnProperty.call(values, name)
      ? String(values[name])
      : match,
  );
}
