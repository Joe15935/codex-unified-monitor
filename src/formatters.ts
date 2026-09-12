import type { Language } from "./i18n";

type NumberStyle = "compact" | "exact" | "usd";
const numberOptions: Record<NumberStyle, Intl.NumberFormatOptions> = {
  compact: { notation: "compact", maximumFractionDigits: 2 },
  exact: {},
  usd: { style: "currency", currency: "USD", maximumFractionDigits: 2 },
};

// Cache formatter instances, never formatted values or usage records. Both
// caches are bounded; the date cache also tolerates repeated timezone edits.
const numberFormats = new Map<string, Intl.NumberFormat>();
const dateFormats = new Map<string, Intl.DateTimeFormat>();
const NUMBER_CACHE_LIMIT = 6;
const DATE_CACHE_LIMIT = 16;

function cached<T>(
  cache: Map<string, T>,
  key: string,
  limit: number,
  create: () => T,
): T {
  const existing = cache.get(key);
  if (existing !== undefined) {
    cache.delete(key);
    cache.set(key, existing);
    return existing;
  }
  // An invalid locale/timezone must retain Intl's error behavior and must not
  // evict an otherwise valid entry.
  const value = create();
  if (cache.size >= limit) {
    const oldest = cache.keys().next().value;
    if (oldest !== undefined) cache.delete(oldest);
  }
  cache.set(key, value);
  return value;
}

function numberFormat(language: Language, style: NumberStyle) {
  return cached(
    numberFormats,
    `${language}:${style}`,
    NUMBER_CACHE_LIMIT,
    () => new Intl.NumberFormat(language, numberOptions[style]),
  );
}

export const formatCount = (n: number, language: Language) =>
  numberFormat(language, "compact").format(n);

export const formatExact = (n: number, language: Language) =>
  numberFormat(language, "exact").format(n);

export const formatUsd = (n: number, language: Language) =>
  numberFormat(language, "usd").format(n);

export function formatDate(
  seconds: number,
  timeZone: string,
  language: Language,
) {
  const formatter = cached(
    dateFormats,
    `${language}\u0000${timeZone}`,
    DATE_CACHE_LIMIT,
    () =>
      new Intl.DateTimeFormat(language, {
        timeZone,
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      }),
  );
  return formatter.format(new Date(seconds * 1000));
}
